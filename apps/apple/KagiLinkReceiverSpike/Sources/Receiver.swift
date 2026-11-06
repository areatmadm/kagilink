// KL-V1 macOS receiver spike (BLE Central). Synthetic data only; real-secret transfer is disabled (D012).
// Session rules follow the Windows receiver: code TTL 60 s and three-failure budget live in the shared core,
// first chunk of the next message within 10 s, a message completes within 5 s of its first chunk,
// the user (or a harness acting for the user) picks the advertiser; never auto-connects.
// Nothing here proves device integrity or safety: all claims are claims. The code is shown in the window only
// and never written to the log; received plaintext is never logged (only its length).
import CoreBluetooth
import Foundation
import SwiftUI

let klServiceUUID = CBUUID(string: "3685355e-2903-4e85-bdd6-474ada0838c1")
let klRxUUID = CBUUID(string: "316da2a3-cfc7-4bbf-a736-58c4e7d8d5cf")
let klTxUUID = CBUUID(string: "7c69e9b3-c395-4fc0-87b2-b564d7b8b29e")
let tagAlphabet = Set("23456789ABCDEFGHJKMNPQRSTUVWXYZ")
let maxFailures = 3
let firstChunkWait: TimeInterval = 10
let messageComplete: TimeInterval = 5

struct Row: Identifiable {
    let id: UUID
    var title: String
    var signalText: String
    var selectable: Bool
}

private struct Advertiser {
    let peripheral: CBPeripheral
    var nameTag: String?
    var dataTag: String?
    var rssi: Int
    var tag: String? { nameTag ?? dataTag }
    var conflict: Bool { nameTag != nil && dataTag != nil && nameTag != dataTag }
}

private enum Phase { case idle, scanning, listing, connecting, handshakeM2, handshakeM4, secret, finished }

func utcStamp() -> String {
    let f = ISO8601DateFormatter()
    f.formatOptions = [.withInternetDateTime]
    return f.string(from: Date())
}

private func statusName(_ s: KLStatus) -> String {
    let names: [Int32: String] = [1: "Expired", 2: "NotActive", 3: "TooManyFailures", 4: "AttemptInProgress", 5: "Malformed",
        6: "Unsupported", 7: "WrongState", 8: "Pake", 9: "ConfirmationFailed", 10: "BadCode", 11: "Authentication",
        12: "Framing", 13: "Limits", 14: "CounterExhausted", 15: "NotApproved", 16: "Closed", 17: "Randomness",
        100: "Null", 101: "Buffer", 102: "Busy", 103: "Panic", 104: "Abi"]
    return names[s.rawValue] ?? "Status\(s.rawValue)"
}

private enum AttemptError: Error {
    case core(KLStatus)
    case link
    case timeout
    case unexpectedMessage
    case unexpectedKind
    case connect(String)
    var text: String {
        switch self {
        case .core(let s): return "Core(\(statusName(s)))"
        case .link: return "Link(Disconnected)"
        case .timeout: return "Timeout"
        case .unexpectedMessage: return "UnexpectedMessage"
        case .unexpectedKind: return "UnexpectedKind"
        case .connect(let m): return "Connect(\(m))"
        }
    }
}

final class ReceiverSpike: NSObject, ObservableObject, CBCentralManagerDelegate, CBPeripheralDelegate {
    @Published var codeText = "—"
    @Published var status = "starting"
    @Published var rows: [Row] = []
    @Published var log: [String] = []
    @Published var busy = false
    @Published var codeActive = false

    private var central: CBCentralManager!
    private var receiver: Receiver?
    private var derivedFailures = 0
    private var phase: Phase = .idle
    private var adverts: [UUID: Advertiser] = [:]
    private var extraListens = 0
    private var autoScan = false
    private var chooseFile: String?
    private var chooseTimer: Timer?

