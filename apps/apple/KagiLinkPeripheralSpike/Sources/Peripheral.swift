// KL-V1 peripheral spike (D021 follow-up): chunk framing now comes from the shared Rust codec (ABI v2). Advertises, answers the receiver's M1..M4 handshake and
// sends one synthetic SECRET through the shared Rust core. Synthetic data only. Measures MTU,
// subscribe/queue behaviour and foreground/background advertising. No pairing, no Keychain.
import CoreBluetooth
import Foundation
import Security
import SwiftUI

private let serviceUUID = CBUUID(string: "3685355e-2903-4e85-bdd6-474ada0838c1")
private let rxUUID = CBUUID(string: "316da2a3-cfc7-4bbf-a736-58c4e7d8d5cf")
private let txUUID = CBUUID(string: "7c69e9b3-c395-4fc0-87b2-b564d7b8b29e")
private let tagAlphabet = Array("23456789ABCDEFGHJKMNPQRSTUVWXYZ")
private let syntheticSecret = Array("synthetic-spike-secret-0123456789".utf8)

#if FAULT_INJECTION
/// Test-only fault injection (build with FAULT_INJECTION=1). Off by default; never compiled into the normal build.
enum FaultMode: String, CaseIterable, Identifiable {
    case none, garbageM2, tamperM4, dropAfterM3, dupChunk, skipChunk, reorderChunk, delayM2, delayM2Long, tamperSecret
    var id: String { rawValue }
    /// Chunk faults need a multi-chunk message, so M2 is sent in small chunks.
    var usesSmallChunks: Bool { self == .dupChunk || self == .skipChunk || self == .reorderChunk }
}
struct FaultStop: Error {}
#endif

final class PeripheralSpike: NSObject, ObservableObject, CBPeripheralManagerDelegate {
    @Published var tag = "----"
    @Published var log: [String] = []
    @Published var running = false
#if FAULT_INJECTION
    @Published var fault: FaultMode = .none
#endif

    private var manager: CBPeripheralManager!
    private var tx: CBMutableCharacteristic?
    private var central: CBCentral?
    private var reassembler: FrameReassembler?
    private var sender: Sender?
    private var channel: Channel?
    private var code: [UInt8] = []
    private var phase = 0 // 0 idle, 1 await M1, 2 await M3, 3 await RESULT
    private var outbound: [[UInt8]] = []
    private var chunkSize = 20
    private var stopTimer: Timer?
    private var updateFalse = 0, readyCalls = 0
    private let started = Date()

    override init() {
        super.init()
        manager = CBPeripheralManager(delegate: self, queue: .main)
    }

    private func note(_ s: String) {
        let line = String(format: "[%6.2fs] %@", Date().timeIntervalSince(started), s)
        print(line)
        log.append(line)
    }

    func noteScene(_ s: String) { note("scene=\(s) advertising=\(manager.isAdvertising)") }

    // MARK: start / stop

    private func randomTag() -> String {
        var out = ""
        while out.count < 4 {
            var b: UInt8 = 0
            guard SecRandomCopyBytes(kSecRandomDefault, 1, &b) == errSecSuccess else { break }
            if b < 248 { out.append(tagAlphabet[Int(b % 31)]) } // rejection sampling, no bias
        }
        return out
    }

    func start(codeDigits: String) {
        guard !running, manager.state == .poweredOn, codeDigits.utf8.count == 6,
              codeDigits.utf8.allSatisfy({ $0 >= 48 && $0 <= 57 }) else { note("start refused (state/code)"); return }
        code = Array(codeDigits.utf8)
        reset()
        tag = randomTag()
        let rx = CBMutableCharacteristic(type: rxUUID, properties: [.write], value: nil, permissions: [.writeable])
        let txc = CBMutableCharacteristic(type: txUUID, properties: [.indicate], value: nil, permissions: [.readable])
        let svc = CBMutableService(type: serviceUUID, primary: true)
        svc.characteristics = [rx, txc]
        tx = txc
        manager.removeAllServices()
        manager.add(svc)
        running = true
        phase = 1
        note("service added, tag KL-\(tag)")
#if FAULT_INJECTION
        note("FAULT MODE = \(fault.rawValue) (test build, synthetic data)")
#endif
    }

