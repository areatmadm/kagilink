package com.areatm.kagilink.spike.peripheral

import android.util.Log
import java.security.SecureRandom

/**
 * Debug-build-only fault injection (compiled out of every other build type). Synthetic data only.
 * Default is none; the Activity resets the selection after each run unless the user keeps it.
 * See apps/android/peripheral/FAULT_INJECTION.md.
 */
object FaultFactory {
    enum class Kind(val label: String) {
        NONE("none (normal)"),
        GARBAGE_M2("a: garbage bytes instead of M2"),
        TAMPER_M4("b: M4 with one byte flipped"),
        DROP_AFTER_M3("c: disconnect right after M3"),
        CHUNK_DUP("d1: duplicate a chunk of M2"),
        CHUNK_DROP("d2: drop a chunk of M2"),
        CHUNK_SWAP("d3: swap two chunks of M2"),
        NO_RESPONSE("e1: never answer M1"),
        DELAY_M2("e2: answer M1 after 6 s (must still succeed)"),
        DELAY_M2_LONG("e3: answer M1 after 11 s (receiver times out)"),
        TAMPER_SECRET("f: SECRET fragment with one byte flipped"),
    }

    private const val TAG = "KLV1"
    private const val DELAY_MS = 6_000L
    private const val DELAY_LONG_MS = 11_000L
    private const val CHUNK = 32 // small chunks so that M2 has >= 3 chunks for the chunk faults

    val labels: List<String> = Kind.values().map { it.label }
    const val heading = "TEST ONLY fault injection (resets to none after each run unless kept):"
    const val keepLabel = "keep fault selected after a run (for repeated attempts)"
    fun create(index: Int): FaultPlan = if (index <= 0) NoFaults else Impl(Kind.values()[index], false)

    /** adb test hook: codes none,a,b,c,d1,d2,d3,e1,e2,e3,f. Sessions log TEST started/ended. */
    fun createTest(code: String): FaultPlan? {
        val kind = when (code) {
            "none" -> Kind.NONE; "a" -> Kind.GARBAGE_M2; "b" -> Kind.TAMPER_M4; "c" -> Kind.DROP_AFTER_M3
            "d1" -> Kind.CHUNK_DUP; "d2" -> Kind.CHUNK_DROP; "d3" -> Kind.CHUNK_SWAP
            "e1" -> Kind.NO_RESPONSE; "e2" -> Kind.DELAY_M2; "e3" -> Kind.DELAY_M2_LONG
            "f" -> Kind.TAMPER_SECRET; else -> return null
        }
        return Impl(kind, true)
    }

    private class Impl(private val k: Kind, private val viaHook: Boolean) : FaultPlan {
        override val name = k.name
        override val label = k.label
        override val armed = k != Kind.NONE
        override val notice = "FAULT INJECTION ARMED: ${k.label}"
        private fun injected(what: String) = Log.i(TAG, "fault injected: $what")

        override fun silentOnM1() = (k == Kind.NO_RESPONSE).also { if (it) injected("M1 received, no M2 will be sent") }

        override fun m2(m2: ByteArray): ByteArray {
            if (k != Kind.GARBAGE_M2) return m2
            SecureRandom().nextBytes(m2)
            injected("M2 replaced by ${m2.size} random bytes")
            return m2
        }

        override fun m2DelayMs() = when (k) { Kind.DELAY_M2 -> DELAY_MS; Kind.DELAY_M2_LONG -> DELAY_LONG_MS; else -> 0L }

        override fun m2ChunkSize(): Int? = when (k) {
            Kind.CHUNK_DUP, Kind.CHUNK_DROP, Kind.CHUNK_SWAP -> CHUNK
            else -> null
        }

        override fun mutateM2Chunks(chunks: MutableList<ByteArray>) {
            if (m2ChunkSize() == null) return
            if (chunks.size < 3) { Log.w(TAG, "fault ${k.name} not injected: only ${chunks.size} chunks"); return }
            when (k) {
                Kind.CHUNK_DUP -> chunks.add(2, chunks[1].copyOf())
                Kind.CHUNK_DROP -> chunks.removeAt(1)
                else -> { val t = chunks[1]; chunks[1] = chunks[2]; chunks[2] = t }
            }
            injected("${k.name} (M2 in ${chunks.size} chunks of <=$CHUNK)")
        }

        override fun dropAfterM3() = (k == Kind.DROP_AFTER_M3).also { if (it) injected("dropping the connection right after M3") }

        override fun m4(m4: ByteArray): ByteArray {
            if (k != Kind.TAMPER_M4) return m4
            m4[m4.size / 2] = (m4[m4.size / 2].toInt() xor 0x01).toByte()
            injected("M4 one byte flipped")
            return m4
        }

        override fun onSessionStart(tag: String, gatt: String, utc: String) {
            if (viaHook) Log.i(TAG, "TEST started tag=KL-$tag fault=${k.name} gatt=$gatt utc=$utc")
            if (viaHook) Log.i(TAG, "TEST auto-approve=yes")
        }

        override fun onSessionEnd(outcome: String, utc: String) {
            if (viaHook) Log.i(TAG, "TEST ended fault=${k.name} outcome=$outcome utc=$utc")
        }

        override fun autoApprove() = viaHook // test automation only: sessions started by TEST_START; manual Start still needs Approve

        override fun secretFragment(index: Int, frag: ByteArray): ByteArray {
            if (k != Kind.TAMPER_SECRET || index != 0) return frag
            frag[frag.size / 2] = (frag[frag.size / 2].toInt() xor 0x01).toByte()
            injected("SECRET fragment $index one byte flipped")
            return frag
        }
    }
}
