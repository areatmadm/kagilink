import SwiftUI

@main
struct SpikeApp: App {
    @StateObject private var spike = PeripheralSpike()
    @Environment(\.scenePhase) private var phase
    @State private var code = ""

    var body: some Scene {
        WindowGroup {
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
                    Button("Start") { spike.start(codeDigits: code); code = "" }.disabled(spike.running || code.count != 6)
                    Button("Stop") { spike.stop("manual") }.disabled(!spike.running)
                }.buttonStyle(.borderedProminent)
                ScrollView { Text(spike.log.joined(separator: "\n")).font(.caption.monospaced()).frame(maxWidth: .infinity, alignment: .leading) }
            }
            .padding(16)
            .onChange(of: phase) { _, new in spike.noteScene("\(new)") }
        }
    }
}