    func stop(_ why: String) {
        manager.stopAdvertising()
        manager.removeAllServices()
        stopTimer?.invalidate()
        Native.wipe(&code)
        sender = nil
        channel = nil
        outbound = []
        central = nil
        running = false
        phase = 0
#if FAULT_INJECTION
        note("FAULT RESULT mode=\(fault.rawValue): ended with \"\(why)\"")
#endif
        note("stopped: \(why); updateValue-false=\(updateFalse) readyCallbacks=\(readyCalls)")
    }

    private func reset() {
        reassembler = try? FrameReassembler()
        sender = nil; channel = nil; outbound = []; central = nil
        updateFalse = 0; readyCalls = 0; chunkSize = 20
    }

    // MARK: delegate

    func peripheralManagerDidUpdateState(_ p: CBPeripheralManager) { note("bluetooth state=\(p.state.rawValue)") }

    func peripheralManager(_ p: CBPeripheralManager, didAdd service: CBService, error: Error?) {
        if let error { note("add service failed: \(error)"); stop("add failed"); return }
        p.startAdvertising([CBAdvertisementDataServiceUUIDsKey: [serviceUUID],
                            CBAdvertisementDataLocalNameKey: "KL-\(tag)"])
        stopTimer = Timer.scheduledTimer(withTimeInterval: 60, repeats: false) { [weak self] _ in self?.stop("60 s timeout") }
    }

    func peripheralManagerDidStartAdvertising(_ p: CBPeripheralManager, error: Error?) {
        note(error.map { "advertising failed: \($0)" } ?? "advertising started name=KL-\(tag) (foreground)")
    }

    func peripheralManager(_ p: CBPeripheralManager, central c: CBCentral, didSubscribeTo ch: CBCharacteristic) {
        guard central == nil else { note("extra subscriber refused"); return } // one connection only
        central = c
        chunkSize = min(c.maximumUpdateValueLength, 244)
#if FAULT_INJECTION
        if fault.usesSmallChunks { chunkSize = 60 }
#endif
        p.stopAdvertising()
        note("subscribed: maximumUpdateValueLength=\(c.maximumUpdateValueLength) (ATT MTU \(c.maximumUpdateValueLength + 3)), chunk=\(chunkSize); advertising stopped")
    }

    func peripheralManager(_ p: CBPeripheralManager, central c: CBCentral, didUnsubscribeFrom ch: CBCharacteristic) {
        note("central unsubscribed"); if running { stop("central gone") }
    }

    func peripheralManager(_ p: CBPeripheralManager, didReceiveWrite requests: [CBATTRequest]) {
        guard let first = requests.first else { return }
        guard central != nil else { p.respond(to: first, withResult: .insufficientAuthorization); return } // drop before subscribe
        var failed = false
        for r in requests where r.characteristic.uuid == rxUUID {
            guard let v = r.value else { failed = true; continue }
            do {
                guard let ra = reassembler else { throw KagiLinkError(status: .null) }
                if let m = try ra.push([UInt8](v)) { try onMessage(m.mtype, m.data) }
            } catch { note("protocol error: \(error)"); failed = true }
        }
        p.respond(to: first, withResult: failed ? .unlikelyError : .success) // one response covers the batch
        if failed { stop("error (peripheral cannot drop the link itself)") }
    }

    func peripheralManagerIsReady(toUpdateSubscribers p: CBPeripheralManager) { readyCalls += 1; pump() }

    // MARK: protocol

