package com.areatm.kagilink.spike.peripheral

/** Non-debug build types: no fault injection exists. */
object FaultFactory {
    val labels: List<String> = emptyList()
    const val heading = ""
    const val keepLabel = ""
    fun create(index: Int): FaultPlan = NoFaults
}
