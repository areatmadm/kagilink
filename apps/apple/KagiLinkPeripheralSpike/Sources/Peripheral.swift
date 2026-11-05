// KL-V1 peripheral spike (D021 follow-up): advertises, answers the receiver's M1..M4 handshake and
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

final class PeripheralSpike: NSObject, ObservableObject, CBPeripheralManagerDelegate {
    @Published var tag = "----"
    @Published var log: [String] = []
    @Published var running = false

    private var manager: CBPeripheralManager!
    private var tx: CBMutableCharacteristic?
    private var central: CBCentral?
    private var reassembler = Framing.Reassembler()
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
        note("stopped: \(why); updateValue-false=\(updateFalse) readyCallbacks=\(readyCalls)")
    }

    private func reset() {
        reassembler = Framing.Reassembler()
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
                if let m = try reassembler.push([UInt8](v)) { try onMessage(m.mtype, m.data) }
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
        case (1, Framing.mtypeHandshake):
            let s = try Sender(code: code, m1: data, deviceID: Array("ios-spike".utf8),
                               purpose: Array("synthetic-login".utf8), claims: Array("mode=spike".utf8))
            Native.wipe(&code) // the code is only needed for M2
            sender = s
            try send(Framing.mtypeHandshake, s.m2)
            phase = 2
        case (2, Framing.mtypeHandshake):
            guard let s = sender else { throw FrameError.malformed }
            let (m4, ch) = try s.finish(m3: data)
            channel = ch
            sender = nil
            try send(Framing.mtypeHandshake, m4)
            try ch.approveOneTransfer() // spike: the Start tap is the single synthetic approval
            for f in try ch.seal(.secret, plaintext: syntheticSecret, maxFragmentPayload: 200) {
                try send(Framing.mtypeChannel, f)
            }
            phase = 3
        case (3, Framing.mtypeChannel):
            guard let ch = channel else { throw FrameError.malformed }
            if let (kind, plain) = try ch.open(fragment: data) {
                note("RESULT-side message kind=\(kind) len=\(plain.count) (content not logged)")
                plain.wipe()
                DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in self?.stop("exchange complete") }
            }
        default:
            throw FrameError.malformed
        }
    }

    private func send(_ mtype: UInt8, _ message: [UInt8]) throws {
        outbound.append(contentsOf: try Framing.chunk(mtype: mtype, message: message, maxChunk: chunkSize))
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
