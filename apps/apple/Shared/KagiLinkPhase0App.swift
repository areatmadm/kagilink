import SwiftUI
import CoreBluetooth
import CryptoKit

@main
struct KagiLinkPhase0App: App {
    @StateObject private var probe = BLEProbe()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup("KagiLink Phase 0") {
            ProbeScreen(probe: probe)
            .onChange(of: scenePhase) { _, phase in
                if phase != .active { probe.stop("앱이 활성 상태를 벗어나 시험을 중지했습니다.") }
            }
        }
    }
}

/// Layout follows the current window, not a device name, idiom or orientation.
/// The same hierarchy reflows when Duo folds/unfolds or its bars become vertical.
private struct ProbeScreen: View {
    @ObservedObject var probe: BLEProbe
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @Environment(\.accessibilityContrast) private var contrast
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        GeometryReader { geometry in
            ZStack {
                backdrop
                ScrollView {
                    VStack(alignment: .leading, spacing: 24) {
                        VStack(alignment: .leading, spacing: 8) {
                            Label("AreaTM · 공개 BLE 시험", systemImage: "wave.3.right")
                                .font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
                            Text("KagiLink").font(.largeTitle.bold()).accessibilityAddTraits(.isHeader)
                            Text("가까운 기기와 연결을 확인하세요.").font(.title3)
                        }
                        LazyVGrid(columns: columns(for: geometry.size.width), alignment: .leading, spacing: 20) {
                            information
                            controls
                        }
                        Text("한 번의 시험이 끝나면 다시 시작해야 합니다. 상대 기기의 신원은 검증되지 않습니다.")
                            .font(.footnote).foregroundStyle(.secondary)
                    }
                    .padding(24)
                    .frame(maxWidth: 1080, alignment: .leading)
                    .frame(maxWidth: .infinity)
                }
            }
        }
    }

    private func columns(for width: CGFloat) -> [GridItem] {
        // Reading order remains one column at accessibility sizes and narrow widths.
        let count = !dynamicTypeSize.isAccessibilitySize && width >= 760 ? 2 : 1
        return Array(repeating: GridItem(.flexible(), alignment: .topLeading), count: count)
    }

    @ViewBuilder private var backdrop: some View {
        if reduceTransparency || contrast == .increased {
            (colorScheme == .dark ? Color.black : Color.white).ignoresSafeArea()
        } else {
            LinearGradient(colors: [Color.cyan.opacity(0.18), Color.blue.opacity(0.08), Color.teal.opacity(0.14)],
                           startPoint: .topLeading, endPoint: .bottomTrailing)
                .ignoresSafeArea() // Only decoration extends outside system safe areas.
        }
    }

    private var information: some View {
        VStack(alignment: .leading, spacing: 18) {
            Label("공개 문자열 시험판", systemImage: "testtube.2").font(.headline)
                .accessibilityAddTraits(.isHeader)
            Text("고정된 공개 문자열만 교환합니다. 비밀번호 전송·저장·인증은 지원하지 않습니다.")
            Text("시험 결과는 암호화·하드웨어 보호·자동 입력 성공을 의미하지 않습니다.")
                .foregroundStyle(.secondary)
            Divider()
            Label(SecureEnclave.isAvailable
                  ? "Secure Enclave 사용 가능 보고 · 실제 보호된 키 생성/사용 미검증"
                  : "Secure Enclave 사용 불가 · 향후 민감 정보 전송 불가", systemImage: "shield.lefthalf.filled")
                .font(.footnote)
        }
        .frame(maxWidth: .infinity, alignment: .leading).padding(24)
        .modifier(ProbeReadableSurface())
    }

    private var controls: some View {
        VStack(alignment: .leading, spacing: 18) {
            VStack(alignment: .leading, spacing: 10) {
                Label("시험 상태", systemImage: "dot.radiowaves.left.and.right").font(.headline)
                    .accessibilityAddTraits(.isHeader)
                Text(probe.status).accessibilityLabel("시험 상태: \(probe.status)")
            }
            .frame(maxWidth: .infinity, alignment: .leading).padding(24)
            .modifier(ProbeReadableSurface())
            ProbeGlassGroup {
                VStack(spacing: 14) {
                    ProbeGlassButton(title: "송신기 시험 시작 · 60초", symbol: "antenna.radiowaves.left.and.right") {
                        probe.startPeripheral()
                    }
                    #if os(macOS)
                    ProbeGlassButton(title: "수신기 시험 검색 · 10초", symbol: "magnifyingglass") { probe.startCentral() }
                    #endif
                    ProbeGlassButton(title: "시험 중지", symbol: "stop.fill") { probe.stop("사용자가 중지했습니다.") }
                }
            }
            Text("송신기는 전면에서 광고하고 수신기가 검색합니다.")
                .font(.footnote).foregroundStyle(.secondary)
        }.frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// Material belongs behind readable content; interactive Liquid Glass belongs on controls.
private struct ProbeReadableSurface: ViewModifier {
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @Environment(\.accessibilityContrast) private var contrast
    @Environment(\.colorScheme) private var colorScheme

    func body(content: Content) -> some View {
        let shape = RoundedRectangle(cornerRadius: 24, style: .continuous)
        if reduceTransparency || contrast == .increased {
            content.background(colorScheme == .dark ? Color.black : Color.white, in: shape)
                .overlay(shape.stroke(Color.primary.opacity(0.35), lineWidth: 1))
        } else {
            content.background(.regularMaterial, in: shape)
                .overlay(shape.stroke(Color.primary.opacity(0.12), lineWidth: 1))
        }
    }
}

private struct ProbeGlassGroup<Content: View>: View {
    private let content: Content
    init(@ViewBuilder content: () -> Content) { self.content = content() }
    var body: some View {
        if #available(iOS 26.0, macOS 26.0, *) {
            GlassEffectContainer(spacing: 12) { content }
        } else {
            content
        }
    }
}

