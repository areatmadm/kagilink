import Foundation

/// State machine in front of the transfer start. Fails closed: only `.acknowledged` lets a transfer start, and
/// `consumeForTransfer()` uses that state up, so one acknowledgement never carries over to a second attempt.
/// This is separate from the per-transfer approval (AGENTS.md) and never replaces it.
@MainActor
public final class RiskAckGate: ObservableObject {
    public enum BlockReason: Equatable, Sendable { case textUnavailable, storageWriteFailed }

    public enum State: Equatable, Sendable {
        case notEvaluated
        case needsNotice
        case acknowledged
        case blocked(BlockReason)
    }

    @Published public private(set) var state: State = .notEvaluated
    /// Starts unchecked on every evaluation; never pre-checked, no timer, no "do not show again".
    @Published public var checked = false
    @Published public private(set) var text: NoticeText?

    private let source: NoticeTextSource
    private let storage: AckStorage
    private let version: Int
    private let now: () -> Date
    private var receiver: ReceiverIdentity = .unknown

    public init(source: NoticeTextSource = BuiltInNoticeTextSource(),
                storage: AckStorage = UserDefaultsAckStorage(),
                version: Int = RiskNotice.version,
                now: @escaping () -> Date = Date.init) {
        self.source = source
        self.storage = storage
        self.version = version
        self.now = now
    }

    /// The Continue button is enabled only while the screen is up, the text loaded and the box is checked.
    public var canContinue: Bool { state == .needsNotice && checked && text != nil }

    public func evaluate(receiver: ReceiverIdentity = .unknown,
                         language: NoticeLanguage = NoticeLanguage.current()) {
        self.receiver = receiver
        checked = false
        guard let loaded = source.text(for: language), !loaded.shortLines.isEmpty,
              !loaded.checkboxLabel.isEmpty else {
            text = nil
            state = .blocked(.textUnavailable)
            return
        }
        text = loaded
        // A read error or corrupt data is "no record": show the screen, never skip it silently.
        let stored = AckRecordCodec.decode((try? storage.load()) ?? nil)
        let show = needsNotice(storedVersion: stored?.noticeVersion, storedReceiverHash: stored?.receiverHash,
                               currentVersion: version, currentReceiverHash: receiver.hash)
        state = show ? .needsNotice : .acknowledged
    }

    public func continueTapped() {
        guard canContinue else { return }
        let record = AckRecord(noticeVersion: version, receiverHash: receiver.hash, acknowledgedAt: now())
        do {
            try storage.save(AckRecordCodec.encode(record))
            state = .acknowledged
        } catch {
            checked = false
            state = .blocked(.storageWriteFailed)
        }
    }

    /// True once, and only from `.acknowledged`. The next transfer must evaluate and acknowledge again.
    public func consumeForTransfer() -> Bool {
        guard state == .acknowledged else { return false }
        state = .notEvaluated
        checked = false
        return true
    }

    /// Back/cancel: leaves no acknowledged state behind.
    public func reset() {
        state = .notEvaluated
        checked = false
    }
}
