package `in`.minori.key.phase0.strongbox

import android.app.Activity
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.ViewGroup
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

class MainActivity : Activity() {
    private lateinit var output: TextView
    private val tag = "KagiStrongBox"

    private fun report(result: Result) {
        Log.i(tag, result.line())
        runOnUiThread { output.append(result.line() + "\n\n") }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val pad = (16 * resources.displayMetrics.density).toInt()
        val layout = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(pad, pad * 2, pad, pad) }
        layout.addView(TextView(this).apply {
            textSize = 18f
            text = "KagiLink StrongBox Phase 0"
        })
        layout.addView(TextView(this).apply {
            textSize = 12f
            text = "Diagnostic only. Synthetic random data, temporary keys deleted, nothing stored or sent " +
                "(no network permission). No TEE/software fallback. Offline attestation proves chain " +
                "consistency only; revocation/status freshness cannot be fully checked offline and " +
                "trusted-root pinning is NOT done. Not approval for any password storage or transfer."
        })
        val run = Button(this).apply { text = "Run StrongBox checks"; setOnClickListener { runChecks() } }
        val bio = Button(this).apply { text = "Run biometric per-use test (needs fingerprint)"; setOnClickListener { BiometricCheck(this@MainActivity, ::report).start() } }
        layout.addView(run)
        layout.addView(bio)
        output = TextView(this).apply { typeface = android.graphics.Typeface.MONOSPACE; textSize = 11f; setTextIsSelectable(true) }
        layout.addView(output)
        setContentView(ScrollView(this).apply {
            addView(layout, ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        })
        if (intent.getBooleanExtra("autorun", false)) runChecks()
    }

    private fun runChecks() {
        Log.i(tag, "INFO model=${Build.MODEL} android=${Build.VERSION.RELEASE} sdk=${Build.VERSION.SDK_INT}")
        Thread { StrongBoxChecks.runNonBiometric(this).forEach(::report) }.start()
    }
}
