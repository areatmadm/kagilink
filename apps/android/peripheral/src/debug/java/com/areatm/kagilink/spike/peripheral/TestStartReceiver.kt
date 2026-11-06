package com.areatm.kagilink.spike.peripheral

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log

/**
 * Debug-only adb test hook (synthetic data only). Protected by android.permission.DUMP, which the
 * adb shell holds and ordinary apps cannot get. Extras: code (6 digits, never logged), fault
 * (none,a,b,c,d1,d2,d3,e1,e2,e3,f), gatt (indicate|notify). The Activity must be open (a backgrounded
 * session fails closed). A new TEST_START ends a running session and starts a fresh one (new tag).
 */
class TestStartReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != ACTION) return
        fun refuse(why: String) { Log.w(TAG, "TEST start refused: $why") }
        val digits = intent.getStringExtra("code") ?: return refuse("no code")
        if (digits.length != 6 || !digits.all { it in '0'..'9' }) return refuse("code must be exactly 6 digits")
        val fault = intent.getStringExtra("fault") ?: "none"
        val plan = FaultFactory.createTest(fault) ?: return refuse("unknown fault '$fault'")
        val gatt = intent.getStringExtra("gatt") ?: "indicate"
        if (gatt != "indicate" && gatt != "notify") return refuse("gatt must be indicate or notify")
        val host = TestHookRegistry.host ?: return refuse("app screen is not open")
        host.testStart(digits.toByteArray(Charsets.US_ASCII), gatt == "notify", plan)?.let { return refuse(it) }
    }

    companion object {
        const val ACTION = "com.areatm.kagilink.spike.peripheral.TEST_START"
        private const val TAG = "KLV1"
    }
}
