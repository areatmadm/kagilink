package com.areatm.kagilink.spike

import java.util.Random

/**
 * Cross-check of the library BLE codec (Rust, C ABI v2, via JNI [Framing]) against the independent
 * Kotlin implementation [FramingReference]: same input -> same chunks, same reassembly outcome for
 * valid, malformed and randomly mutated sequences. Synthetic data only. Reports through [check].
 */
fun codecCrossCheck(check: (Boolean, String) -> Unit) {
    chunkingEquality(check)
    invalidChunkingInputs(check)
    reassemblyEquality(check)
    malformedSequences(check)
    randomMutations(check)
    ackEquality(check)
}

private fun msg(len: Int): ByteArray = ByteArray(len) { ((it * 31 + len) and 0xff).toByte() }

private fun hex(b: ByteArray) = b.joinToString("") { "%02x".format(it) }

private fun chunksJni(mtype: Int, m: ByteArray, max: Int) = Framing.chunkMessage(mtype, m, max)
private fun chunksRef(mtype: Int, m: ByteArray, max: Int) = FramingReference.chunkMessage(mtype, m, max)

private fun chunkingEquality(check: (Boolean, String) -> Unit) {
    for (max in listOf(6, 7, 20, 23, 64, 200, 244, 512)) {
        var mismatches = 0
        var cases = 0
        for (len in 1..512) {
            for (mtype in intArrayOf(Native.MTYPE_HANDSHAKE, Native.MTYPE_CHANNEL)) {
                cases++
                val a = chunksJni(mtype, msg(len), max)
                val b = chunksRef(mtype, msg(len), max)
                if (a.size != b.size || a.indices.any { !a[it].contentEquals(b[it]) }) mismatches++
            }
        }
        check(mismatches == 0, "chunking equal for max_chunk=$max over $cases cases (mismatches=$mismatches)")
    }
}

private fun invalidChunkingInputs(check: (Boolean, String) -> Unit) {
    data class Bad(val name: String, val mtype: Int, val len: Int, val max: Int)
    val cases = listOf(
        Bad("mtype 0", 0, 10, 64), Bad("mtype 3", 3, 10, 64), Bad("mtype 255", 255, 10, 64),
        Bad("empty message", 1, 0, 64), Bad("513 bytes", 1, 513, 64),
        Bad("chunk 5", 1, 10, 5), Bad("chunk 513", 1, 10, 513), Bad("chunk 0", 1, 10, 0),
    )
    for (c in cases) {
        val jni = try { chunksJni(c.mtype, msg(c.len), c.max); "ok" } catch (e: Framing.FramingException) { "limits:${e.status}" }
        val ref = try { chunksRef(c.mtype, msg(c.len), c.max); "ok" } catch (e: IllegalArgumentException) { "rejected" }
        check(jni == "limits:${Native.ERR_LIMITS}" && ref == "rejected", "both reject ${c.name} (jni=$jni ref=$ref)")
    }
}

private fun outcomeOf(m: Any?): String = when (m) {
    null -> "none"
    is Framing.Message -> "msg:${m.mtype}:${m.data.size}:${m.data.contentHashCode()}"
    is FramingReference.Message -> "msg:${m.mtype}:${m.data.size}:${m.data.contentHashCode()}"
    else -> "?"
}

private fun runJni(chunks: List<ByteArray>): List<String> {
    val out = ArrayList<String>()
    Framing.Reassembler().use { r ->
        for (c in chunks) {
            out.add(try { outcomeOf(r.push(c)) } catch (e: Framing.FramingException) { "err" })
        }
        out.add("dead=${r.isDead}")
    }
    return out
}

private fun runRef(chunks: List<ByteArray>): List<String> {
    val out = ArrayList<String>()
    val r = FramingReference.Reassembler()
    for (c in chunks) {
        out.add(try { outcomeOf(r.push(c)) } catch (e: FramingReference.FramingException) { "err" })
    }
    out.add("dead=${r.isDead}")
    return out
}

private fun reassemblyEquality(check: (Boolean, String) -> Unit) {
    var bad = 0
    var cases = 0
    for (len in intArrayOf(1, 2, 3, 4, 5, 100, 244, 500, 512)) {
        for (max in intArrayOf(6, 20, 23, 244, 512)) {
            for (mtype in intArrayOf(1, 2)) {
                cases++
                val chunks = chunksJni(mtype, msg(len), max)
                val a = runJni(chunks)
                val b = runRef(chunks)
                val want = outcomeOf(Framing.Message(mtype, msg(len)))
                if (a != b || a[a.size - 2] != want || a.last() != "dead=false") bad++
            }
        }
    }
    check(bad == 0, "valid sequences reassemble identically over $cases cases (bad=$bad)")
}

