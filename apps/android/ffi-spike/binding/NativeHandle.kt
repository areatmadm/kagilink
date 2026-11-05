package com.areatm.kagilink.spike

import java.util.concurrent.atomic.AtomicLong

/**
 * Owns one native handle and frees it exactly once. [close] swaps the value to 0 with getAndSet
 * and frees only a non-zero previous value, so a second close is a no-op. The library handle is a
 * registry token: a free racing another call, or a use after close, returns an error (KL_ERR_NULL)
 * instead of touching freed memory; reading [get] after close yields 0, which is NULL. Keep the
 * single-owner pattern anyway.
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
