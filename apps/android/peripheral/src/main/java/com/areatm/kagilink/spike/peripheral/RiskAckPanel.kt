package com.areatm.kagilink.spike.peripheral

import android.app.Activity
import android.content.Context
import android.view.View
import android.widget.Button
import android.widget.CheckBox
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import com.areatm.kagilink.riskack.AckGate
import com.areatm.kagilink.riskack.NoticeTexts
import com.areatm.kagilink.riskack.RiskAck
import com.areatm.kagilink.riskack.RiskRecord

/**
 * Risk acknowledgement screen section (D023, docs/RISK_ACK_SCREEN_SPEC.md). The decision logic lives in
 * the pure-Kotlin apps/android/riskack module; this class only wires views. The notice text is a draft
 * (not legally reviewed). Separate from the per-transfer approval. No secrets are handled here.
 */
class RiskAckPanel(private val activity: Activity, private val onStateChanged: () -> Unit) {
    private val prefs = activity.getSharedPreferences("kl_risk_ack", Context.MODE_PRIVATE)
    private val texts: NoticeTexts? = NoticeTexts.load { key ->
        val id = activity.resources.getIdentifier(key, "string", activity.packageName)
        if (id == 0) null else activity.getString(id)
    }
    private val version: Int? = try { activity.resources.getInteger(R.integer.risk_notice_version) } catch (e: Exception) { null }
    val gate = AckGate(textsLoaded = texts != null && version != null)

    private lateinit var checkbox: CheckBox
    private lateinit var continueBtn: Button
    private lateinit var fullBox: ScrollView
    private lateinit var fullBtn: Button
    val view: LinearLayout = build()

    private fun pad() = (8 * activity.resources.displayMetrics.density).toInt()

    private fun build(): LinearLayout {
        val box = LinearLayout(activity).apply { orientation = LinearLayout.VERTICAL; id = R.id.risk_ack_box }
        val t = texts
        if (t == null || version == null) {
            box.addView(TextView(activity).apply {
                id = R.id.risk_ack_unavailable
                text = activity.getString(R.string.risk_unavailable)
                textSize = 16f
            })
            return box
        }
        box.addView(TextView(activity).apply { text = t["risk_short_1"]; textSize = 18f; setPadding(0, pad(), 0, pad()) })
        box.addView(TextView(activity).apply { text = t["risk_short_2"]; textSize = 18f; setPadding(0, 0, 0, pad()) })
        box.addView(TextView(activity).apply { text = t["risk_draft_marker"]; textSize = 14f; setTypeface(typeface, android.graphics.Typeface.ITALIC) })
        fullBtn = Button(activity).apply {
            id = R.id.risk_ack_full_button
            text = activity.getString(R.string.risk_full_open)
            setOnClickListener {
                val open = fullBox.visibility != View.VISIBLE
                fullBox.visibility = if (open) View.VISIBLE else View.GONE
                text = activity.getString(if (open) R.string.risk_full_close else R.string.risk_full_open)
            }
        }
        box.addView(fullBtn)
        fullBox = ScrollView(activity).apply {
            visibility = View.GONE
            val col = LinearLayout(activity).apply { orientation = LinearLayout.VERTICAL }
            col.addView(TextView(activity).apply { text = t["risk_full_title"]; textSize = 16f; setTypeface(typeface, android.graphics.Typeface.BOLD) })
            for (i in 1..6) col.addView(TextView(activity).apply { text = "$i. " + t["risk_full_$i"]; textSize = 14f; setPadding(0, pad() / 2, 0, 0) })
            addView(col)
            layoutParams = LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, (220 * activity.resources.displayMetrics.density).toInt())
        }
        box.addView(fullBox)
        checkbox = CheckBox(activity).apply {
            id = R.id.risk_ack_checkbox
            text = t["risk_checkbox"]
            textSize = 16f
            isChecked = false // never pre-checked
            setOnCheckedChangeListener { _, v -> gate.setChecked(v); refresh() }
        }
        box.addView(checkbox)
        continueBtn = Button(activity).apply {
            id = R.id.risk_ack_continue
            text = activity.getString(R.string.risk_continue)
            isEnabled = false
            setOnClickListener { onContinue() }
        }
        box.addView(continueBtn)
        return box
    }

    /** Stored record (version, receiver hash, time) -> does this transfer need the notice? Fail-safe: yes. */
    fun noticeNeeded(): Boolean {
        val v = version ?: return true
        val rec = try { RiskRecord.decode(prefs.getString("record", null)) } catch (e: Exception) { null }
        // No stable receiver identity exists yet (the KL-XXXX tag is per session): unknown identity -> always show.
        return RiskAck.needsNotice(rec?.version, rec?.receiverHash, v, currentReceiverHash = null)
    }

    /** The notice text loaded and, when needed, the user acknowledged it. Gate for Start. */
    fun mayStartTransfer(): Boolean = gate.mayStartTransfer(noticeNeeded())

    /** The start form (code entry, Start) is visible only when a transfer may start. */
    fun formVisible(): Boolean = mayStartTransfer()

    private fun onContinue() {
        if (!gate.continueClicked()) return
        val v = version
        if (v != null) {
            try {
                prefs.edit().putString("record", RiskRecord(v, null, System.currentTimeMillis() / 1000).encode()).apply()
            } catch (e: Exception) { /* a failed write only means the screen is shown again next time */ }
        }
        refresh()
        onStateChanged()
    }

    /** Back navigation, end of a transfer or backgrounding: nothing acknowledged is left behind. */
    fun reset() {
        gate.reset()
        refresh()
        onStateChanged()
    }

    private fun refresh() {
        if (texts == null || version == null) return
        if (checkbox.isChecked != gate.checked) checkbox.isChecked = gate.checked
        continueBtn.isEnabled = gate.canContinue
        view.visibility = if (mayStartTransfer()) View.GONE else View.VISIBLE
    }

    fun show() { refresh() }
}
