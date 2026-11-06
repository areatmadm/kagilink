package com.areatm.kagilink.spike.peripheral

/**
 * Neutral seam for debug-only test automation: the Activity registers itself here and a debug-only
 * receiver (src/debug) may call it. Release builds have no receiver, so nothing ever calls it.
 */
interface TestHookHost {
    /** Starts a session like the Start button. Returns null on success or a short refusal reason. */
    fun testStart(code: ByteArray, notify: Boolean, plan: FaultPlan): String?
}

object TestHookRegistry {
    @Volatile var host: TestHookHost? = null
}
