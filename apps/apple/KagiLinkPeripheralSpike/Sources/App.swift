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
