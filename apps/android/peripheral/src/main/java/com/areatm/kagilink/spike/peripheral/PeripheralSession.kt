package com.areatm.kagilink.spike.peripheral

import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothGattServer
import android.bluetooth.BluetoothGattServerCallback
import android.bluetooth.BluetoothGattService
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothStatusCodes
import android.bluetooth.le.AdvertiseCallback
import android.bluetooth.le.AdvertiseData
import android.bluetooth.le.AdvertiseSettings
import android.bluetooth.le.BluetoothLeAdvertiser
import android.content.Context
import android.os.Handler
import android.os.HandlerThread
import android.os.ParcelUuid
import android.os.SystemClock
import android.util.Log
import com.areatm.kagilink.spike.Framing
import com.areatm.kagilink.spike.Native
import com.areatm.kagilink.spike.NativeHandle
import java.security.SecureRandom
import java.util.UUID

/**
 * KL-V1 BLE peripheral spike (docs/BLE_GATT_SPEC.md, framing v1). SYNTHETIC DATA ONLY.
 *
 * Phone = Peripheral: advertises the KL-V1 service UUID plus a 4-byte ASCII service-data tag (new
 * random tag per start, scan response), accepts ONE central, requires nothing from BLE pairing,
 * runs M1..M4 through the shared Rust core (JNI) and sends one synthetic SECRET after explicit
 * user approval. No real secrets, no Keystore/StrongBox, no logging of secret material.
 * All protocol work runs on one handler thread; GATT callbacks only post to it.
 */
@SuppressLint("MissingPermission")
class PeripheralSession(private val ctx: Context, private val ui: Listener) {
    interface Listener {
        fun onStatus(text: String)
        fun onTag(tag: String)
        fun onApprovalReady()
        fun onFinished(summary: String)
    }

    companion object {
        const val TAG = "KLV1"
        val SERVICE: UUID = UUID.fromString("3685355e-2903-4e85-bdd6-474ada0838c1")
        val RX: UUID = UUID.fromString("316da2a3-cfc7-4bbf-a736-58c4e7d8d5cf")
        val TX: UUID = UUID.fromString("7c69e9b3-c395-4fc0-87b2-b564d7b8b29e")
        val CCCD: UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")
        const val TAG_ALPHABET = "23456789ABCDEFGHJKMNPQRSTUVWXYZ"
        const val SESSION_MS = 60_000L
        const val CCCD_WAIT_MS = 5_000L
        const val MESSAGE_MS = 5_000L
        const val ACK_MS = 2_000L
        const val FAULT_DELAY_MS = 6_000L
        const val FAULT_DELAY_LONG_MS = 11_000L
        const val FAULT_CHUNK = 32 // small chunks so that M2 has >= 3 chunks for the chunk faults
        const val MAX_CHUNK_CAP = 244
        val SYNTHETIC_SECRET = "synthetic-secret-android-kl-v1".toByteArray()
        val DEVICE_ID = "android-spike".toByteArray()
        val PURPOSE = "synthetic-login".toByteArray()
        val CLAIMS = "mode=spike".toByteArray()
    }

    /**
     * Test-only fault injection (default NONE, honoured only in debuggable builds, reset to NONE
     * after every session). Synthetic data only. Expected receiver reaction: docs/BLE_GATT_SPEC.md
     * and apps/android/peripheral/FAULT_INJECTION.md.
     */
    enum class Fault(val label: String) {
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

    private enum class State { IDLE, ADVERTISING, AWAIT_M1, AWAIT_M3, CHANNEL, DONE }

    private val thread = HandlerThread("kl-v1").also { it.start() }
    private val h = Handler(thread.looper)
    private val manager = ctx.getSystemService(BluetoothManager::class.java)

