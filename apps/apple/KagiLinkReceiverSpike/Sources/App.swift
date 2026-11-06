import SwiftUI

@main
struct ReceiverSpikeApp: App {
    @StateObject private var rx = ReceiverSpike()

    var body: some Scene {
        WindowGroup {
            VStack(alignment: .leading, spacing: 10) {
                Text("KL-V1 macOS receiver spike (synthetic data, no real secrets)").font(.headline)
                Text("Code (valid 60 s) — type it on the phone:").font(.caption)
                Text(rx.codeText).font(.system(size: 44, weight: .bold, design: .monospaced))
                Text(rx.status).font(.callout)
                HStack {
                    Button("New code") { rx.newCode() }.disabled(rx.busy)
                    Button("Scan (10 s)") { rx.startScan() }.disabled(rx.busy || !rx.codeActive)
                }
                ForEach(rx.rows) { r in
                    Button("\(r.title)   signal: \(r.signalText)") { rx.choose(r.id) }
                        .disabled(!r.selectable)
                }
                ScrollView { Text(rx.log.joined(separator: "\n")).font(.caption.monospaced())
                    .frame(maxWidth: .infinity, alignment: .leading) }
            }
            .padding(16).frame(minWidth: 560, minHeight: 480)
            .onAppear { rx.launch() }
        }
    }
}
