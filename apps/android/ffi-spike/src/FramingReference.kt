package com.areatm.kagilink.spike

import java.io.ByteArrayOutputStream

/**
 * Independent Kotlin implementation of docs/BLE_GATT_SPEC.md section 5 (chunk framing v1). Written when the C ABI had no
 * chunk functions yet, so this mirrors core/kagilink-core/src/ble_frame.rs. Pure, no Android types.
 * TEST REFERENCE ONLY: kept so the library codec (ABI v2, via JNI) can be cross-checked against an
 * independent implementation; the app uses [Framing] (JNI) instead.
 * Chunk = flags:u8 | seq:u8 | body. FIRST body starts with mtype:u8 | total_len:u16 BE.
 */
object FramingReference {
    const val MAX_BLE_MESSAGE = 512
    const val MIN_CHUNK = 6
    const val MAX_CHUNK = 512
    const val MTYPE_HANDSHAKE = 1
    const val MTYPE_CHANNEL = 2
    private const val FIRST = 1
    private const val LAST = 2
    private const val ACK = 4
    private const val HEADER = 2
    private const val PREFIX = 3

    private fun validMtype(m: Int) = m == MTYPE_HANDSHAKE || m == MTYPE_CHANNEL

    fun chunkMessage(mtype: Int, message: ByteArray, maxChunk: Int): List<ByteArray> {
        require(validMtype(mtype) && message.isNotEmpty() && message.size <= MAX_BLE_MESSAGE)
        require(maxChunk in MIN_CHUNK..MAX_CHUNK)
        val stream = ByteArray(PREFIX + message.size)
        stream[0] = mtype.toByte()
        stream[1] = (message.size ushr 8).toByte()
        stream[2] = message.size.toByte()
        message.copyInto(stream, PREFIX)
        val room = maxChunk - HEADER
        val count = (stream.size + room - 1) / room
        require(count <= 256)
        return List(count) { i ->
            val from = i * room
            val to = minOf(from + room, stream.size)
            var flags = 0
            if (i == 0) flags = flags or FIRST
            if (i == count - 1) flags = flags or LAST
            val chunk = ByteArray(HEADER + (to - from))
            chunk[0] = flags.toByte()
            chunk[1] = i.toByte()
            stream.copyInto(chunk, HEADER, from, to)
            chunk
        }
    }

    fun ackChunk(seq: Int) = byteArrayOf(ACK.toByte(), seq.toByte())

    /** The acknowledged sequence number if [chunk] is exactly an ACK chunk. */
    fun parseAck(chunk: ByteArray): Int? =
        if (chunk.size == 2 && chunk[0].toInt() == ACK) chunk[1].toInt() and 0xff else null

    class Message(val mtype: Int, val data: ByteArray)

    class FramingException(msg: String) : Exception(msg)

    /** Strict in-order reassembler for one direction; any error is final (dead). */
    class Reassembler {
        private var dead = false
        private var mtype = 0
        private var total = 0
        private var nextSeq = 0
        private var receiving = false
        private val data = ByteArrayOutputStream()

        val isDead get() = dead

        private fun die(m: String): Nothing {
            dead = true
            data.reset()
            throw FramingException(m)
        }

        /** Returns the finished message when its last chunk arrives, otherwise null. */
        fun push(chunk: ByteArray): Message? {
            if (dead) throw FramingException("closed")
            if (chunk.size <= HEADER || chunk.size > MAX_CHUNK) die("malformed")
            val flags = chunk[0].toInt() and 0xff
            val seq = chunk[1].toInt() and 0xff
            if (flags and (FIRST or LAST).inv() != 0) die("reserved/ack flags")
            val first = flags and FIRST != 0
            val last = flags and LAST != 0
            var from = HEADER
            if (!receiving) {
                if (!first || seq != 0 || chunk.size - HEADER < PREFIX) die("bad first chunk")
                mtype = chunk[2].toInt() and 0xff
                total = ((chunk[3].toInt() and 0xff) shl 8) or (chunk[4].toInt() and 0xff)
                if (!validMtype(mtype)) die("unknown mtype")
                if (total == 0 || total > MAX_BLE_MESSAGE) die("bad length")
                from = HEADER + PREFIX
                data.reset()
                nextSeq = 0
                receiving = true
            } else if (first || seq != nextSeq) {
                die("out of order")
            }
            val n = chunk.size - from
            if (data.size() + n > total) die("overflow")
            data.write(chunk, from, n)
            if (data.size() == total) {
                if (!last) die("missing LAST")
                receiving = false
                val out = data.toByteArray()
                data.reset()
                return Message(mtype, out)
            }
            if (last) die("LAST too early")
            if (nextSeq == 255) die("seq overflow")
            nextSeq++
            return null
        }
    }
}