    private var peripheral: CBPeripheral?
    private var rxChar: CBCharacteristic?
    private var txChar: CBCharacteristic?
    private var chunkSize = 20
    private var txNeedsAck = false
    private var reassembler: FrameReassembler?
    private var channel: Channel?
    private var m3Sent = false
    private var channelObtained = false
    private var fragments = 0
    private var pendingWrites: [[UInt8]] = []
    private var afterWrites: (() -> Void)?
    private var ackWritesInFlight = 0
    private var waitTimer: Timer?
    private var messageTimer: Timer?
    private var gotFirstChunk = false
    private var ended = false

    // MARK: lifecycle

    func launch() {
        let args = CommandLine.arguments
        autoScan = args.contains("--autoscan")
        if let i = args.firstIndex(of: "--choose-file"), i + 1 < args.count { chooseFile = args[i + 1] }
        central = CBCentralManager(delegate: self, queue: nil)
        newCode()
    }

    private func say(_ s: String) {
        print(s); fflush(stdout)
        DispatchQueue.main.async { self.log.append(s); if self.log.count > 300 { self.log.removeFirst(100) } }
    }

    func newCode() {
        do {
            try KagiLink.requireMatchingABI()
            receiver = try Receiver(deviceID: Array("mac-receiver".utf8), claims: Array("macos-spike-claim".utf8))
            derivedFailures = 0
            refreshCode()
            say("Code created: \(utcStamp())")
            say("Enter the code shown in the window on the phone, press start, note the KL-XXXX tag.")
            status = "code ready; press Scan"
            if autoScan { startScan() }
        } catch {
            say("Core error: \(error)")
            status = "core error"
        }
    }

    @discardableResult
    private func refreshCode() -> Bool {
        guard let r = receiver, let code = try? r.code() else {
            codeText = "—"; codeActive = false; return false
        }
        codeText = String(decoding: code.copyBytes(), as: UTF8.self)
        codeActive = true
        return true
    }

    // MARK: scanning and selection

    func centralManagerDidUpdateState(_ c: CBCentralManager) {
        say("bluetooth state=\(c.state.rawValue) authorization=\(CBCentralManager.authorization.rawValue)")
    }

    func startScan() {
        guard central.state == .poweredOn else { say("Bluetooth is not powered on"); return }
        guard refreshCode() else { say("The code is no longer active (expired, used up or consumed)."); status = "code inactive"; return }
        adverts = [:]; rows = []; extraListens = 0
        phase = .scanning; busy = true; status = "scanning 10 s"
        central.scanForPeripherals(withServices: [klServiceUUID], options: [CBCentralManagerScanOptionAllowDuplicatesKey: true])
        DispatchQueue.main.asyncAfter(deadline: .now() + 10) { self.finishScan() }
    }

    private func finishScan() {
        guard phase == .scanning else { return }
        if extraListens < 3, adverts.values.contains(where: { $0.tag == nil }) {
            extraListens += 1
            say("An advertiser has not shown its tag yet; listening 3 s more ...")
            DispatchQueue.main.asyncAfter(deadline: .now() + 3) { self.finishScan() }
            return
        }
        central.stopScan()
        if adverts.isEmpty {
            say("No KagiLink senders found"); phase = .idle; busy = false; status = "nothing found"
            if autoScan, refreshCode() { startScan() }
            return
        }
        phase = .listing
        busy = false
        rebuildRows(final: true)
        status = "pick the row whose tag matches the phone"
        startChoosePolling()
    }

    func centralManager(_ c: CBCentralManager, didDiscover p: CBPeripheral, advertisementData ad: [String: Any], rssi RSSI: NSNumber) {
        guard phase == .scanning else { return }
        var a = adverts[p.identifier] ?? Advertiser(peripheral: p, nameTag: nil, dataTag: nil, rssi: RSSI.intValue)
        a.rssi = RSSI.intValue
        if let name = ad[CBAdvertisementDataLocalNameKey] as? String, name.count == 7, name.hasPrefix("KL-"),
           name.dropFirst(3).allSatisfy({ tagAlphabet.contains($0) }) {
            a.nameTag = String(name.dropFirst(3))
        }
        if let sd = (ad[CBAdvertisementDataServiceDataKey] as? [CBUUID: Data])?[klServiceUUID], sd.count == 4,
           let t = String(data: sd, encoding: .ascii), t.allSatisfy({ tagAlphabet.contains($0) }) {
            a.dataTag = t
        }
        adverts[p.identifier] = a
    }

