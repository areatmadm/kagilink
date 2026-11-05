package com.areatm.kagilink.spike

import kotlin.system.exitProcess

/**
 * Kotlin port of tests/c/consumer.c scenarios over JNI. Synthetic data only.
 * Run (no app install) with app_process: args[0] = absolute path of libkagilink_jni.so.
 * Exit code 0 only if every check passed.
 */
private var checks = 0
private var failures = 0

private fun check(cond: Boolean, what: String) {
    checks++
    if (!cond) {
        failures++
        System.err.println("FAIL: $what")
    }
}

private const val BUF = 16384
private val RX_ID = "k-rx-device".toByteArray()
private val TX_ID = "k-tx-device".toByteArray()
private val PURPOSE = "synthetic-login".toByteArray()
private val CLAIMS = "claims".toByteArray()

private fun be32(b: ByteArray, o: Int): Int =
    ((b[o].toInt() and 0xff) shl 24) or ((b[o + 1].toInt() and 0xff) shl 16) or
        ((b[o + 2].toInt() and 0xff) shl 8) or (b[o + 3].toInt() and 0xff)

private class Flow {
    var rx = 0L; var tx = 0L; var rxCh = 0L; var txCh = 0L
    val code = ByteArray(6)
    val m1 = ByteArray(BUF); val m2 = ByteArray(BUF); val m3 = ByteArray(BUF); val m4 = ByteArray(BUF)
    var m1Len = 0; var m2Len = 0; var m3Len = 0; var m4Len = 0

    fun free() {
        Native.channelFree(txCh); Native.channelFree(rxCh)
        Native.senderFree(tx); Native.receiverFree(rx)
        rx = 0; tx = 0; rxCh = 0; txCh = 0
        Native.secureWipe(code)
    }
}

private fun slice(a: ByteArray, n: Int) = a.copyOf(n)

/** Runs M1..M3 with the real code, or with codeOverride if given. */
private fun flowToM3(f: Flow, codeOverride: ByteArray? = null): Int {
    val h = LongArray(1)
    var st = Native.receiverNew(RX_ID, CLAIMS, h)
    f.rx = h[0]
    if (st != Native.OK) return st
    var len = longArrayOf(f.code.size.toLong())
    st = Native.receiverCode(f.rx, f.code, len)
    if (st != Native.OK) return st
    if (len[0] != 6L) return 5
    len = longArrayOf(BUF.toLong())
    st = Native.receiverBeginAttempt(f.rx, f.m1, len)
    if (st != Native.OK) return st
    f.m1Len = len[0].toInt()
    len = longArrayOf(BUF.toLong())
    st = Native.senderStart(codeOverride ?: f.code, slice(f.m1, f.m1Len), TX_ID, PURPOSE, CLAIMS,
        f.m2, len, h)
    f.tx = h[0]
    if (st != Native.OK) return st
    f.m2Len = len[0].toInt()
    len = longArrayOf(BUF.toLong())
    st = Native.receiverHandleM2(f.rx, slice(f.m2, f.m2Len), f.m3, len)
    f.m3Len = len[0].toInt()
    return st
}

private fun handshake(f: Flow): Boolean {
    if (flowToM3(f) != Native.OK) return false
    val ch = LongArray(1)
    var len = longArrayOf(BUF.toLong())
    val s1 = Native.senderFinish(f.tx, slice(f.m3, f.m3Len), f.m4, len, ch)
    f.txCh = ch[0]; f.m4Len = len[0].toInt()
    check(s1 == Native.OK, "sender_finish = $s1")
    val s2 = Native.receiverFinish(f.rx, slice(f.m4, f.m4Len), ch)
    f.rxCh = ch[0]
    check(s2 == Native.OK, "receiver_finish = $s2")
    return s1 == Native.OK && s2 == Native.OK
}

