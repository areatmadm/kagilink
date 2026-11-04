package `in`.minori.key.phase0

import android.Manifest
import android.app.Activity
import android.bluetooth.*
import android.bluetooth.le.*
import android.content.pm.PackageManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.ParcelUuid
import android.os.SystemClock
import android.view.WindowManager
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import java.util.UUID

/** Phase 0 public diagnostic. No entry field, storage, authentication or secret API. */
class MainActivity : Activity() {
    companion object {
        private val SERVICE = UUID.fromString("7d9a0001-6f8e-4d52-9b91-89c767b6e201")
        private val CHARACTERISTIC = UUID.fromString("7d9a0002-6f8e-4d52-9b91-89c767b6e201")
        private val PING = "KagiLink-P0-PING".toByteArray(Charsets.US_ASCII)
        private val PONG = "KagiLink-P0-PONG".toByteArray(Charsets.US_ASCII)
    }
    private val main = Handler(Looper.getMainLooper())
    private lateinit var status: TextView
    private var server: BluetoothGattServer? = null
    private var advertiser: BluetoothLeAdvertiser? = null
    private var active = false
    private var generation = 0
    private var expires = 0L
    private var peer: BluetoothDevice? = null
    private var replied = false
    private val timeout = Runnable { stop("시험 시간 만료. 다시 시작해야 합니다.") }
    private val radioReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action == BluetoothAdapter.ACTION_STATE_CHANGED &&
                intent.getIntExtra(BluetoothAdapter.EXTRA_STATE, BluetoothAdapter.ERROR) != BluetoothAdapter.STATE_ON) {
                stop("Bluetooth 라디오가 꺼지거나 상태가 바뀌어 시험을 중지했습니다.")
            }
        }
    }
    private fun show(message: String) { status.text = message }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        window.setDecorFitsSystemWindows(false)
        window.insetsController?.setSystemBarsAppearance(0,
            WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS)
        val layout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(GlassUI.dp(this@MainActivity, 24), GlassUI.dp(this@MainActivity, 24),
                GlassUI.dp(this@MainActivity, 24), GlassUI.dp(this@MainActivity, 24))
        }
        layout.addView(GlassUI.text(this, "KagiLink", 34f, heading = true))
        layout.addView(GlassUI.text(this, "AreaTM · Android BLE 시험판", 15f))
        val information = GlassUI.panel(this)
        information.addView(GlassUI.text(this, "가까이에서, 한 번만", 22f, heading = true))
        information.addView(GlassUI.text(this, "공개 고정 문자열만 교환합니다. 비밀번호 전송·저장·인증은 지원하지 않습니다."))
        information.addView(GlassUI.text(this, "송신기는 광고하고 Windows/macOS 수신기가 검색합니다. 상대 기기 신원은 검증되지 않습니다."))
        val strongBox = packageManager.hasSystemFeature(PackageManager.FEATURE_STRONGBOX_KEYSTORE)
        information.addView(GlassUI.text(this, if (strongBox) "StrongBox 기능 보고됨 · 실제 키 생성/사용 미검증" else "StrongBox 미지원 · 향후 민감 정보 전송 불가", 15f))
        layout.addView(information)
        val controls = GlassUI.panel(this)
        controls.addView(GlassUI.text(this, "연결 시험", 22f, heading = true))
        status = GlassUI.text(this, "중지됨. 앱을 전면에 유지하고 시험을 시작하세요.")
        status.accessibilityLiveRegion = android.view.View.ACCESSIBILITY_LIVE_REGION_POLITE
        controls.addView(status)
        controls.addView(GlassUI.button(this, "공개 BLE 시험 시작 (60초)") {
                if (!permitted()) requestPermissions(arrayOf(Manifest.permission.BLUETOOTH_ADVERTISE, Manifest.permission.BLUETOOTH_CONNECT), 1)
                else start()
        })
        controls.addView(GlassUI.button(this, "중지") { stop("사용자가 중지했습니다.") })
        layout.addView(controls)
        val scroll = ScrollView(this).apply { background = GlassBackdrop(); isFillViewport = true; addView(layout) }
        scroll.setOnApplyWindowInsetsListener { view, insets ->
            val safe = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout())
            view.setPadding(safe.left, safe.top, safe.right, safe.bottom)
            insets
        }
        setContentView(scroll)
        val filter = IntentFilter(BluetoothAdapter.ACTION_STATE_CHANGED)
        // Bluetooth broadcasts can originate in a privileged Bluetooth process.
        // This protected system action is used only to stop, never to authorize.
        registerReceiver(radioReceiver, filter, RECEIVER_EXPORTED)
    }

    private fun permitted() = checkSelfPermission(Manifest.permission.BLUETOOTH_ADVERTISE) == PackageManager.PERMISSION_GRANTED &&
        checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT) == PackageManager.PERMISSION_GRANTED

    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        // Permission grant never silently starts a radio test; require another explicit tap.
        show(if (permitted()) "권한 허용됨. 시작 버튼을 다시 누르세요." else "Bluetooth 권한 거부. 시험이 중지되었습니다.")
    }

    @Suppress("MissingPermission")
    private fun start() {
        stop("준비 중")
        if (!permitted()) { show("Bluetooth 권한이 필요합니다."); return }
        try {
            val manager = getSystemService(BluetoothManager::class.java)
                ?: run { show("Bluetooth 관리자 사용 불가"); return }
            val adapter = manager.adapter
            if (adapter == null || !adapter.isEnabled || !adapter.isMultipleAdvertisementSupported) {
                show("BLE 광고 사용 불가. 어댑터와 라디오를 확인하세요."); return
            }
            advertiser = adapter.bluetoothLeAdvertiser
            if (advertiser == null) { show("BLE 광고 API 사용 불가"); return }
            active = true
            expires = SystemClock.elapsedRealtime() + 60_000L
            val ticket = generation
            server = manager.openGattServer(this, callback(ticket))
            val service = BluetoothGattService(SERVICE, BluetoothGattService.SERVICE_TYPE_PRIMARY)
            service.addCharacteristic(BluetoothGattCharacteristic(CHARACTERISTIC,
                BluetoothGattCharacteristic.PROPERTY_READ or BluetoothGattCharacteristic.PROPERTY_WRITE,
                BluetoothGattCharacteristic.PERMISSION_READ or BluetoothGattCharacteristic.PERMISSION_WRITE))
            if (server == null || server?.addService(service) != true) { stop("GATT 서비스 생성 실패"); return }
            main.postDelayed(timeout, 60_000L)
            show("GATT 서비스 준비 중 · 민감 정보 전송 비활성")
        } catch (_: Exception) { stop("Bluetooth API 실패. 시험이 중지되었습니다.") }
    }

    // All platform callbacks are marshalled onto main, where lifecycle/generation is checked.
    @Suppress("MissingPermission")
    private fun callback(ticket: Int) = object : BluetoothGattServerCallback() {
        override fun onServiceAdded(result: Int, service: BluetoothGattService) { main.post {
            if (!live(ticket)) return@post
            if (result != BluetoothGatt.GATT_SUCCESS || service.uuid != SERVICE) { stop("GATT 서비스 등록 실패"); return@post }
            try {
                advertiser?.startAdvertising(AdvertiseSettings.Builder().setConnectable(true)
                    .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY).setTimeout(60_000).build(),
                    AdvertiseData.Builder().addServiceUuid(ParcelUuid(SERVICE)).setIncludeDeviceName(false).build(), advertising(ticket))
            } catch (_: Exception) { stop("광고 시작 실패"); }
        } }
        override fun onConnectionStateChange(device: BluetoothDevice, result: Int, state: Int) { main.post {
            if (!live(ticket)) return@post
            if (result != BluetoothGatt.GATT_SUCCESS || (state == BluetoothProfile.STATE_DISCONNECTED && device == peer)) {
                stop("연결 종료. 새 시험을 시작하세요.")
            }
        } }
        override fun onCharacteristicWriteRequest(device: BluetoothDevice, requestId: Int,
            characteristic: BluetoothGattCharacteristic, preparedWrite: Boolean, responseNeeded: Boolean, offset: Int, value: ByteArray) {
            // Do not retain arbitrary incoming byte arrays on the UI queue.
            val fixedRequest = value.contentEquals(PING)
            main.post {
            if (!live(ticket)) return@post
            val valid = characteristic.uuid == CHARACTERISTIC && !preparedWrite && responseNeeded &&
                offset == 0 && peer == null && !replied && fixedRequest
            try {
                if (valid) peer = device
                val sent = !responseNeeded || server?.sendResponse(device, requestId,
                    if (valid) BluetoothGatt.GATT_SUCCESS else BluetoothGatt.GATT_FAILURE, 0, null) == true
                if (!valid) stop("잘못되거나 중복된 시험 요청. 중지되었습니다.")
                else if (!sent) stop("시험 쓰기 응답 실패")
                else show("고정 요청 수신. 동일 수신기의 고정 응답 읽기 대기 중.")
            } catch (_: Exception) { stop("GATT 쓰기 실패") }
        } }
        override fun onCharacteristicReadRequest(device: BluetoothDevice, requestId: Int, offset: Int,
            characteristic: BluetoothGattCharacteristic) { main.post {
            if (!live(ticket)) return@post
            val valid = characteristic.uuid == CHARACTERISTIC && offset == 0 && device == peer && !replied
            try {
                if (valid) replied = true
                val sent = server?.sendResponse(device, requestId,
                    if (valid) BluetoothGatt.GATT_SUCCESS else BluetoothGatt.GATT_FAILURE, 0, if (valid) PONG else null) == true
                if (!valid || !sent) stop("시험 읽기 실패 또는 잘못된 순서")
                else {
                    advertiseCallback?.let { advertiser?.stopAdvertising(it) }
                    show("고정 응답을 Bluetooth 스택에 전달했습니다. 수신기 결과를 확인하세요.\n인증·입력 성공을 의미하지 않습니다.")
                    // Keep the link briefly so the ATT response can leave the platform stack.
                    main.postDelayed({ if (ticket == generation) stop("시험 종료. 최종 결과는 수신기에서 확인하세요.") }, 1000)
                }
            } catch (_: Exception) { stop("GATT 읽기 실패") }
        } }
        override fun onExecuteWrite(device: BluetoothDevice, requestId: Int, execute: Boolean) { main.post {
            if (!live(ticket)) return@post
            try { server?.sendResponse(device, requestId, BluetoothGatt.GATT_REQUEST_NOT_SUPPORTED, 0, null) } catch (_: Exception) {}
            stop("준비 쓰기는 지원하지 않습니다.")
        } }
    }

    private var advertiseCallback: AdvertiseCallback? = null
    private fun advertising(ticket: Int): AdvertiseCallback = object : AdvertiseCallback() {
        override fun onStartSuccess(settings: AdvertiseSettings) { main.post {
            if (live(ticket)) show("광고 중 · 수신기 연결 대기 · 60초 이내 공개 시험만 허용")
        } }
        override fun onStartFailure(errorCode: Int) { main.post { if (ticket == generation) stop("BLE 광고 실패 ($errorCode)") } }
    }.also { advertiseCallback = it }

    private fun live(ticket: Int): Boolean {
        if (ticket != generation || !active) return false
        if (SystemClock.elapsedRealtime() >= expires || !permitted()) { stop("시간 만료 또는 권한 해제"); return false }
        return true
    }

    @Suppress("MissingPermission")
    private fun stop(message: String) {
        active = false
        generation++
        main.removeCallbacks(timeout)
        try { advertiseCallback?.let { advertiser?.stopAdvertising(it) } } catch (_: Exception) {}
        try { server?.close() } catch (_: Exception) {}
        server = null; advertiser = null; advertiseCallback = null; peer = null; replied = false
        if (::status.isInitialized) show(message)
    }

    override fun onPause() { stop("앱이 전면을 벗어나 시험이 중지되었습니다."); super.onPause() }
    override fun onDestroy() { stop("종료됨"); unregisterReceiver(radioReceiver); super.onDestroy() }
}
