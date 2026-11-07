import SwiftUI

/// The acknowledgement screen. Wording comes only from `NoticeText`; the receiver and any TPM/device result
/// are claims and are never described as safe or verified here.
public struct RiskAckView: View {
    @ObservedObject private var gate: RiskAckGate
    private let onContinue: () -> Void
    private let onCancel: () -> Void
    @State private var showingFull = false

    public init(gate: RiskAckGate, onContinue: @escaping () -> Void, onCancel: @escaping () -> Void) {
        self.gate = gate
        self.onContinue = onContinue
        self.onCancel = onCancel
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if let text = gate.text {
                    notice(text)
                } else {
                    Text(Self.unavailableMessage()).font(.body)
                    Button(Self.cancelLabel(), action: onCancel).buttonStyle(.bordered)
                }
            }
            .padding(16)
        }
        .onChange(of: gate.state) { _, state in
            if state == .acknowledged { onContinue() }
        }
    }

    @ViewBuilder private func notice(_ text: NoticeText) -> some View {
        Text(text.draftBanner).font(.footnote.bold()).foregroundStyle(.secondary)
        ForEach(Array(text.shortLines.enumerated()), id: \.offset) { _, line in
            Text(line).font(.title3).fixedSize(horizontal: false, vertical: true)
        }
        Button(text.ui.viewFull) { showingFull = true }
            .sheet(isPresented: $showingFull) { FullNoticeView(text: text) }
        checkbox(text)
        if case .blocked(let reason) = gate.state {
            Text(reason == .storageWriteFailed ? text.ui.storageFailed : text.ui.unavailable)
                .font(.callout.bold()).foregroundStyle(.red)
        }
        HStack {
            Button(text.ui.cancelButton, action: onCancel).buttonStyle(.bordered)
            Button(text.ui.continueButton) { gate.continueTapped() }
                .buttonStyle(.borderedProminent)
                .disabled(!gate.canContinue)
        }
    }

    /// A button, not a Toggle switch, so the box shape plus checkmark (not colour alone) shows the state.
    private func checkbox(_ text: NoticeText) -> some View {
        Button { gate.checked.toggle() } label: {
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: gate.checked ? "checkmark.square.fill" : "square").font(.title2)
                Text(text.checkboxLabel).multilineTextAlignment(.leading)
            }
        }
        .buttonStyle(.plain)
        .accessibilityLabel(text.checkboxLabel)
        .accessibilityHint(text.ui.checkboxHint)
        .accessibilityAddTraits(.isToggle)
        .accessibilityValue(gate.checked ? "1" : "0")
    }

    private static func unavailableMessage() -> String {
        NoticeLanguage.current() == .ko ? NoticeResource.ko.ui.unavailable : NoticeResource.en.ui.unavailable
    }

    private static func cancelLabel() -> String {
        NoticeLanguage.current() == .ko ? NoticeResource.ko.ui.cancelButton : NoticeResource.en.ui.cancelButton
    }
}

struct FullNoticeView: View {
    let text: NoticeText
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    Text(text.draftBanner).font(.footnote.bold()).foregroundStyle(.secondary)
                    Text(Self.attributed(text.title)).font(.headline)
                    ForEach(Array(text.fullItems.enumerated()), id: \.offset) { _, item in
                        Text(Self.attributed(item))
                    }
                }
                .padding(16)
            }
            .navigationTitle(text.ui.fullTitle)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button(text.ui.close) { dismiss() } } }
        }
    }

    private static func attributed(_ markdown: String) -> AttributedString {
        (try? AttributedString(markdown: markdown)) ?? AttributedString(markdown)
    }
}