    private var state = State.IDLE
    private var gatt: BluetoothGattServer? = null
    private var advertiser: BluetoothLeAdvertiser? = null
    private var advCallback: AdvertiseCallback? = null
    private var central: BluetoothDevice? = null
    private var txChar: BluetoothGattCharacteristic? = null
    private var code: ByteArray? = null
    private var useIndicate = true
    private var cccdEnabled = false
    private var mtu = 23
    // Each native handle is freed exactly once through NativeHandle (getAndSet(0)); all calls on
    // them run on the single handler thread, so a free is never concurrent with another call.
    private var sender: NativeHandle? = null
    private var channel: NativeHandle? = null
    private var reassembler = Framing.Reassembler()
    private var sessionTimer: Runnable? = null
    private var finishing = false
    private var messageTimer: Runnable? = null
    private val queue = ArrayDeque<ByteArray>()
    private var inFlight = false
    private var awaitingAck = -1
    private var ackTimer: Runnable? = null
    private var sentAtNs = 0L

    // Measurements (no secrets).
    private var advStartMs = 0L
    private var connectMs = 0L
    private var cccdMs = 0L
    private var sentCallbacks = 0
    private var sentCallbackFailures = 0
    private var latencySumNs = 0L
    private var latencyMaxNs = 0L
    private var chunksSent = 0
    private var acksReceived = 0
    private var mtuHistory = mutableListOf<Int>()
    private var advTag = ""
    private var fault = Fault.NONE

    /** [codeDigits] is wiped by this class once used or on cleanup. */
    fun start(codeDigits: ByteArray, notifyMode: Boolean, requested: Fault = Fault.NONE) = h.post {
        if (state != State.IDLE) { ui.onStatus("already running"); return@post }
        code = codeDigits
        useIndicate = !notifyMode
        val debuggable = (ctx.applicationInfo.flags and android.content.pm.ApplicationInfo.FLAG_DEBUGGABLE) != 0
        fault = if (debuggable) requested else Fault.NONE
        if (fault != Fault.NONE) {
            Log.w(TAG, "FAULT INJECTION ARMED: ${fault.name} (test build, synthetic data)")
            ui.onStatus("FAULT INJECTION ARMED: ${fault.label}")
        }
        val adapter = manager.adapter
        if (adapter == null || !adapter.isEnabled) { finish("Bluetooth is off"); return@post }
        advertiser = adapter.bluetoothLeAdvertiser
        if (advertiser == null) { finish("no BLE advertiser"); return@post }
        val rng = SecureRandom()
        advTag = String(CharArray(4) { TAG_ALPHABET[rng.nextInt(TAG_ALPHABET.length)] })
        ui.onTag(advTag)
        val server = manager.openGattServer(ctx, serverCallback)
        if (server == null) { finish("openGattServer failed"); return@post }
        gatt = server
        val svc = BluetoothGattService(SERVICE, BluetoothGattService.SERVICE_TYPE_PRIMARY)
        svc.addCharacteristic(BluetoothGattCharacteristic(
            RX, BluetoothGattCharacteristic.PROPERTY_WRITE, BluetoothGattCharacteristic.PERMISSION_WRITE))
        val tx = BluetoothGattCharacteristic(
            TX, if (useIndicate) BluetoothGattCharacteristic.PROPERTY_INDICATE
            else BluetoothGattCharacteristic.PROPERTY_NOTIFY, 0)
        tx.addDescriptor(BluetoothGattDescriptor(CCCD,
            BluetoothGattDescriptor.PERMISSION_READ or BluetoothGattDescriptor.PERMISSION_WRITE))
        svc.addCharacteristic(tx)
        txChar = tx
        state = State.ADVERTISING
        val bound = Runnable { finish("60 s session bound reached") }
        sessionTimer = bound
        h.postDelayed(bound, SESSION_MS)
        if (!server.addService(svc)) finish("addService failed")
    }