private fun testAbiAndWipe() {
    check(Native.abiVersion() == Native.EXPECTED_ABI, "abi version")
    val z = byteArrayOf(1, 2, 3, 4)
    Native.secureWipe(z)
    check(z.all { it == 0.toByte() }, "secureWipe zeroes the Java array")
    Native.secureWipe(null) // must not crash
}

private fun testRoundTrip() {
    val f = Flow()
    val secret = "synthetic-secret-0123456789".toByteArray()
    check(handshake(f), "handshake")
    if (failures > 0) { f.free(); return }
    val sealed = ByteArray(BUF)
    var len = longArrayOf(BUF.toLong())
    val count = LongArray(1)
    check(Native.channelSeal(f.txCh, Native.KIND_SECRET, secret, 8, sealed, len, count) ==
        Native.ERR_NOT_APPROVED, "SECRET before approval is refused")
    check(Native.channelApproveOneTransfer(f.txCh) == Native.OK, "approve one transfer")
    check(Native.channelSetReady(f.rxCh, 1) == Native.OK, "receiver ready")
    len = longArrayOf(BUF.toLong())
    check(Native.channelSeal(f.txCh, Native.KIND_SECRET, secret, 8, sealed, len, count) == Native.OK,
        "seal SECRET")
    check(count[0] > 1, "really fragmented (count=${count[0]})")
    val sealedLen = len[0].toInt()
    val plain = ByteArray(BUF)
    var plainLen = longArrayOf(0)
    val info = IntArray(2)
    var off = 0
    for (i in 0 until count[0].toInt()) {
        check(off + 4 <= sealedLen, "length prefix in range")
        val flen = be32(sealed, off); off += 4
        check(off + flen <= sealedLen, "fragment in range")
        plainLen = longArrayOf(BUF.toLong())
        val st = Native.channelOpenFragment(f.rxCh, sealed.copyOfRange(off, off + flen), plain, plainLen, info)
        check(st == Native.OK, "open fragment $i = $st")
        check(info[1] == (if (i + 1 == count[0].toInt()) 1 else 0), "complete flag at $i")
        off += flen
    }
    check(plainLen[0] == secret.size.toLong(), "plaintext length")
    check(plain.copyOf(secret.size).contentEquals(secret), "plaintext matches")
    check(info[0] == Native.KIND_SECRET, "kind is SECRET")
    Native.secureWipe(plain); Native.secureWipe(sealed)
    check(plain.all { it == 0.toByte() }, "plaintext wiped")
    len = longArrayOf(BUF.toLong())
    check(Native.channelSeal(f.txCh, Native.KIND_SECRET, secret, 8, ByteArray(BUF), len, count) ==
        Native.ERR_NOT_APPROVED, "approval is consumed")
    Native.secureWipe(secret)
    f.free()
}

