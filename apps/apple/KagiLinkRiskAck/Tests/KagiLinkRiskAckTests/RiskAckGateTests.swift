import XCTest
@testable import KagiLinkRiskAck

private final class MemoryStorage: AckStorage, @unchecked Sendable {
    var data: Data?
    var failLoad = false
    var failSave = false
    struct Boom: Error {}
    func load() throws -> Data? { if failLoad { throw Boom() }; return data }
    func save(_ data: Data) throws { if failSave { throw Boom() }; self.data = data }
}

private struct MissingSource: NoticeTextSource {
    func text(for language: NoticeLanguage) -> NoticeText? { nil }
}

@MainActor
final class RiskAckGateTests: XCTestCase {
    private func makeGate(_ storage: MemoryStorage = MemoryStorage(), source: NoticeTextSource = BuiltInNoticeTextSource(),
                          version: Int = 1) -> RiskAckGate {
        RiskAckGate(source: source, storage: storage, version: version, now: { Date(timeIntervalSince1970: 1_000) })
    }

    // UI behaviour at view-model level: the Continue button is `.disabled(!canContinue)`.
    func testContinueDisabledUntilCheckedAndTogglesBack() {
        let gate = makeGate()
        gate.evaluate(language: .en)
        XCTAssertEqual(gate.state, .needsNotice)
        XCTAssertFalse(gate.checked, "box starts unchecked")
        XCTAssertFalse(gate.canContinue)
        gate.checked = true
        XCTAssertTrue(gate.canContinue)
        gate.checked = false
        XCTAssertFalse(gate.canContinue)
    }

    func testContinueWhileUncheckedDoesNothing() {
        let gate = makeGate()
        gate.evaluate(language: .ko)
        gate.continueTapped()
        XCTAssertEqual(gate.state, .needsNotice)
    }

    func testScreenAppearsBeforeTransferCanStart() {
        let gate = makeGate()
        XCTAssertFalse(gate.consumeForTransfer(), "no transfer before evaluate")
        gate.evaluate(language: .en)
        XCTAssertFalse(gate.consumeForTransfer(), "no transfer while the notice is pending")
        gate.checked = true
        gate.continueTapped()
        XCTAssertTrue(gate.consumeForTransfer())
    }

    func testAcknowledgementIsSingleUseEvenWithSameReceiver() {
        let storage = MemoryStorage()
        let gate = makeGate(storage)
        let receiver = ReceiverIdentity.claimed("synthetic-receiver")
        gate.evaluate(receiver: receiver, language: .en)
        gate.checked = true
        gate.continueTapped()
        XCTAssertTrue(gate.consumeForTransfer())
        XCTAssertFalse(gate.consumeForTransfer(), "second use of the same acknowledgement is refused")
        gate.evaluate(receiver: receiver, language: .en)
        XCTAssertEqual(gate.state, .acknowledged, "stable identity + same version may skip the screen")
    }

    func testUnknownReceiverShowsBeforeEveryTransfer() {
        let storage = MemoryStorage()
        let gate = makeGate(storage)
        for _ in 0..<3 {
            gate.evaluate(language: .en)
            XCTAssertEqual(gate.state, .needsNotice)
            XCTAssertFalse(gate.checked)
            gate.checked = true
            gate.continueTapped()
            XCTAssertTrue(gate.consumeForTransfer())
        }
    }

    func testBackLeavesNoAcknowledgedState() {
        let gate = makeGate()
        gate.evaluate(language: .en)
        gate.checked = true
        gate.continueTapped()
        XCTAssertEqual(gate.state, .acknowledged)
        gate.reset()
        XCTAssertFalse(gate.checked)
        XCTAssertFalse(gate.consumeForTransfer())
    }

    func testReevaluateAlwaysStartsUnchecked() {
        let gate = makeGate()
        gate.evaluate(language: .en)
        gate.checked = true
        gate.evaluate(language: .en)
        XCTAssertFalse(gate.checked)
    }

    func testChangedReceiverAndBumpedVersionShowAgain() {
        let storage = MemoryStorage()
        let first = makeGate(storage, version: 1)
        first.evaluate(receiver: .claimed("a"), language: .en)
        first.checked = true
        first.continueTapped()

        let otherReceiver = makeGate(storage, version: 1)
        otherReceiver.evaluate(receiver: .claimed("b"), language: .en)
        XCTAssertEqual(otherReceiver.state, .needsNotice)

        let newVersion = makeGate(storage, version: 2)
        newVersion.evaluate(receiver: .claimed("a"), language: .en)
        XCTAssertEqual(newVersion.state, .needsNotice)
    }

    // Failure tests: each ends in "show the screen" or "no transfer", never a silent skip.
    func testMissingResourceBlocksTransfer() {
        let gate = makeGate(source: MissingSource())
        gate.evaluate(language: .en)
        XCTAssertEqual(gate.state, .blocked(.textUnavailable))
        gate.checked = true
        XCTAssertFalse(gate.canContinue)
        gate.continueTapped()
        XCTAssertFalse(gate.consumeForTransfer())
    }

    func testCorruptStorageShowsScreen() {
        let storage = MemoryStorage()
        storage.data = Data("garbage".utf8)
        let gate = makeGate(storage)
        gate.evaluate(receiver: .claimed("a"), language: .en)
        XCTAssertEqual(gate.state, .needsNotice)
    }

    func testUnreadableStorageShowsScreen() {
        let storage = MemoryStorage()
        storage.failLoad = true
        let gate = makeGate(storage)
        gate.evaluate(receiver: .claimed("a"), language: .en)
        XCTAssertEqual(gate.state, .needsNotice)
    }

    func testVersionMismatchShowsScreen() {
        let storage = MemoryStorage()
        storage.data = try? AckRecordCodec.encode(AckRecord(noticeVersion: 0, receiverHash: ReceiverIdentity.claimed("a").hash,
                                                            acknowledgedAt: Date(timeIntervalSince1970: 0)))
        let gate = makeGate(storage, version: 1)
        gate.evaluate(receiver: .claimed("a"), language: .en)
        XCTAssertEqual(gate.state, .needsNotice)
    }

    func testSaveFailureBlocksTransfer() {
        let storage = MemoryStorage()
        storage.failSave = true
        let gate = makeGate(storage)
        gate.evaluate(language: .en)
        gate.checked = true
        gate.continueTapped()
        XCTAssertEqual(gate.state, .blocked(.storageWriteFailed))
        XCTAssertFalse(gate.consumeForTransfer())
    }

    func testStoredRecordHasVersionHashAndTimestamp() throws {
        let storage = MemoryStorage()
        let gate = makeGate(storage)
        gate.evaluate(receiver: .claimed("a"), language: .en)
        gate.checked = true
        gate.continueTapped()
        let record = try XCTUnwrap(AckRecordCodec.decode(storage.data))
        XCTAssertEqual(record.noticeVersion, 1)
        XCTAssertEqual(record.receiverHash, ReceiverIdentity.claimed("a").hash)
        XCTAssertEqual(record.acknowledgedAt, Date(timeIntervalSince1970: 1_000))
    }
}
