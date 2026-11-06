package com.areatm.kagilink.spike.peripheral

import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import android.os.Bundle
import android.text.InputType
import android.view.Gravity
import android.view.WindowManager
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ArrayAdapter
import android.widget.ScrollView
import android.widget.Spinner
import android.widget.TextView
import com.areatm.kagilink.spike.Native

/**
 * Minimal diagnostic UI for the KL-V1 peripheral spike. SYNTHETIC DATA ONLY: the user types the
 * 6-digit code shown by the receiver (a throwaway test code), presses start, and later approves
 * one synthetic SECRET transfer. Not a product UI. Window is FLAG_SECURE.
 */
class PeripheralActivity : Activity(), PeripheralSession.Listener, TestHookHost {
    private lateinit var codeField: EditText
    private lateinit var notifyBox: CheckBox
    private var faultSpinner: Spinner? = null
    private var keepFault: CheckBox? = null
    private var modeView: TextView? = null
    private lateinit var tagView: TextView
    private lateinit var statusView: TextView
    private lateinit var startBtn: Button
    private lateinit var approveBtn: Button
    private lateinit var stopBtn: Button
    private lateinit var session: PeripheralSession

    override fun onCreate(b: Bundle?) {
        super.onCreate(b)
        window.setFlags(WindowManager.LayoutParams.FLAG_SECURE, WindowManager.LayoutParams.FLAG_SECURE)
        Native.loadFromApk()
        check(Native.abiVersion() == Native.EXPECTED_ABI) { "ABI mismatch" }
        session = PeripheralSession(applicationContext, this)
        TestHookRegistry.host = this
        val pad = (16 * resources.displayMetrics.density).toInt()
        val col = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(pad, pad * 2, pad, pad) }
        col.addView(TextView(this).apply { text = "KagiLink KL-V1 peripheral spike\n(synthetic data only, not a product)"; textSize = 16f })
        val prefs = getSharedPreferences("kl_test_settings", MODE_PRIVATE) // test settings only, never the code
        codeField = EditText(this).apply {
            hint = "6-digit test code from the receiver"
            inputType = InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_VARIATION_PASSWORD
            filters = arrayOf(android.text.InputFilter.LengthFilter(6))
        }
        notifyBox = CheckBox(this).apply {
            text = "Notify + app ACK mode (default: Indicate)"
            isChecked = prefs.getBoolean("notify", false)
            setOnCheckedChangeListener { _, v -> prefs.edit().putBoolean("notify", v).apply(); showMode() }
        }
        if (FaultFactory.labels.isNotEmpty()) { // only debug builds have fault injection
            modeView = TextView(this).apply { textSize = 22f; setTypeface(typeface, android.graphics.Typeface.BOLD) }
            col.addView(modeView)
            col.addView(TextView(this).apply { text = FaultFactory.heading })
            faultSpinner = Spinner(this).apply {
                adapter = ArrayAdapter(this@PeripheralActivity, android.R.layout.simple_spinner_dropdown_item,
                    FaultFactory.labels)
                setSelection(prefs.getInt("fault", 0).coerceIn(0, FaultFactory.labels.size - 1))
                onItemSelectedListener = object : android.widget.AdapterView.OnItemSelectedListener {
                    override fun onItemSelected(p: android.widget.AdapterView<*>?, v: android.view.View?, pos: Int, id: Long) {
                        prefs.edit().putInt("fault", pos).apply(); showMode()
                    }
                    override fun onNothingSelected(p: android.widget.AdapterView<*>?) {}
                }
            }
            col.addView(faultSpinner)
            keepFault = CheckBox(this).apply {
                text = FaultFactory.keepLabel
                isChecked = prefs.getBoolean("keep", true) // debug build default: keep the chosen mode
                setOnCheckedChangeListener { _, v -> prefs.edit().putBoolean("keep", v).apply() }
            }
            col.addView(keepFault)
            showMode()
        }
        col.addView(notifyBox)
        col.addView(codeField) // directly above Start: type the code, tap Start
        startBtn = Button(this).apply { text = "Start (advertise 60 s)"; setOnClickListener { onStartClicked() } }
        col.addView(startBtn)
        tagView = TextView(this).apply { textSize = 48f; gravity = Gravity.CENTER; text = "----" }
        col.addView(tagView)
        approveBtn = Button(this).apply {
            text = "Approve ONE synthetic SECRET transfer"; isEnabled = false
            setOnClickListener { isEnabled = false; session.approveAndSendSecret() }
        }
        col.addView(approveBtn)
        stopBtn = Button(this).apply { text = "Stop"; setOnClickListener { session.stop() } }
        col.addView(stopBtn)
        statusView = TextView(this).apply { textSize = 13f; setTextIsSelectable(true) }
        col.addView(ScrollView(this).apply { addView(statusView) })
        setContentView(col)
    }

    private fun onStartClicked() {
        val need = arrayOf(Manifest.permission.BLUETOOTH_ADVERTISE, Manifest.permission.BLUETOOTH_CONNECT)
        if (need.any { checkSelfPermission(it) != PackageManager.PERMISSION_GRANTED }) {
            requestPermissions(need, 1)
            return
        }
        val digits = codeField.text.toString()
        if (digits.length != 6 || !digits.all { it in '0'..'9' }) { status("enter exactly 6 digits"); return }
        val code = digits.toByteArray(Charsets.US_ASCII)
        codeField.text.clear()
        startBtn.isEnabled = false
        statusView.text = ""
        val plan = FaultFactory.create(faultSpinner?.selectedItemPosition ?: 0)
        session.start(code, notifyBox.isChecked, plan) // session wipes `code`
    }

    override fun onRequestPermissionsResult(rc: Int, perms: Array<out String>, res: IntArray) {
        if (res.isNotEmpty() && res.all { it == PackageManager.PERMISSION_GRANTED }) status("permissions granted; press Start")
        else status("Nearby devices permission denied")
    }

    private fun showMode() {
        val pos = faultSpinner?.selectedItemPosition ?: return
        modeView?.text = "MODE: " + FaultFactory.labels[pos] + "\n" + (if (notifyBox.isChecked) "Notify+ACK" else "Indicate")
    }

    private fun status(t: String) = runOnUiThread { statusView.append(t + "\n") }
    override fun onStatus(text: String) = status(text)
    override fun onTag(tag: String) { runOnUiThread { tagView.text = tag } }
    override fun onApprovalReady() { runOnUiThread { approveBtn.isEnabled = true } }
    override fun onFinished(summary: String) {
        runOnUiThread {
            statusView.append(summary + "\n")
            if (keepFault?.isChecked != true) faultSpinner?.setSelection(0); showMode()
            startBtn.isEnabled = true; approveBtn.isEnabled = false; tagView.text = "----"
        }
    }

    override fun onDestroy() { if (TestHookRegistry.host === this) TestHookRegistry.host = null; super.onDestroy() }

    // Debug-only automation entry (the only caller is the debug receiver). Runs on the main thread.
    override fun testStart(code: ByteArray, notify: Boolean, plan: FaultPlan): String? {
        val need = arrayOf(Manifest.permission.BLUETOOTH_ADVERTISE, Manifest.permission.BLUETOOTH_CONNECT)
        if (need.any { checkSelfPermission(it) != PackageManager.PERMISSION_GRANTED }) return "Nearby devices permission not granted"
        session.stop() // a running session ends first; the new start is queued behind it
        codeField.text.clear()
        statusView.text = ""
        startBtn.isEnabled = false
        session.start(code, notify, plan) // session wipes `code`
        return null
    }

    override fun onStop() { super.onStop(); session.stop() } // fail closed when backgrounded
}