private fun testWrongCode() {
    val f = Flow()
    val h = LongArray(1)
    check(Native.receiverNew(RX_ID, CLAIMS, h) == Native.OK, "receiver new")
    f.rx = h[0]
    var len = longArrayOf(6)
    check(Native.receiverCode(f.rx, f.code, len) == Native.OK, "code")
    val wrong = f.code.copyOf()
    wrong[0] = if (wrong[0] == '0'.code.toByte()) '1'.code.toByte() else '0'.code.toByte()
    len = longArrayOf(BUF.toLong())
    check(Native.receiverBeginAttempt(f.rx, f.m1, len) == Native.OK, "begin attempt")
    f.m1Len = len[0].toInt()
    len = longArrayOf(BUF.toLong())
    check(Native.senderStart(wrong, slice(f.m1, f.m1Len), TX_ID, PURPOSE, CLAIMS, f.m2, len, h) ==
        Native.OK, "sender start with wrong code")
    f.tx = h[0]; f.m2Len = len[0].toInt()
    len = longArrayOf(BUF.toLong())
    check(Native.receiverHandleM2(f.rx, slice(f.m2, f.m2Len), f.m3, len) == Native.OK, "handle m2")
    f.m3Len = len[0].toInt()
    len = longArrayOf(BUF.toLong())
    val ch = LongArray(1)
    val st = Native.senderFinish(f.tx, slice(f.m3, f.m3Len), f.m4, len, ch)
    check(st == Native.ERR_CONFIRMATION_FAILED, "wrong code -> confirmation failed ($st)")
    check(ch[0] == 0L, "no channel on failure")
    len = longArrayOf(BUF.toLong())
    check(Native.senderFinish(f.tx, slice(f.m3, f.m3Len), f.m4, len, ch) != Native.OK,
        "pending state consumed even on failure")

    // D026: only attempts that already sent M3 use the three-failure budget.
    for (i in 0 until 3) {
        val bogus = ByteArray(33); bogus[0] = 0x14
        var s2 = 0L
        if (i > 0) {
            val m1b = ByteArray(BUF); val m2b = ByteArray(BUF); val m3b = ByteArray(BUF)
            var l = longArrayOf(BUF.toLong())
            check(Native.receiverBeginAttempt(f.rx, m1b, l) == Native.OK, "begin attempt $i")
            val m1n = l[0].toInt()
            l = longArrayOf(BUF.toLong())
            val hs = LongArray(1)
            check(Native.senderStart(wrong, slice(m1b, m1n), TX_ID, PURPOSE, CLAIMS, m2b, l, hs) ==
                Native.OK, "sender start $i")
            s2 = hs[0]
            val m2n = l[0].toInt()
            l = longArrayOf(BUF.toLong())
            check(Native.receiverHandleM2(f.rx, slice(m2b, m2n), m3b, l) == Native.OK, "handle m2 $i")
        }
        val c = LongArray(1)
        check(Native.receiverFinish(f.rx, bogus, c) != Native.OK, "bogus M4 rejected ($i)")
        check(c[0] == 0L, "no channel from bogus M4 ($i)")
        Native.senderFree(s2)
    }
    val code = ByteArray(6)
    check(Native.receiverCode(f.rx, code, longArrayOf(6)) == Native.ERR_TOO_MANY_FAILURES,
        "three M3-attempt failures destroy the code")
    Native.secureWipe(wrong)
    f.free()
}

private fun testTamperClosesChannel() {
    val f = Flow()
    val msg = "synthetic-control".toByteArray()
    check(handshake(f), "handshake")
    if (failures > 0) { f.free(); return }
    val sealed = ByteArray(BUF)
    val len = longArrayOf(BUF.toLong())
    val count = LongArray(1)
    check(Native.channelSeal(f.txCh, Native.KIND_CONTROL, msg, 256, sealed, len, count) == Native.OK, "seal CONTROL")
    check(count[0] == 1L, "single fragment")
    val flen = be32(sealed, 0)
    val frag = sealed.copyOfRange(4, 4 + flen)
    frag[flen - 1] = (frag[flen - 1].toInt() xor 1).toByte() // flip a tag bit
    val plain = ByteArray(BUF)
    val info = IntArray(2)
    val st = Native.channelOpenFragment(f.rxCh, frag, plain, longArrayOf(BUF.toLong()), info)
    check(st != Native.OK, "tampered fragment rejected ($st)")
    check(info[1] == 0, "not complete")
    check(Native.channelIsClosed(f.rxCh) == 1, "channel closed after tamper")
    f.free()
}

