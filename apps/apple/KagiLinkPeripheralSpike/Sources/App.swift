import SwiftUI
#if canImport(KagiLinkRiskAck)
import KagiLinkRiskAck
#endif

@main
struct SpikeApp: App {
    @StateObject private var spike = PeripheralSpike()
    @Environment(\.scenePhase) private var phase
    @State private var code = ""
    // D023 risk acknowledgement: shown before the code entry/Start step, before EVERY transfer
    // (no stable receiver identity yet). Separate from, and never a substitute for, the per-transfer approval.
    @StateObject private var riskGate = RiskAckGate()

    var body: some Scene {
        WindowGroup {
            Group {
            if riskGate.state == .acknowledged || spike.running {
            VStack(alignment: .leading, spacing: 12) {
                Text("KL-V1 peripheral spike (synthetic)").font(.headline)
                Text("KL-\(spike.tag)").font(.system(size: 48, weight: .bold, design: .monospaced))
#if FAULT_INJECTION
                VStack(alignment: .leading, spacing: 4) {
                    Text("FAULT TEST BUILD — mode: \(spike.fault.rawValue)").font(.caption.bold()).foregroundStyle(.red)
                    Picker("Fault mode", selection: $spike.fault) { ForEach(FaultMode.allCases) { Text($0.rawValue).tag($0) } }
                        .pickerStyle(.menu).disabled(spike.running)
                }
#endif
                TextField("6-digit code from the receiver", text: $code).keyboardType(.numberPad).textFieldStyle(.roundedBorder)
                HStack {
                    Button("Start") {
                        // The acknowledgement is single use: no Start without it, and the next transfer asks again.
                        guard riskGate.consumeForTransfer() else { return }
                        spike.start(codeDigits: code); code = ""
                    }.disabled(spike.running || code.count != 6)
                    Button("Stop") { spike.stop("manual") }.disabled(!spike.running)
                }.buttonStyle(.borderedProminent)
                ScrollView { Text(spike.log.joined(separator: "\n")).font(.caption.monospaced()).frame(maxWidth: .infinity, alignment: .leading) }
            }
            .padding(16)
            } else {
                RiskAckView(gate: riskGate, onContinue: {}, onCancel: { riskGate.reset(); riskGate.evaluate() })
            }
            }
            .onAppear { if riskGate.state == .notEvaluated { riskGate.evaluate() } }
            // Back to the notice whenever the acknowledgement was used up (a transfer started) and nothing is running.
            .onChange(of: riskGate.state) { _, new in if new == .notEvaluated && !spike.running { riskGate.evaluate() } }
            .onChange(of: spike.running) { _, running in if !running && riskGate.state == .notEvaluated { riskGate.evaluate() } }
#if FAULT_INJECTION
            // Test build only: keep the screen on while the app is in the foreground, and remember the
            // selected fault mode (never the code) between launches to reduce taps.
            .onAppear {
                UIApplication.shared.isIdleTimerDisabled = true
                if let raw = UserDefaults.standard.string(forKey: "faultMode"), let m = FaultMode(rawValue: raw) { spike.fault = m }
            }
            .onChange(of: spike.fault) { _, m in UserDefaults.standard.set(m.rawValue, forKey: "faultMode") }
#endif
            .onChange(of: phase) { _, new in
#if FAULT_INJECTION
                UIApplication.shared.isIdleTimerDisabled = (new == .active)
#endif
                spike.noteScene("\(new)")
            }
        }
    }
}
