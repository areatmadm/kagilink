package com.areatm.kagilink.spike.peripheral

/**
 * Test seam for the peripheral spike. The main source set only has this interface and the no-op
 * [NoFaults]; the actual fault-injection code lives in src/debug (FaultFactory.kt) and is NOT
 * compiled into other build types (src/release has a stub that returns [NoFaults]).
 */
interface FaultPlan {
    val name: String
    val label: String
    val armed: Boolean
    val notice: String
    fun silentOnM1(): Boolean
    fun m2(m2: ByteArray): ByteArray
    fun m2DelayMs(): Long
    fun m2ChunkSize(): Int?
    fun mutateM2Chunks(chunks: MutableList<ByteArray>)
    fun dropAfterM3(): Boolean
    fun m4(m4: ByteArray): ByteArray
    fun secretFragment(index: Int, frag: ByteArray): ByteArray
    fun onSessionStart(tag: String, gatt: String, utc: String)
    fun onSessionEnd(outcome: String, utc: String)
    fun autoApprove(): Boolean
}

object NoFaults : FaultPlan {
    override val name = "NONE"
    override val label = "none (normal)"
    override val armed = false
    override val notice = ""
    override fun silentOnM1() = false
    override fun m2(m2: ByteArray) = m2
    override fun m2DelayMs() = 0L
    override fun m2ChunkSize(): Int? = null
    override fun mutateM2Chunks(chunks: MutableList<ByteArray>) {}
    override fun dropAfterM3() = false
    override fun m4(m4: ByteArray) = m4
    override fun secretFragment(index: Int, frag: ByteArray) = frag
    override fun onSessionStart(tag: String, gatt: String, utc: String) {}
    override fun onSessionEnd(outcome: String, utc: String) {}
    override fun autoApprove() = false
}