private fun testNullAndBuffers() {
    val h = LongArray(1)
    // The shim turns a null Java array into (NULL, 0) and so cannot produce a NULL pointer with a
    // non-zero length (the consumer.c case kl_receiver_new(NULL, 1, ...)); that variant is not
    // reachable from Kotlin by design. Library-side NULL handling is exercised via the other args.
    check(Native.receiverNew(RX_ID, CLAIMS, null) == Native.ERR_NULL, "null out handle -> KL_ERR_NULL")
    check(Native.receiverNew(RX_ID, CLAIMS, h) == Native.OK, "receiver new")
    val r = h[0]
    val small = ByteArray(2)
    val n = longArrayOf(small.size.toLong())
    check(Native.receiverCode(r, small, n) == Native.ERR_BUFFER, "small buffer -> KL_ERR_BUFFER")
    check(n[0] == 6L, "required size reported")
    check(small.all { it == 0.toByte() }, "nothing written on KL_ERR_BUFFER")
    val q = longArrayOf(0)
    check(Native.receiverCode(r, null, q) == Native.ERR_BUFFER && q[0] == 6L, "size query with null output")
    val code = ByteArray(6)
    val n2 = longArrayOf(code.size.toLong())
    check(Native.receiverCode(r, code, n2) == Native.OK && n2[0] == 6L, "code with exact buffer")
    check(code.all { it in '0'.code.toByte()..'9'.code.toByte() }, "six ASCII digits")
    Native.secureWipe(code)
    check(Native.receiverCode(0L, code, longArrayOf(6)) == Native.ERR_NULL, "NULL receiver handle")
    Native.receiverFree(r)
    Native.receiverFree(0L); Native.senderFree(0L); Native.channelFree(0L) // no-ops
    check(Native.channelIsClosed(0L) == 1, "NULL channel reads as closed")
}

/** Registry-token handles (library PR #30): stale, double-freed and garbage values fail closed. */
private fun testHandleLifecycle() {
    val h = LongArray(1)
    check(Native.receiverNew(RX_ID, CLAIMS, h) == Native.OK, "receiver new (lifecycle)")
    val r = h[0]
    Native.receiverFree(r)
    val buf = ByteArray(6)
    check(Native.receiverCode(r, buf, longArrayOf(6)) == Native.ERR_NULL, "use after free -> KL_ERR_NULL")
    Native.receiverFree(r) // double free through the raw binding: must not crash
    check(Native.receiverCode(r, buf, longArrayOf(6)) == Native.ERR_NULL, "still KL_ERR_NULL after double free")
    check(Native.receiverCode(0x7f12345678L, buf, longArrayOf(6)) == Native.ERR_NULL, "garbage handle value -> KL_ERR_NULL")
    check(Native.channelIsClosed(r) == 1, "stale handle reads as closed")
    // NativeHandle: a second close and close of an emptied handle never call free again.
    check(Native.receiverNew(RX_ID, CLAIMS, h) == Native.OK, "receiver new (NativeHandle)")
    var frees = 0
    val nh = NativeHandle(h[0]) { v -> frees++; Native.receiverFree(v) }
    check(nh.isOpen && nh.get() == h[0], "NativeHandle open")
    nh.close(); nh.close()
    check(frees == 1 && !nh.isOpen && nh.get() == 0L, "NativeHandle double close frees exactly once")
    check(Native.receiverCode(h[0], buf, longArrayOf(6)) == Native.ERR_NULL, "NativeHandle target is gone")
    val zero = NativeHandle(0L) { frees++ }
    zero.close()
    check(frees == 1, "NativeHandle(0) close does not call free")
}

fun main(args: Array<String>) {
    if (args.isEmpty()) {
        System.err.println("usage: SpikeMain <absolute path of libkagilink_jni.so>")
        exitProcess(64)
    }
    Native.load(args[0])
    if (Native.abiVersion() != Native.EXPECTED_ABI) {
        System.err.println("ABI mismatch: library ${Native.abiVersion()}, binding ${Native.EXPECTED_ABI}")
        exitProcess(2)
    }
    testAbiAndWipe()
    testRoundTrip()
    testWrongCode()
    testTamperClosesChannel()
    testNullAndBuffers()
    testHandleLifecycle()
    codecCrossCheck(::check)
    println("Kotlin/JNI consumer: $checks checks, $failures failures")
    exitProcess(if (failures == 0) 0 else 1)
}