    fun approveAndSendSecret() = h.post {
        val chH = channel?.get() ?: 0L
        if (state != State.CHANNEL || chH == 0L) { ui.onStatus("not ready"); return@post }
        // The UI button is the explicit user approval (synthetic data only in this spike).
        var st = Native.channelApproveOneTransfer(chH)
        if (st != Native.OK) { finish("approve failed $st"); return@post }
        val sealed = ByteArray(4096)
        val len = longArrayOf(sealed.size.toLong())
        val count = LongArray(1)
        st = Native.channelSeal(chH, Native.KIND_SECRET, SYNTHETIC_SECRET.copyOf(), 200, sealed, len, count)
        if (st != Native.OK) { Native.secureWipe(sealed); finish("seal failed $st"); return@post }
        var off = 0
        for (i in 0 until count[0].toInt()) {
            val flen = ((sealed[off].toInt() and 0xff) shl 24) or ((sealed[off + 1].toInt() and 0xff) shl 16) or
                ((sealed[off + 2].toInt() and 0xff) shl 8) or (sealed[off + 3].toInt() and 0xff)
            off += 4
            val frag = sealed.copyOfRange(off, off + flen)
            if (fault == Fault.TAMPER_SECRET && i == 0) {
                frag[frag.size / 2] = (frag[frag.size / 2].toInt() xor 0x01).toByte()
                Log.i(TAG, "fault injected: SECRET fragment $i one byte flipped")
            }
            sendMessage(Framing.MTYPE_CHANNEL, frag)
            off += flen
        }
        Native.secureWipe(sealed)
        ui.onStatus("SECRET (synthetic) queued: ${count[0]} fragments")
    }

    fun stop() { h.post { if (state != State.IDLE) finish("stopped by user") } }

    // ---- advertising -------------------------------------------------------------------------

    private fun startAdvertising() {
        val cb = object : AdvertiseCallback() {
            override fun onStartSuccess(s: AdvertiseSettings) {
                h.post {
                    advStartMs = SystemClock.elapsedRealtime()
                    ui.onStatus("advertising tag $advTag (service UUID + service data), 60 s")
                    Log.i(TAG, "adv started tag-len=4 mode=${if (useIndicate) "indicate" else "notify"}")
                }
            }
            override fun onStartFailure(errorCode: Int) { h.post { finish("advertise failed $errorCode") } }
        }
        advCallback = cb
        val settings = AdvertiseSettings.Builder().setConnectable(true)
            .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY)
            .setTxPowerLevel(AdvertiseSettings.ADVERTISE_TX_POWER_MEDIUM)
            .setTimeout(SESSION_MS.toInt()).build()
        // Advertisement: flags + 128-bit service UUID. Scan response: 4-byte ASCII tag as service
        // data keyed by the service UUID. No device name, no other data (spec section 3).
        val adv = AdvertiseData.Builder().addServiceUuid(ParcelUuid(SERVICE))
            .setIncludeDeviceName(false).setIncludeTxPowerLevel(false).build()
        val scan = AdvertiseData.Builder()
            .addServiceData(ParcelUuid(SERVICE), advTag.toByteArray(Charsets.US_ASCII))
            .setIncludeDeviceName(false).setIncludeTxPowerLevel(false).build()
        advertiser?.startAdvertising(settings, adv, scan, cb)
    }

    // ---- GATT server -------------------------------------------------------------------------

    private val serverCallback = object : BluetoothGattServerCallback() {
        override fun onServiceAdded(status: Int, service: BluetoothGattService) {
            h.post {
                if (status == BluetoothGatt.GATT_SUCCESS) startAdvertising() else finish("service add failed $status")
            }
        }

        override fun onConnectionStateChange(device: BluetoothDevice, status: Int, newState: Int) {
            h.post {
                if (newState == BluetoothGatt.STATE_CONNECTED) {
                    if (central == null && state == State.ADVERTISING) {
                        central = device
                        connectMs = SystemClock.elapsedRealtime()
                        state = State.AWAIT_M1
                        stopAdvertising()
                        ui.onStatus("central connected; waiting for indication enable")
                        val sess = sessionTimer
                        h.postDelayed({ if (sess === sessionTimer && !cccdEnabled && state != State.IDLE) finish("central did not enable ${if (useIndicate) "indications" else "notifications"} in 5 s") }, CCCD_WAIT_MS)
                    } else {
                        gatt?.cancelConnection(device) // at most one connection
                    }
                } else if (newState == BluetoothGatt.STATE_DISCONNECTED && device == central) {
                    if (state != State.DONE) finish("central disconnected")
                }
            }
        }

        override fun onMtuChanged(device: BluetoothDevice, newMtu: Int) {
            h.post { mtu = newMtu; mtuHistory.add(newMtu); Log.i(TAG, "mtu=$newMtu") }
        }

        override fun onDescriptorReadRequest(device: BluetoothDevice, requestId: Int, offset: Int, d: BluetoothGattDescriptor) {
            val v = if (cccdEnabled) (if (useIndicate) BluetoothGattDescriptor.ENABLE_INDICATION_VALUE else BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE) else BluetoothGattDescriptor.DISABLE_NOTIFICATION_VALUE
            gatt?.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, 0, v)
        }

