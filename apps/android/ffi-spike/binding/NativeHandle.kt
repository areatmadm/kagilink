package com.areatm.kagilink.spike

import java.util.concurrent.atomic.AtomicLong

/**
 * Owns one native handle and frees it exactly once (header rule: a free must be exclusive, never
 * twice, never concurrent with another call on the same handle). [close] swaps the value to 0 with
 * getAndSet and frees only a non-zero previous value, so a second close is a no-op. The CALLER must
 * still ensure no other thread is inside a native call on this handle while closing; reading
 * [get] after close yields 0, which the library treats as NULL (KL_ERR_NULL), never a stale pointer.
 */
class NativeHandle(initial: Long, private val free: (Long) -> Unit) : AutoCloseable {
    private val value = AtomicLong(initial)

    fun get(): Long = value.get()

    val isOpen: Boolean get() = value.get() != 0L

    override fun close() {
        val previous = value.getAndSet(0)
        if (previous != 0L) free(previous)
    }
}
