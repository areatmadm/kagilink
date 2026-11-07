package com.areatm.kagilink.riskack

import kotlin.system.exitProcess

private var checks = 0
private var failures = 0
private fun check(ok: Boolean, what: String) {
    checks++
    if (!ok) { failures++; println("FAIL: $what") }
}

private const val H1 = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90"
private const val H2 = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"

private fun testNeedsNotice() {
    // First use: nothing stored.
    check(RiskAck.needsNotice(null, null, 1, H1), "first use shows the notice")
    check(RiskAck.needsNotice(null, null, 1, null), "first use, unknown identity shows the notice")
    // Same version and same stable receiver: the only case that may skip.
    check(!RiskAck.needsNotice(1, H1, 1, H1), "same version and same receiver: no notice")
    // Changed receiver.
    check(RiskAck.needsNotice(1, H1, 1, H2), "changed receiver shows the notice")
    // Bumped version.
    check(RiskAck.needsNotice(1, H1, 2, H1), "bumped version shows the notice")
    check(RiskAck.needsNotice(2, H1, 1, H1), "older current version than stored shows the notice")
    // Unknown identity (today's reality): always shown, even with a stored record.
    check(RiskAck.needsNotice(1, H1, 1, null), "unknown current identity shows the notice")
    check(RiskAck.needsNotice(1, null, 1, null), "no identity at all shows the notice")
    check(RiskAck.needsNotice(1, null, 1, H1), "stored record without identity shows the notice")
    // Corrupt record parses to null parts.
    val corrupt = RiskRecord.decode("garbage")
    check(corrupt == null, "corrupt record decodes to null")
    check(RiskAck.needsNotice(corrupt?.version, corrupt?.receiverHash, 1, H1), "corrupt record shows the notice")
}

private fun testRecord() {
    val r = RiskRecord(3, H1, 1_800_000_000L)
    val back = RiskRecord.decode(r.encode())
    check(back != null && back.version == 3 && back.receiverHash == H1 && back.epochSeconds == 1_800_000_000L, "record round trip")
    val none = RiskRecord.decode(RiskRecord(1, null, 5).encode())
    check(none != null && none.receiverHash == null, "record without receiver identity round trip")
    for (bad in listOf(null, "", "KLRA1", "KLRA1;1;x;5", "KLRA2;1;-;5", "KLRA1;1;-;-5", "KLRA1;1;-;5;extra",
        "KLRA1;1;${H1.uppercase()};5", "KLRA1;99999999;-;5", " KLRA1;1;-;5")) {
        check(RiskRecord.decode(bad) == null, "rejects '${bad}'")
    }
    // The record carries no code/secret/personal field: only version, optional hash, time.
    check(r.encode().split(";").size == 4, "record has exactly four fields")
}

private fun testTexts() {
    val full = NoticeTexts.KEYS.associateWith { "text for $it" }
    check(NoticeTexts.load { full[it] } != null, "all texts present loads")
    for (missing in NoticeTexts.KEYS) {
        check(NoticeTexts.load { if (it == missing) null else full[it] } == null, "missing $missing -> not loaded")
        check(NoticeTexts.load { if (it == missing) "   " else full[it] } == null, "blank $missing -> not loaded")
        check(NoticeTexts.load { if (it == missing) throw IllegalStateException("no resource") else full[it] } == null,
            "resource exception for $missing -> not loaded")
    }
}

private fun testGate() {
    var g = AckGate(textsLoaded = true)
    check(!g.checked && !g.acknowledged && !g.canContinue, "starts unchecked, not acknowledged, continue disabled")
    check(!g.continueClicked(), "continue click while unchecked does nothing")
    check(!g.acknowledged, "still not acknowledged")
    g.setChecked(true)
    check(g.canContinue, "checked enables continue")
    g.setChecked(false)
    check(!g.canContinue, "unchecking disables continue again")
    check(!g.continueClicked() && !g.acknowledged, "continue after uncheck does nothing")
    g.setChecked(true)
    check(g.continueClicked() && g.acknowledged, "continue after check acknowledges")
    check(g.mayStartTransfer(noticeNeeded = true), "acknowledged may start when notice needed")
    g.reset()
    check(!g.checked && !g.acknowledged, "reset (back/end of transfer) leaves no acknowledged state")
    check(!g.mayStartTransfer(noticeNeeded = true), "after reset a transfer may not start")
    // Acknowledgement alone does not exist without the checkbox.
    g.setChecked(true); g.continueClicked(); g.setChecked(false)
    check(g.checked, "checkbox cannot be toggled after acknowledgement")
    // Back after continue clears everything.
    g.reset()
    check(!g.acknowledged && !g.checked, "back after continue clears the state")
    // Texts not loaded: nothing works, no transfer.
    val bad = AckGate(textsLoaded = false)
    bad.setChecked(true)
    check(!bad.canContinue && !bad.continueClicked(), "unloaded texts: continue never enabled")
    check(!bad.mayStartTransfer(noticeNeeded = false) && !bad.mayStartTransfer(noticeNeeded = true), "unloaded texts: no transfer")
    // Not needed notice (only with a stable identity in the future) never needs acknowledgement, but texts must load.
    g = AckGate(true)
    check(g.mayStartTransfer(noticeNeeded = false), "notice not needed: may start")
}

fun main() {
    testNeedsNotice(); testRecord(); testTexts(); testGate()
    println("RiskAck tests: $checks checks, $failures failures")
    if (failures != 0) exitProcess(1)
}