private struct ProbeGlassButton: View {
    let title: String
    let symbol: String
    let action: () -> Void
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @Environment(\.accessibilityContrast) private var contrast
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        Button(action: action) {
            Label(title, systemImage: symbol)
                .font(.headline).multilineTextAlignment(.leading)
                .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
                .padding(.horizontal, 20).padding(.vertical, 10)
                .contentShape(RoundedRectangle(cornerRadius: 22, style: .continuous))
        }
        .buttonStyle(.plain)
        .modifier(ProbeControlSurface(opaque: reduceTransparency || contrast == .increased,
                                      dark: colorScheme == .dark))
    }
}

private struct ProbeControlSurface: ViewModifier {
    let opaque: Bool
    let dark: Bool
    func body(content: Content) -> some View {
        let shape = RoundedRectangle(cornerRadius: 22, style: .continuous)
        if opaque {
            content.background(dark ? Color.black : Color.white, in: shape)
                .overlay(shape.stroke(Color.primary.opacity(0.5), lineWidth: 1))
        } else if #available(iOS 26.0, macOS 26.0, *) {
            content.glassEffect(.regular.tint(.cyan.opacity(0.12)).interactive(), in: .rect(cornerRadius: 22))
        } else {
            // iOS 18 supports the same hierarchy with public SwiftUI materials.
            content.background(.regularMaterial, in: shape)
                .overlay(shape.stroke(Color.primary.opacity(0.18), lineWidth: 1))
        }
    }
}