        override fun onDescriptorWriteRequest(device: BluetoothDevice, requestId: Int, d: BluetoothGattDescriptor,
                                              preparedWrite: Boolean, responseNeeded: Boolean, offset: Int, value: ByteArray) {
            val want = if (useIndicate) BluetoothGattDescriptor.ENABLE_INDICATION_VALUE else BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE
            val ok = d.uuid == CCCD && device == central && !preparedWrite && offset == 0 &&
                (value.contentEquals(want) || value.contentEquals(BluetoothGattDescriptor.DISABLE_NOTIFICATION_VALUE))
            if (responseNeeded) gatt?.sendResponse(device, requestId,
                if (ok) BluetoothGatt.GATT_SUCCESS else BluetoothGatt.GATT_REQUEST_NOT_SUPPORTED, 0, null)
            if (ok) h.post {
                cccdEnabled = value.contentEquals(want)
                if (cccdEnabled) {
                    cccdMs = SystemClock.elapsedRealtime()
                    ui.onStatus("central enabled ${if (useIndicate) "INDICATE" else "NOTIFY"}; mtu=$mtu")
                }
            }
        }

        override fun onCharacteristicWriteRequest(device: BluetoothDevice, requestId: Int, c: BluetoothGattCharacteristic,
                                                  preparedWrite: Boolean, responseNeeded: Boolean, offset: Int, value: ByteArray) {
            // RX is Write-with-response. Drop everything until the central enabled TX and when it
            // is not the one accepted connection (spec section 2).
            val accept = c.uuid == RX && device == central && cccdEnabled && !preparedWrite && offset == 0
            if (responseNeeded) gatt?.sendResponse(device, requestId,
                if (accept) BluetoothGatt.GATT_SUCCESS else BluetoothGatt.GATT_REQUEST_NOT_SUPPORTED, 0, null)
            if (accept) h.post { onRx(value) }
        }

