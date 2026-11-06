package com.areatm.kagilink.riskack

/**
 * Risk acknowledgement logic (D023, docs/RISK_ACK_SCREEN_SPEC.md). Pure Kotlin, no Android types, so
 * the same code is unit-tested on a host JVM and on the device. Synthetic data only; no secrets here.
 */
object RiskAck {
    /**
     * True when the notice must be shown before a transfer. Fail-safe: only a stored record that has a
     * stable receiver identity equal to the current one AND the current notice version skips the notice.
     * An unknown current identity (today: always, the KL-XXXX tag is per session and not an identity)
     * means "show". Missing/corrupt record (nulls) means "show".
     */
    fun needsNotice(
        storedVersion: Int?, storedReceiverHash: String?,
        currentVersion: Int, currentReceiverHash: String?,
    ): Boolean {
        if (currentReceiverHash == null) return true
        if (storedVersion == null || storedReceiverHash == null) return true
        if (storedVersion != currentVersion) return true
        return storedReceiverHash != currentReceiverHash
    }
}

/** Local, non-secret record: notice version, hash of the claimed receiver identity (or none) and time. */
class RiskRecord(val version: Int, val receiverHash: String?, val epochSeconds: Long) {
    fun encode(): String = "KLRA1;$version;${receiverHash ?: "-"};$epochSeconds"

    companion object {
        private val FORMAT = Regex("^KLRA1;(\\d{1,6});([0-9a-f]{64}|-);(\\d{1,12})$")

        /** Anything that does not match exactly (including null) is "no record" and shows the screen. */
        fun decode(s: String?): RiskRecord? {
            val m = FORMAT.matchEntire(s ?: return null) ?: return null
            val (v, h, t) = m.destructured
            return RiskRecord(v.toInt(), if (h == "-") null else h, t.toLong())
        }
    }
}

/** All notice texts of one language; built only when every required text is present and non-blank. */
class NoticeTexts private constructor(val byKey: Map<String, String>) {
    operator fun get(key: String): String = byKey.getValue(key)

    companion object {
        val KEYS = listOf(
            "risk_short_1", "risk_short_2", "risk_checkbox", "risk_draft_marker", "risk_full_title",
            "risk_full_1", "risk_full_2", "risk_full_3", "risk_full_4", "risk_full_5", "risk_full_6",
        )

        /** null = the text cannot be loaded -> the screen blocks the transfer (fail closed). */
        fun load(fetch: (String) -> String?): NoticeTexts? {
            val m = LinkedHashMap<String, String>()
            for (k in KEYS) {
                val v = try { fetch(k) } catch (e: Exception) { null }
                if (v == null || v.isBlank()) return null
                m[k] = v
            }
            return NoticeTexts(m)
        }
    }
}

/**
 * State of the acknowledgement screen. The checkbox starts unchecked, Continue is disabled until it is
 * checked, back clears everything, and acknowledgement never survives a transfer. This is NOT the
 * per-transfer approval (that stays a separate button).
 */
class AckGate(private val textsLoaded: Boolean) {
    var checked = false
        private set
    var acknowledged = false
        private set

    val canContinue: Boolean get() = textsLoaded && checked && !acknowledged

    fun setChecked(v: Boolean) { if (!acknowledged) checked = v }

    /** Returns true only when the continue button is enabled; otherwise nothing changes. */
    fun continueClicked(): Boolean {
        if (!canContinue) return false
        acknowledged = true
        return true
    }

    /** Back navigation, end of a transfer or backgrounding: no acknowledged state is left behind. */
    fun reset() { checked = false; acknowledged = false }

    /** The transfer may start only with loaded texts and, when the notice is needed, an acknowledgement. */
    fun mayStartTransfer(noticeNeeded: Boolean): Boolean = textsLoaded && (!noticeNeeded || acknowledged)
}
