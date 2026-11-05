package com.areatm.kagilink.spike

/**
 * Raw 1:1 binding of core/kagilink-ffi/include/kagilink.h through jni/kagilink_jni.c.
 * Binding spike (D021), synthetic data only. Handles are opaque Longs (0 = NULL). Secrets travel
 * as ByteArray, never String; callers must wipe them with [secureWipe] after use.
 * Array-length conventions: `outLen[0]` is the capacity offered on entry and the written/required
 * length on return; a null output array with outLen[0] = 0 is a size query. outLen[0] is written
 * only meaningfully on OK (bytes written) and ERR_BUFFER (bytes required); on any other status it
 * is set to 0 and the output array is untouched.
 *
 * Handle ownership: a handle is an opaque library token (not an address), so a stale, doubled or
 * racing free is an error code or a no-op, never memory corruption (docs/FFI_DESIGN.md, "Handle
 * registry"). Still wrap each handle in an owner that releases it exactly once, for example
 * `val h = AtomicLong(raw)` and `val prev = h.getAndSet(0); if (prev != 0L) free(prev)`.
 */
object Native {
    const val EXPECTED_ABI = 2

    const val KIND_CONTROL = 1
    const val KIND_SECRET = 2
    const val KIND_RESULT = 3

    const val OK = 0
    const val ERR_TOO_MANY_FAILURES = 3
    const val ERR_CONFIRMATION_FAILED = 9
    const val ERR_NOT_APPROVED = 15
    const val ERR_NULL = 100
    const val ERR_BUFFER = 101

    @JvmStatic fun load(absolutePath: String) = System.load(absolutePath)
    /** For an app that packages libkagilink_jni.so in its APK (jniLibs). */
    @JvmStatic fun loadFromApk() = System.loadLibrary("kagilink_jni")

    @JvmStatic external fun abiVersion(): Int
    @JvmStatic external fun secureWipe(buf: ByteArray?)

    @JvmStatic external fun receiverNew(id: ByteArray?, claims: ByteArray?, outHandle: LongArray?): Int
    @JvmStatic external fun receiverCode(h: Long, out: ByteArray?, outLen: LongArray): Int
    @JvmStatic external fun receiverBeginAttempt(h: Long, out: ByteArray?, outLen: LongArray): Int
    @JvmStatic external fun receiverHandleM2(h: Long, m2: ByteArray?, out: ByteArray?, outLen: LongArray): Int
    @JvmStatic external fun receiverFinish(h: Long, m4: ByteArray?, outChannel: LongArray?): Int
    @JvmStatic external fun receiverFree(h: Long)

    @JvmStatic external fun senderStart(
        code: ByteArray?, m1: ByteArray?, deviceId: ByteArray?, purpose: ByteArray?,
        claims: ByteArray?, out: ByteArray?, outLen: LongArray, outSender: LongArray?,
    ): Int
    @JvmStatic external fun senderFinish(
        h: Long, m3: ByteArray?, out: ByteArray?, outLen: LongArray, outChannel: LongArray?,
    ): Int
    @JvmStatic external fun senderFree(h: Long)

    @JvmStatic external fun channelApproveOneTransfer(h: Long): Int
    @JvmStatic external fun channelSetReady(h: Long, ready: Int): Int
    @JvmStatic external fun channelSeal(
        h: Long, kind: Int, plain: ByteArray?, maxFragmentPayload: Long,
        out: ByteArray?, outLen: LongArray, outCount: LongArray?,
    ): Int
    /** outInfo[0] = kind, outInfo[1] = 1 only when a whole authenticated message is in outPlain. */
    @JvmStatic external fun channelOpenFragment(
        h: Long, fragment: ByteArray?, outPlain: ByteArray?, outLen: LongArray, outInfo: IntArray,
    ): Int
    @JvmStatic external fun channelClose(h: Long): Int
    @JvmStatic external fun channelIsClosed(h: Long): Int
    @JvmStatic external fun channelFree(h: Long)

    // ---- BLE chunk codec (ABI v2). Message types: MTYPE_HANDSHAKE = 1, MTYPE_CHANNEL = 2. ----
    const val MTYPE_HANDSHAKE = 1
    const val MTYPE_CHANNEL = 2
    const val ERR_LIMITS = 13

    /** out receives repeated [uint32 BE length][chunk]; outCount[0] the chunk count. */
    @JvmStatic external fun frameChunk(
        mtype: Int, msg: ByteArray?, maxChunk: Long, out: ByteArray?, outLen: LongArray, outCount: LongArray?,
    ): Int
    @JvmStatic external fun reassemblerNew(outHandle: LongArray?): Int
    /** outComplete[0] = 1 only when this chunk completed a message (then call [reassemblerTake]). */
    @JvmStatic external fun reassemblerPush(h: Long, chunk: ByteArray?, outComplete: IntArray): Int
    @JvmStatic external fun reassemblerTake(h: Long, outMtype: IntArray, out: ByteArray?, outLen: LongArray): Int
    @JvmStatic external fun reassemblerIsDead(h: Long): Int
    @JvmStatic external fun reassemblerFree(h: Long)
    @JvmStatic external fun ackChunk(seq: Int, out: ByteArray?, outLen: LongArray): Int
    /** outInfo[0] = 1 if exactly an ACK chunk, outInfo[1] = acknowledged sequence number. */
    @JvmStatic external fun parseAck(chunk: ByteArray?, outInfo: IntArray): Int
}