        override fun onNotificationSent(device: BluetoothDevice, status: Int) {
            h.post {
                val dt = SystemClock.elapsedRealtimeNanos() - sentAtNs
                sentCallbacks++
                latencySumNs += dt
                if (dt > latencyMaxNs) latencyMaxNs = dt
                if (status != BluetoothGatt.GATT_SUCCESS) { sentCallbackFailures++; finish("onNotificationSent status $status"); return@post }
                if (useIndicate) { inFlight = false; pump() } // Indicate: confirmation received
            }
        }
    }

    private fun stopAdvertising() {
        advCallback?.let { advertiser?.stopAdvertising(it) }
        advCallback = null
    }

    // ---- framing, sending --------------------------------------------------------------------

    private fun chunkSize() = minOf(mtu - 3, MAX_CHUNK_CAP)

    private fun sendMessage(mtype: Int, message: ByteArray, chunkOverride: Int? = null,
                            mutate: ((MutableList<ByteArray>) -> Unit)? = null) {
        val chunks = Framing.chunkMessage(mtype, message, chunkOverride ?: chunkSize()).toMutableList()
        mutate?.invoke(chunks)
        for (c in chunks) queue.addLast(c)
        pump()
    }

    /** Sends M2, applying the chunk-level faults (d1..d3) which need at least 3 chunks. */
    private fun sendM2(m2: ByteArray) {
        when (fault) {
            Fault.CHUNK_DUP, Fault.CHUNK_DROP, Fault.CHUNK_SWAP -> sendMessage(Framing.MTYPE_HANDSHAKE, m2, FAULT_CHUNK) { c ->
                if (c.size < 3) { Log.w(TAG, "fault ${fault.name} not injected: only ${c.size} chunks"); return@sendMessage }
                when (fault) {
                    Fault.CHUNK_DUP -> c.add(2, c[1].copyOf())
                    Fault.CHUNK_DROP -> c.removeAt(1)
                    else -> { val t = c[1]; c[1] = c[2]; c[2] = t }
                }
                Log.i(TAG, "fault injected: ${fault.name} (M2 ${m2.size} bytes in ${c.size} chunks of <=$FAULT_CHUNK)")
            }
            else -> sendMessage(Framing.MTYPE_HANDSHAKE, m2)
        }
    }

    private fun pump() {
        val dev = central ?: return
        val tx = txChar ?: return
        if (inFlight || queue.isEmpty() || !cccdEnabled || state == State.DONE) return
        val chunk = queue.removeFirst()
        inFlight = true
        sentAtNs = SystemClock.elapsedRealtimeNanos()
        chunksSent++
        val st = gatt?.notifyCharacteristicChanged(dev, tx, useIndicate, chunk)
        if (st != BluetoothStatusCodes.SUCCESS) { finish("notifyCharacteristicChanged status $st"); return }
        if (!useIndicate) { // Notify + application ACK, stop-and-wait (spec section 2)
            awaitingAck = chunk[1].toInt() and 0xff
            val t = Runnable { finish("no application ACK within 2 s") }
            ackTimer = t
            h.postDelayed(t, ACK_MS)
        }
    }

    private fun onRx(chunk: ByteArray) {
        if (state == State.DONE) return
        Framing.parseAck(chunk)?.let { seq ->
            acksReceived++
            if (!useIndicate && inFlight && awaitingAck == seq) {
                ackTimer?.let { h.removeCallbacks(it) }
                awaitingAck = -1; inFlight = false; pump()
            }
            return
        }
        if (messageTimer == null) {
            val t = Runnable { finish("message not complete within 5 s") }
            messageTimer = t
            h.postDelayed(t, MESSAGE_MS)
        }
        val msg = try { reassembler.push(chunk) } catch (e: Framing.FramingException) {
            finish("framing error: ${e.message}"); return
        } ?: return
        messageTimer?.let { h.removeCallbacks(it) }
        messageTimer = null
        onMessage(msg)
    }

    // ---- protocol ----------------------------------------------------------------------------

    private fun onMessage(m: Framing.Message) {
        when {
            m.mtype == Framing.MTYPE_HANDSHAKE && state == State.AWAIT_M1 -> {
                val c = code ?: run { finish("no code"); return }
                if (fault == Fault.NO_RESPONSE) {
                    Native.secureWipe(c); code = null
                    state = State.AWAIT_M3
                    Log.i(TAG, "fault injected: M1 received, no M2 will be sent")
                    ui.onStatus("FAULT e1: M1 received, staying silent")
                    return
                }
                val out = ByteArray(1024); val len = longArrayOf(out.size.toLong()); val hs = LongArray(1)
                val st = Native.senderStart(c, m.data, DEVICE_ID, PURPOSE, CLAIMS, out, len, hs)
                Native.secureWipe(c); code = null
                if (st != Native.OK) { finish("sender_start failed $st"); return }
                sender = NativeHandle(hs[0], Native::senderFree)
                state = State.AWAIT_M3
                var m2 = out.copyOf(len[0].toInt())
                if (fault == Fault.GARBAGE_M2) {
                    SecureRandom().nextBytes(m2)
                    Log.i(TAG, "fault injected: M2 replaced by ${m2.size} random bytes")
                }
                if (fault == Fault.DELAY_M2 || fault == Fault.DELAY_M2_LONG) {
                    val sess = sessionTimer
                    val delay = if (fault == Fault.DELAY_M2) FAULT_DELAY_MS else FAULT_DELAY_LONG_MS
                    ui.onStatus("FAULT ${fault.name}: M2 delayed $delay ms")
                    h.postDelayed({
                        if (sess === sessionTimer && state == State.AWAIT_M3) {
                            Log.i(TAG, "fault injected: M2 sent after $delay ms")
                            sendM2(m2)
                        }
                    }, delay)
                } else {
                    sendM2(m2)
                    ui.onStatus("M1 received, M2 sent")
                }
            }
            m.mtype == Framing.MTYPE_HANDSHAKE && state == State.AWAIT_M3 -> {
                if (fault == Fault.DROP_AFTER_M3) {
                    Log.i(TAG, "fault injected: dropping the connection right after M3")
                    finish("fault c: disconnected right after M3"); return
                }
                val out = ByteArray(1024); val len = longArrayOf(out.size.toLong()); val ch = LongArray(1)
                val st = Native.senderFinish(sender?.get() ?: 0L, m.data, out, len, ch)
                sender?.close(); sender = null
                if (st != Native.OK) { finish("sender_finish failed $st (wrong code or tamper)"); return }
                channel = NativeHandle(ch[0], Native::channelFree)
                state = State.CHANNEL
                val m4 = out.copyOf(len[0].toInt())
                if (fault == Fault.TAMPER_M4) {
                    m4[m4.size / 2] = (m4[m4.size / 2].toInt() xor 0x01).toByte()
                    Log.i(TAG, "fault injected: M4 one byte flipped")
                }
                sendMessage(Framing.MTYPE_HANDSHAKE, m4)
                ui.onStatus("M3 verified, M4 sent; secure channel open")
                ui.onApprovalReady()
            }
            m.mtype == Framing.MTYPE_CHANNEL && state == State.CHANNEL -> {
                val plain = ByteArray(1024); val len = longArrayOf(plain.size.toLong()); val info = IntArray(2)
                val st = Native.channelOpenFragment(channel?.get() ?: 0L, m.data, plain, len, info)
                if (st != Native.OK) { Native.secureWipe(plain); finish("open_fragment failed $st"); return }
                if (info[1] == 1 && info[0] == Native.KIND_RESULT) {
                    Native.secureWipe(plain)
                    finish("RESULT received (${len[0]} bytes); transfer complete", ok = true)
                } else Native.secureWipe(plain)
            }
            else -> finish("unexpected message type ${m.mtype} in state $state")
        }
    }

    // ---- teardown ----------------------------------------------------------------------------

    private fun finish(reason: String, ok: Boolean = false) {
        if (finishing) return
        finishing = true
        state = State.DONE
        messageTimer?.let { h.removeCallbacks(it) }; ackTimer?.let { h.removeCallbacks(it) }
        sessionTimer?.let { h.removeCallbacks(it) }
        messageTimer = null; ackTimer = null; sessionTimer = null
        stopAdvertising()
        code?.let { Native.secureWipe(it) }; code = null
        channel?.let { Native.channelClose(it.get()); it.close() }; channel = null
        sender?.close(); sender = null
        reassembler.close()
        queue.clear()
        central?.let { gatt?.cancelConnection(it) }
        gatt?.close(); gatt = null; central = null
        val avgMs = if (sentCallbacks > 0) latencySumNs / sentCallbacks / 1e6 else 0.0
        val summary = "${if (ok) "OK" else "ended"}: $reason | fault=${fault.name} mode=${if (useIndicate) "indicate" else "notify+ack"} " +
            "mtu=$mtu(history=$mtuHistory) chunk=${chunkSize()} chunksSent=$chunksSent sentCallbacks=$sentCallbacks " +
            "cbFail=$sentCallbackFailures cbAvg=${"%.1f".format(avgMs)}ms cbMax=${"%.1f".format(latencyMaxNs / 1e6)}ms " +
            "acks=$acksReceived adv->connect=${if (connectMs > 0 && advStartMs > 0) connectMs - advStartMs else -1}ms " +
            "connect->cccd=${if (cccdMs > 0) cccdMs - connectMs else -1}ms"
        Log.i(TAG, summary)
        ui.onFinished(summary)
        state = State.IDLE
        reassembler = Framing.Reassembler()
        // Allow a fresh session (fresh tag, keys, code) after cleanup; nothing resumes.
        cccdEnabled = false; mtu = 23; inFlight = false; awaitingAck = -1
        mtuHistory = mutableListOf(); sentCallbacks = 0; sentCallbackFailures = 0
        latencySumNs = 0; latencyMaxNs = 0; chunksSent = 0; acksReceived = 0
        advStartMs = 0; connectMs = 0; cccdMs = 0
        fault = Fault.NONE // never stays armed across sessions
        finishing = false
    }
}