    private func rebuildRows(final: Bool) {
        let list = adverts.values.sorted { $0.rssi > $1.rssi }
        var counts: [String: Int] = [:]
        for a in list { if let t = a.tag { counts[t, default: 0] += 1 } }
        rows = list.map { a in
            let signal = a.rssi > -60 ? "strong" : (a.rssi > -80 ? "medium" : "weak")
            guard let t = a.tag else { return Row(id: a.peripheral.identifier, title: "(no usable tag)  cannot choose", signalText: signal, selectable: false) }
            if a.conflict { return Row(id: a.peripheral.identifier, title: "KL-\(t)  cannot choose (tag forms disagree)", signalText: signal, selectable: false) }
            if counts[t, default: 0] > 1 { return Row(id: a.peripheral.identifier, title: "KL-\(t)  cannot choose (duplicate tag; restart the phone session)", signalText: signal, selectable: false) }
            return Row(id: a.peripheral.identifier, title: "KL-\(t)", signalText: signal, selectable: true)
        }
        if final { for (i, r) in rows.enumerated() { say("[\(i + 1)] \(r.title)  signal: \(r.signalText)") } }
    }

    private func startChoosePolling() {
        chooseTimer?.invalidate()
        guard let path = chooseFile else { return }
        chooseTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] t in
            guard let self, self.phase == .listing else { t.invalidate(); return }
            guard let s = try? String(contentsOfFile: path, encoding: .utf8) else { return }
            try? FileManager.default.removeItem(atPath: path)
            let typed = s.trimmingCharacters(in: .whitespacesAndNewlines).uppercased().replacingOccurrences(of: "KL-", with: "")
            t.invalidate()
            if let r = self.rows.first(where: { $0.title.hasPrefix("KL-\(typed)") && $0.selectable }) { self.choose(r.id) }
            else { self.say("No valid row chosen. Nothing was connected. (typed \(typed.count) characters, \(self.rows.count) rows listed)"); self.phase = .idle; self.status = "no valid choice" }
        }
    }

    func choose(_ id: UUID) {
        guard phase == .listing, let a = adverts[id], let row = rows.first(where: { $0.id == id }), row.selectable else { return }
        chooseTimer?.invalidate()
        resetAttempt()
        peripheral = a.peripheral
        a.peripheral.delegate = self
        phase = .connecting; busy = true; status = "connecting"
        central.connect(a.peripheral)
        armStage(10, "connect/discovery")
    }

    // MARK: attempt

    private func resetAttempt() {
        ended = false; m3Sent = false; channelObtained = false; fragments = 0; gotFirstChunk = false
        pendingWrites = []; afterWrites = nil; ackWritesInFlight = 0
        reassembler = nil; channel = nil; rxChar = nil; txChar = nil
        waitTimer?.invalidate(); messageTimer?.invalidate()
    }

    private func armStage(_ s: TimeInterval, _ what: String) {
        waitTimer?.invalidate()
        waitTimer = Timer.scheduledTimer(withTimeInterval: s, repeats: false) { [weak self] _ in self?.fail(.connect("\(what) timeout")) }
    }

    func centralManager(_ c: CBCentralManager, didConnect p: CBPeripheral) { p.discoverServices([klServiceUUID]) }
    func centralManager(_ c: CBCentralManager, didFailToConnect p: CBPeripheral, error: Error?) { fail(.connect("failed")) }
    func centralManager(_ c: CBCentralManager, didDisconnectPeripheral p: CBPeripheral, error: Error?) {
        if !ended && p.identifier == peripheral?.identifier { fail(.link) }
    }

    func peripheral(_ p: CBPeripheral, didDiscoverServices error: Error?) {
        guard error == nil, let s = p.services?.first(where: { $0.uuid == klServiceUUID }) else { return fail(.connect("service not found")) }
        p.discoverCharacteristics([klRxUUID, klTxUUID], for: s)
    }

    func peripheral(_ p: CBPeripheral, didDiscoverCharacteristicsFor s: CBService, error: Error?) {
        guard error == nil else { return fail(.connect("characteristics")) }
        rxChar = s.characteristics?.first { $0.uuid == klRxUUID }
        txChar = s.characteristics?.first { $0.uuid == klTxUUID }
        guard let tx = txChar, rxChar != nil else { return fail(.connect("characteristics missing")) }
        // Chunk sizing uses the withoutResponse value (spec section 5), capped at 244, never below 20.
        chunkSize = max(20, min(p.maximumWriteValueLength(for: .withoutResponse), 244))
        txNeedsAck = !tx.properties.contains(.indicate)
        p.setNotifyValue(true, for: tx)
    }

    func peripheral(_ p: CBPeripheral, didUpdateNotificationStateFor ch: CBCharacteristic, error: Error?) {
        guard error == nil, ch.isNotifying else { return fail(.connect("subscribe")) }
        waitTimer?.invalidate()
        guard let rx = receiver else { return fail(.core(.null)) }
        say("Attempt started: \(utcStamp())")
        say("link: chunk=\(chunkSize) ack-mode=\(txNeedsAck ? "notify" : "indicate")")
        do {
            reassembler = try FrameReassembler()
            let m1 = try rx.beginAttempt()
            phase = .handshakeM2
            send(mtype: Codec.mtypeHandshake, m1) { [weak self] in self?.armFirstChunk() }
        } catch let e as KagiLinkError { fail(.core(e.status)) } catch { fail(.core(.null)) }
    }

    private func send(mtype: UInt8, _ message: [UInt8], then: @escaping () -> Void) {
        do { pendingWrites = try Codec.chunk(mtype: mtype, message: message, maxChunk: chunkSize) }
        catch let e as KagiLinkError { return fail(.core(e.status)) } catch { return fail(.core(.null)) }
        afterWrites = then
        writeNext()
    }

    private func writeNext() {
        guard !ended, let p = peripheral, let rx = rxChar else { return }
        if pendingWrites.isEmpty { let f = afterWrites; afterWrites = nil; f?(); return }
        p.writeValue(Data(pendingWrites.removeFirst()), for: rx, type: .withResponse)
    }

    func peripheral(_ p: CBPeripheral, didWriteValueFor ch: CBCharacteristic, error: Error?) {
        if ackWritesInFlight > 0 { ackWritesInFlight -= 1; if error != nil { fail(.link) }; return }
        if error != nil { return fail(.link) }
        writeNext()
    }

    private func armFirstChunk() {
        gotFirstChunk = false
        messageTimer?.invalidate()
        waitTimer?.invalidate()
        waitTimer = Timer.scheduledTimer(withTimeInterval: firstChunkWait, repeats: false) { [weak self] _ in self?.fail(.timeout) }
    }

    func peripheral(_ p: CBPeripheral, didUpdateValueFor ch: CBCharacteristic, error: Error?) {
        guard !ended, error == nil, ch.uuid == klTxUUID, let data = ch.value, let re = reassembler else { return }
        let chunk = [UInt8](data)
        if !gotFirstChunk {
            gotFirstChunk = true
            waitTimer?.invalidate()
            messageTimer = Timer.scheduledTimer(withTimeInterval: messageComplete, repeats: false) { [weak self] _ in self?.fail(.timeout) }
        }
        let seq = chunk.count > 1 ? chunk[1] : nil
        let done: (mtype: UInt8, data: [UInt8])?
        do { done = try re.push(chunk) } catch let e as KagiLinkError { return fail(.core(e.status)) } catch { return fail(.core(.null)) }
        if txNeedsAck, let s = seq, let rxc = rxChar, let ack = try? Codec.ackChunk(seq: s) {
            ackWritesInFlight += 1
            p.writeValue(Data(ack), for: rxc, type: .withResponse)
        }
        if let m = done {
            messageTimer?.invalidate()
            gotFirstChunk = false
            handle(mtype: m.mtype, data: m.data)
        }
    }

    private func handle(mtype: UInt8, data: [UInt8]) {
        guard let rx = receiver else { return fail(.core(.null)) }
        do {
            switch phase {
            case .handshakeM2:
                guard mtype == Codec.mtypeHandshake else { return fail(.unexpectedMessage) }
                let m3 = try rx.handleM2(data)
                m3Sent = true
                phase = .handshakeM4
                send(mtype: Codec.mtypeHandshake, m3) { [weak self] in self?.armFirstChunk() }
            case .handshakeM4:
                guard mtype == Codec.mtypeHandshake else { return fail(.unexpectedMessage) }
                let ch = try rx.finish(m4: data)
                channel = ch; channelObtained = true
                try ch.setReadyToReceive(true)
                phase = .secret
                armFirstChunk()
            case .secret:
                guard mtype == Codec.mtypeChannel, let ch = channel else { try? channel?.close(); return fail(.unexpectedMessage) }
                fragments += 1
                guard let msg = try ch.open(fragment: data) else { return armFirstChunk() }
                guard msg.kind == .secret else { try? ch.close(); return fail(.unexpectedKind) }
                let n = msg.plaintext.count
                let report = Array("received:\(n)".utf8)
                let frags = try ch.seal(.result, plaintext: report, maxFragmentPayload: 200)
                sendResult(frags, ch: ch, received: n)
            default: break
            }
        } catch let e as KagiLinkError { fail(.core(e.status)) } catch { fail(.core(.null)) }
    }

    private func sendResult(_ frags: [[UInt8]], ch: Channel, received: Int) {
        var rest = frags
        func next() {
            if rest.isEmpty { try? ch.close(); return succeed(received: received) }
            let f = rest.removeFirst()
            send(mtype: Codec.mtypeChannel, f) { next() }
        }
        next()
    }

    // MARK: outcome

    private func succeed(received: Int) {
        guard !ended else { return }
        ended = true
        cleanup()
        say("Failure budget used: \(derivedFailures)/\(maxFailures) (derived by this driver: the C ABI has no counter query)")
        say("Transfer OK: \(received) bytes received in \(fragments) fragments")
        finishAttempt(ok: true)
    }

    private func fail(_ e: AttemptError) {
        guard !ended else { return }
        ended = true
        cleanup()
        if m3Sent && !channelObtained { derivedFailures += 1 }
        say("Failure budget used: \(derivedFailures)/\(maxFailures) (derived by this driver: counts attempts that sent M3 and did not finish; the C ABI has no counter query)")
        say("Attempt failed: \(e.text)")
        finishAttempt(ok: false)
    }

    private func cleanup() {
        waitTimer?.invalidate(); messageTimer?.invalidate()
        try? channel?.close()
        if let p = peripheral { central.cancelPeripheralConnection(p) }
    }

    private func finishAttempt(ok: Bool) {
        phase = .finished; busy = false
        channel = nil; reassembler = nil
        let active = refreshCode()
        if ok { status = "transfer OK (synthetic)"; codeText = "—"; return }
        if !active || derivedFailures >= maxFailures {
            say("The code is no longer active (expired, used up or consumed). core code() active=\(active)")
            codeText = "—"; codeActive = false; status = "code no longer active"
            return
        }
        status = "failed; same code still active: scan again"
        phase = .idle
        if autoScan { DispatchQueue.main.asyncAfter(deadline: .now() + 1) { self.startScan() } }
    }
}