    private func onMessage(_ mtype: UInt8, _ data: [UInt8]) throws {
        note("message type=\(mtype) len=\(data.count) phase=\(phase)")
        switch (phase, mtype) {
        case (1, Codec.mtypeHandshake):
            let s = try Sender(code: code, m1: data, deviceID: Array("ios-spike".utf8),
                               purpose: Array("synthetic-login".utf8), claims: Array("mode=spike".utf8))
            Native.wipe(&code) // the code is only needed for M2
            sender = s
            try sendM2(s.m2)
            phase = 2
        case (2, Codec.mtypeHandshake):
#if FAULT_INJECTION
            if fault == .dropAfterM3 {
                note("FAULT dropAfterM3: M3 received; no M4, error response, service removed (the app cannot drop the link itself)")
                throw FaultStop()
            }
#endif
            guard let s = sender else { throw KagiLinkError(status: .malformed) }
            let (m4plain, ch) = try s.finish(m3: data)
            var m4 = m4plain
#if FAULT_INJECTION
            if fault == .tamperM4 { m4[m4.count - 1] ^= 0x01; note("FAULT tamperM4: flipped one bit of M4") }
#endif
            channel = ch
            sender = nil
            try send(Codec.mtypeHandshake, m4)
            try ch.approveOneTransfer() // spike: the Start tap is the single synthetic approval
            for (i, fragment) in try ch.seal(.secret, plaintext: syntheticSecret, maxFragmentPayload: 200).enumerated() {
                var f = fragment
#if FAULT_INJECTION
                if fault == .tamperSecret && i == 0 { f[f.count - 1] ^= 0x01; note("FAULT tamperSecret: flipped one bit of the first SECRET fragment") }
#endif
                try send(Codec.mtypeChannel, f)
            }
            phase = 3
        case (3, Codec.mtypeChannel):
            guard let ch = channel else { throw KagiLinkError(status: .malformed) }
            if let (kind, plain) = try ch.open(fragment: data) {
                note("RESULT-side message kind=\(kind) len=\(plain.count) (content not logged)")
                plain.wipe()
                DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in self?.stop("exchange complete") }
            }
        default:
            throw KagiLinkError(status: .malformed)
        }
    }

    /// M2 goes through here so test builds can mangle it; the normal build just sends it.
    private func sendM2(_ m2: [UInt8]) throws {
#if FAULT_INJECTION
        switch fault {
        case .garbageM2:
            note("FAULT garbageM2: sending 40 garbage bytes instead of M2")
            try send(Codec.mtypeHandshake, [UInt8](repeating: 0xA5, count: 40))
        case .delayM2, .delayM2Long:
            // The receiver waits up to 10 s for a first chunk (FIRST_CHUNK_WAIT): 6 s is still accepted by design,
            // 11 s must time out.
            let seconds: Double = fault == .delayM2Long ? 11 : 6
            note("FAULT \(fault.rawValue): holding M2 for \(Int(seconds)) s")
            DispatchQueue.main.asyncAfter(deadline: .now() + seconds) { [weak self] in
                guard let self, self.running else { return }
                self.note("FAULT \(self.fault.rawValue): sending the late M2 now")
                try? self.send(Codec.mtypeHandshake, m2)
            }
        case .dupChunk, .skipChunk, .reorderChunk:
            var chunks = try Codec.chunk(mtype: Codec.mtypeHandshake, message: m2, maxChunk: chunkSize)
            note("FAULT \(fault.rawValue): M2 split into \(chunks.count) chunks of at most \(chunkSize) bytes")
            if fault == .dupChunk { chunks.insert(chunks[0], at: 0) }
            if fault == .skipChunk && chunks.count > 2 { chunks.remove(at: 1) }
            if fault == .reorderChunk && chunks.count > 2 { chunks.swapAt(1, 2) }
            outbound.append(contentsOf: chunks)
            pump()
        default:
            try send(Codec.mtypeHandshake, m2)
        }
#else
        try send(Codec.mtypeHandshake, m2)
#endif
    }

    private func send(_ mtype: UInt8, _ message: [UInt8]) throws {
        outbound.append(contentsOf: try Codec.chunk(mtype: mtype, message: message, maxChunk: chunkSize))
        pump()
    }

    private func pump() {
        guard let tx, let central else { return }
        while let next = outbound.first {
            if manager.updateValue(Data(next), for: tx, onSubscribedCentrals: [central]) {
                outbound.removeFirst()
            } else {
                updateFalse += 1
                return // wait for peripheralManagerIsReady
            }
        }
    }
}