/// Isolated public diagnostic. These UUIDs and payloads MUST NOT carry production data.
/// Core Bluetooth delegates and all UI mutations run on the main queue.
final class BLEProbe: NSObject, ObservableObject, CBPeripheralManagerDelegate,
                      CBCentralManagerDelegate, CBPeripheralDelegate {
    static let serviceID = CBUUID(string: "7d9a0001-6f8e-4d52-9b91-89c767b6e201")
    static let characteristicID = CBUUID(string: "7d9a0002-6f8e-4d52-9b91-89c767b6e201")
    static let ping = Data("KagiLink-P0-PING".utf8)
    static let pong = Data("KagiLink-P0-PONG".utf8)
    @Published private(set) var status = "중지됨. 시험할 역할을 선택하세요."

    private var peripheralManager: CBPeripheralManager?
    private var centralManager: CBCentralManager?
    private var peer: CBPeripheral?
    private var candidates: [UUID: CBPeripheral] = [:]
    private var boundCentral: UUID?
    private var sentReply = false
    private var readPending = false
    private var characteristic: CBCharacteristic?
    private var deadline: ContinuousClock.Instant?
    private var timeout: DispatchWorkItem?
    private var scanTimeout: DispatchWorkItem?
    private var completion: DispatchWorkItem?
    private var generation = 0

    private func begin() {
        stop("준비 중")
        deadline = ContinuousClock().now.advanced(by: .seconds(60))
        let ticket = generation
        let work = DispatchWorkItem { [weak self] in
            guard let self, self.generation == ticket else { return }
            self.stop("시험 시간 만료. 다시 시작해야 합니다.")
        }
        timeout = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 60, execute: work)
    }

    private func live() -> Bool {
        guard let deadline else { return false }
        if ContinuousClock().now >= deadline { stop("시험 시간 만료"); return false }
        return true
    }

    func stop(_ message: String) {
        generation += 1
        deadline = nil
        timeout?.cancel(); timeout = nil
        scanTimeout?.cancel(); scanTimeout = nil
        completion?.cancel(); completion = nil
        peripheralManager?.delegate = nil
        if peripheralManager?.state == .poweredOn {
            peripheralManager?.stopAdvertising()
            peripheralManager?.removeAllServices()
        }
        centralManager?.delegate = nil
        peer?.delegate = nil
        if centralManager?.state == .poweredOn {
            centralManager?.stopScan()
            if let peer { centralManager?.cancelPeripheralConnection(peer) }
        }
        peripheralManager = nil; centralManager = nil; peer = nil
        characteristic = nil; candidates.removeAll(); boundCentral = nil
        sentReply = false; readPending = false
        status = message
    }

    func startPeripheral() {
        begin()
        status = "Bluetooth 송신기 준비 중 · 공개 시험만 허용"
        peripheralManager = CBPeripheralManager(delegate: self, queue: .main,
            options: [CBPeripheralManagerOptionShowPowerAlertKey: false])
    }

    func peripheralManagerDidUpdateState(_ manager: CBPeripheralManager) {
        guard manager === peripheralManager, live() else { return }
        guard manager.state == .poweredOn else {
            if manager.state != .unknown { stop("Bluetooth 권한·라디오·지원 상태를 확인하세요.") }
            return
        }
        let service = CBMutableService(type: Self.serviceID, primary: true)
        service.characteristics = [CBMutableCharacteristic(type: Self.characteristicID,
            properties: [.read, .write], value: nil, permissions: [.readable, .writeable])]
        manager.add(service)
    }

    func peripheralManager(_ manager: CBPeripheralManager, didAdd service: CBService, error: Error?) {
        guard manager === peripheralManager, live() else { return }
        guard error == nil, service.uuid == Self.serviceID else { stop("GATT 서비스 등록 실패"); return }
        // Do not advertise personal device names or any connection code.
        manager.startAdvertising([CBAdvertisementDataServiceUUIDsKey: [Self.serviceID]])
    }

    func peripheralManagerDidStartAdvertising(_ manager: CBPeripheralManager, error: Error?) {
        guard manager === peripheralManager, live() else { return }
        if error != nil { stop("BLE 광고 실패"); return }
        status = "광고 중 · 수신기 연결 대기 · 앱을 전면에 유지하세요."
    }

    func peripheralManager(_ manager: CBPeripheralManager, didReceiveWrite requests: [CBATTRequest]) {
        guard manager === peripheralManager, live(), let first = requests.first else { return }
        let valid = requests.count == 1 && first.characteristic.uuid == Self.characteristicID &&
            first.offset == 0 && first.value == Self.ping && boundCentral == nil && !sentReply
        if valid { boundCentral = first.central.identifier }
        manager.respond(to: first, withResult: valid ? .success : .unlikelyError)
        if !valid { stop("잘못되거나 중복된 시험 요청"); return }
        status = "고정 요청 수신. 동일 수신기의 고정 응답 읽기 대기 중."
    }

    func peripheralManager(_ manager: CBPeripheralManager, didReceiveRead request: CBATTRequest) {
        guard manager === peripheralManager, live() else { return }
        guard request.characteristic.uuid == Self.characteristicID, request.offset == 0,
              request.central.identifier == boundCentral, !sentReply else {
            manager.respond(to: request, withResult: .unlikelyError)
            stop("시험 읽기 실패 또는 잘못된 순서"); return
        }
        sentReply = true
        request.value = Self.pong
        manager.respond(to: request, withResult: .success)
        manager.stopAdvertising()
        status = "고정 응답을 Bluetooth 스택에 전달했습니다. 최종 결과는 수신기에서 확인하세요."
        let ticket = generation
        let work = DispatchWorkItem { [weak self] in
            guard let self, self.generation == ticket else { return }
            self.stop("시험 종료. 인증·입력 성공을 의미하지 않습니다.")
        }
        completion = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 1, execute: work)
    }

    #if os(macOS)
    func startCentral() {
        begin()
        status = "Bluetooth 수신기 준비 중"
        centralManager = CBCentralManager(delegate: self, queue: .main,
            options: [CBCentralManagerOptionShowPowerAlertKey: false])
    }
    #endif

    func centralManagerDidUpdateState(_ manager: CBCentralManager) {
        guard manager === centralManager, live() else { return }
        guard manager.state == .poweredOn else {
            if manager.state != .unknown { stop("Bluetooth 권한·라디오·지원 상태를 확인하세요.") }
            return
        }
        manager.scanForPeripherals(withServices: [Self.serviceID])
        status = "시험용 송신기 검색 중 (10초)"
        let ticket = generation
        let work = DispatchWorkItem { [weak self, weak manager] in
            guard let self, let manager, self.generation == ticket, self.live() else { return }
            manager.stopScan()
            guard self.candidates.count == 1, let selected = self.candidates.values.first else {
                self.stop(self.candidates.isEmpty ? "시험용 송신기 없음" : "송신기 여러 개 발견. 다른 시험기를 중지하세요.")
                return
            }
            self.peer = selected
            selected.delegate = self
            manager.connect(selected)
            self.status = "시험용 송신기에 연결 중 · 신원 미검증"
        }
        scanTimeout = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 10, execute: work)
    }

    func centralManager(_ manager: CBCentralManager, didDiscover peripheral: CBPeripheral,
                        advertisementData: [String: Any], rssi RSSI: NSNumber) {
        guard manager === centralManager, live(), peer == nil else { return }
        // Transient routing identity only. Never output names, addresses or received data.
        if candidates.count < 2 || candidates[peripheral.identifier] != nil {
            candidates[peripheral.identifier] = peripheral
        }
    }

    func centralManager(_ manager: CBCentralManager, didConnect peripheral: CBPeripheral) {
        guard manager === centralManager, peripheral === peer, live() else { return }
        peripheral.discoverServices([Self.serviceID])
    }

    func centralManager(_ manager: CBCentralManager, didFailToConnect peripheral: CBPeripheral, error: Error?) {
        if manager === centralManager && peripheral === peer { stop("시험 연결 실패") }
    }

    func centralManager(_ manager: CBCentralManager, didDisconnectPeripheral peripheral: CBPeripheral, error: Error?) {
        if manager === centralManager && peripheral === peer { stop("응답 검증 전 연결 종료") }
    }

    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        guard peripheral === peer, live() else { return }
        let matches = peripheral.services?.filter { $0.uuid == Self.serviceID } ?? []
        guard error == nil, matches.count == 1 else { stop("시험 서비스 사용 불가"); return }
        peripheral.discoverCharacteristics([Self.characteristicID], for: matches[0])
    }

    func peripheral(_ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService, error: Error?) {
        guard peripheral === peer, live() else { return }
        let matches = service.characteristics?.filter { $0.uuid == Self.characteristicID } ?? []
        guard error == nil, service.uuid == Self.serviceID, matches.count == 1,
              matches[0].properties.contains(.write), matches[0].properties.contains(.read),
              characteristic == nil else { stop("시험 특성 사용 불가"); return }
        characteristic = matches[0]
        peripheral.writeValue(Self.ping, for: matches[0], type: .withResponse)
    }

    func peripheral(_ peripheral: CBPeripheral, didWriteValueFor item: CBCharacteristic, error: Error?) {
        guard peripheral === peer, live(), item === characteristic else { return }
        guard error == nil, !readPending else { stop("고정 요청 쓰기 실패"); return }
        readPending = true
        peripheral.readValue(for: item)
    }

    func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor item: CBCharacteristic, error: Error?) {
        guard peripheral === peer, live(), item === characteristic else { return }
        guard readPending, error == nil, item.value == Self.pong else { stop("고정 응답 불일치 또는 읽기 실패"); return }
        stop("공개 시험 쓰기·읽기 검증 성공. 인증·암호화·자동 입력·장치 무결성은 미검증입니다.")
    }
}
