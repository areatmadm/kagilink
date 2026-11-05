package com.areatm.kagilink.spike

/**
 * BLE chunk framing v1 (docs/BLE_GATT_SPEC.md section 5) through the shared Rust codec
 * (C ABI v2, kl_frame_chunk / kl_reassembler_* / kl_ack_chunk / kl_parse_ack). No codec logic lives
 * here: only buffer sizing and error mapping. Synthetic data only.
 */
object Framing {
    const val MAX_BLE_MESSAGE = 512
    const val MIN_CHUNK = 6
    const val MAX_CHUNK = 512
    const val MTYPE_HANDSHAKE = Native.MTYPE_HANDSHAKE
    const val MTYPE_CHANNEL = Native.MTYPE_CHANNEL

    class Message(val mtype: Int, val data: ByteArray)

    class FramingException(val status: Int, msg: String) : Exception(msg)

    /** Largest framed output: 512 data + 3 prefix, 4 bytes length prefix per chunk plus chunk headers. */
    private const val FRAME_BUF = 16 * 1024

    /** Throws [FramingException] with the library status (KL_ERR_LIMITS for a bad mtype/length/size). */
    fun chunkMessage(mtype: Int, message: ByteArray, maxChunk: Int): List<ByteArray> {
        val out = ByteArray(FRAME_BUF)
        val len = longArrayOf(out.size.toLong())
        val count = LongArray(1)
        val st = Native.frameChunk(mtype, message, maxChunk.toLong(), out, len, count)
        if (st != Native.OK) throw FramingException(st, "kl_frame_chunk status $st")
        val chunks = ArrayList<ByteArray>(count[0].toInt())
        var off = 0
        val total = len[0].toInt()
        for (i in 0 until count[0].toInt()) {
            if (off + 4 > total) throw FramingException(-1, "truncated frame output")
            val n = ((out[off].toInt() and 0xff) shl 24) or ((out[off + 1].toInt() and 0xff) shl 16) or
                ((out[off + 2].toInt() and 0xff) shl 8) or (out[off + 3].toInt() and 0xff)
            off += 4
            if (n < 0 || off + n > total) throw FramingException(-1, "bad chunk length")
            chunks.add(out.copyOfRange(off, off + n))
            off += n
        }
        return chunks
    }

    fun ackChunk(seq: Int): ByteArray {
        val out = ByteArray(2)
        val len = longArrayOf(2)
        val st = Native.ackChunk(seq, out, len)
        if (st != Native.OK || len[0] != 2L) throw FramingException(st, "kl_ack_chunk status $st")
        return out
    }

    /** The acknowledged sequence number if [chunk] is exactly an ACK chunk, else null. */
    fun parseAck(chunk: ByteArray): Int? {
        val info = IntArray(2)
        val st = Native.parseAck(chunk, info)
        if (st != Native.OK) throw FramingException(st, "kl_parse_ack status $st")
        return if (info[0] == 1) info[1] else null
    }

    /** Strict in-order reassembler for one direction; any codec error is final. Close when done. */
    class Reassembler : AutoCloseable {
        private val handle: NativeHandle

        init {
            val h = LongArray(1)
            val st = Native.reassemblerNew(h)
            if (st != Native.OK) throw FramingException(st, "kl_reassembler_new status $st")
            handle = NativeHandle(h[0], Native::reassemblerFree)
        }

        val isDead: Boolean get() = Native.reassemblerIsDead(handle.get()) == 1

        /** Returns the finished message when its last chunk arrives, otherwise null. */
        fun push(chunk: ByteArray): Message? {
            val complete = IntArray(1)
            val st = Native.reassemblerPush(handle.get(), chunk, complete)
            if (st != Native.OK) throw FramingException(st, "kl_reassembler_push status $st")
            if (complete[0] != 1) return null
            val out = ByteArray(MAX_BLE_MESSAGE)
            val len = longArrayOf(out.size.toLong())
            val mtype = IntArray(1)
            val t = Native.reassemblerTake(handle.get(), mtype, out, len)
            if (t != Native.OK) { Native.secureWipe(out); throw FramingException(t, "kl_reassembler_take status $t") }
            val data = out.copyOf(len[0].toInt())
            Native.secureWipe(out)
            return Message(mtype[0], data)
        }

        override fun close() = handle.close()
    }
}