private fun same(check: (Boolean, String) -> Unit, name: String, chunks: List<ByteArray>, expectDead: Boolean?) {
    val a = runJni(chunks)
    val b = runRef(chunks)
    check(a == b, "$name: library and reference agree ($a vs $b)")
    if (expectDead != null) check(a.last() == "dead=$expectDead", "$name: dead=$expectDead")
}

private fun malformedSequences(check: (Boolean, String) -> Unit) {
    val m = msg(200)
    val good = chunksJni(1, m, 64) // 4 chunks
    check(good.size >= 4, "need a multi-chunk message")
    same(check, "duplicate first chunk", listOf(good[0], good[0]) + good.drop(1), true)
    same(check, "gap (skip second)", listOf(good[0]) + good.drop(2), true)
    same(check, "reorder 2/3", listOf(good[0], good[2], good[1]) + good.drop(3), true)
    same(check, "FIRST twice", listOf(good[0], good[0].copyOf().also { it[1] = 1 }), true)
    same(check, "missing FIRST", good.drop(1), true)
    same(check, "reserved bit 3", listOf(good[0].copyOf().also { it[0] = (it[0].toInt() or 0x08).toByte() }) + good.drop(1), true)
    same(check, "ACK flag on data chunk", listOf(good[0].copyOf().also { it[0] = (it[0].toInt() or 0x04).toByte() }) + good.drop(1), true)
    same(check, "unknown mtype", listOf(good[0].copyOf().also { it[2] = 9 }) + good.drop(1), true)
    same(check, "zero length", listOf(byteArrayOf(3, 0, 1, 0, 0, 7)), true)
    same(check, "oversize length 513", listOf(byteArrayOf(3, 0, 1, 0x02, 0x01, 7)), true)
    same(check, "LAST too early", listOf(good[0], good[1].copyOf().also { it[0] = (it[0].toInt() or 0x02).toByte() }), true)
    same(check, "length reached without LAST", listOf(byteArrayOf(1, 0, 1, 0, 1, 7)), true)
    same(check, "chunk of 2 bytes", listOf(byteArrayOf(1, 0)), true)
    same(check, "chunk of 513 bytes", listOf(ByteArray(513).also { it[0] = 1 }), true)
    same(check, "data after error stays rejected", listOf(good[1]) + good, true)
    same(check, "two messages back to back", good + good, false)
}

private fun randomMutations(check: (Boolean, String) -> Unit) {
    val rnd = Random(20261106L)
    var disagreements = 0
    val iterations = 3000
    repeat(iterations) { n ->
        val len = 1 + rnd.nextInt(512)
        val max = 6 + rnd.nextInt(60)
        val chunks = chunksJni(1 + rnd.nextInt(2), msg(len), max).map { it.copyOf() }.toMutableList()
        when (rnd.nextInt(6)) {
            0 -> { val c = chunks[rnd.nextInt(chunks.size)]; c[rnd.nextInt(c.size)] = (c[rnd.nextInt(c.size)].toInt() xor (1 + rnd.nextInt(255))).toByte() }
            1 -> chunks.removeAt(rnd.nextInt(chunks.size))
            2 -> { val i = rnd.nextInt(chunks.size); chunks.add(i, chunks[i].copyOf()) }
            3 -> if (chunks.size > 1) { val i = rnd.nextInt(chunks.size - 1); val t = chunks[i]; chunks[i] = chunks[i + 1]; chunks[i + 1] = t }
            4 -> { val i = rnd.nextInt(chunks.size); chunks[i] = chunks[i].copyOf(rnd.nextInt(chunks[i].size + 1)) }
            else -> { val c = chunks[rnd.nextInt(chunks.size)]; c[rnd.nextInt(2)] = rnd.nextInt(256).toByte() }
        }
        if (runJni(chunks) != runRef(chunks)) disagreements++
        if (n == iterations - 1) check(disagreements == 0, "random mutations: library == reference over $iterations cases (disagreements=$disagreements)")
    }
}

private fun ackEquality(check: (Boolean, String) -> Unit) {
    var bad = 0
    for (seq in 0..255) if (!Framing.ackChunk(seq).contentEquals(FramingReference.ackChunk(seq))) bad++
    check(bad == 0, "ACK chunks equal for seq 0..255 (bad=$bad)")
    val samples = listOf(
        byteArrayOf(), byteArrayOf(4), byteArrayOf(4, 7), byteArrayOf(4, 7, 8), byteArrayOf(0, 0), byteArrayOf(5, 7),
        byteArrayOf(1, 2), byteArrayOf(0xff.toByte(), 0xff.toByte()), Framing.chunkMessage(1, msg(10), 20)[0],
    )
    var diff = 0
    for (s in samples) if (Framing.parseAck(s) != FramingReference.parseAck(s)) diff++
    check(diff == 0, "parse_ack agrees on ${samples.size} samples (diff=$diff)")
    check(Framing.parseAck(Framing.ackChunk(200)) == 200, "parse_ack round trip")
}
