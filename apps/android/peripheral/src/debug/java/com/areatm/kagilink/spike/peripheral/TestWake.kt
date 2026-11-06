package com.areatm.kagilink.spike.peripheral

import android.app.Activity

/** Debug builds only: the test screen turns the screen on and shows over the (non-bypassed) lock screen. */
object TestWake {
    fun apply(a: Activity) {
        a.setShowWhenLocked(true)
        a.setTurnScreenOn(true)
    }
}
