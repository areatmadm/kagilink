import XCTest
@testable import KagiLinkRiskAck

final class NeedsNoticeTests: XCTestCase {
    func testFirstUseShowsNotice() {
        XCTAssertTrue(needsNotice(storedVersion: nil, storedReceiverHash: nil, currentVersion: 1, currentReceiverHash: "aa"))
    }

    func testSameVersionAndStableReceiverSkips() {
        XCTAssertFalse(needsNotice(storedVersion: 1, storedReceiverHash: "aa", currentVersion: 1, currentReceiverHash: "aa"))
    }

    func testChangedReceiverShows() {
        XCTAssertTrue(needsNotice(storedVersion: 1, storedReceiverHash: "aa", currentVersion: 1, currentReceiverHash: "bb"))
    }

    func testBumpedVersionShows() {
        XCTAssertTrue(needsNotice(storedVersion: 1, storedReceiverHash: "aa", currentVersion: 2, currentReceiverHash: "aa"))
    }

    func testOlderStoredVersionShows() {
        XCTAssertTrue(needsNotice(storedVersion: 3, storedReceiverHash: "aa", currentVersion: 2, currentReceiverHash: "aa"))
    }

    func testUnknownCurrentIdentityAlwaysShows() {
        XCTAssertTrue(needsNotice(storedVersion: 1, storedReceiverHash: nil, currentVersion: 1, currentReceiverHash: nil))
        XCTAssertTrue(needsNotice(storedVersion: 1, storedReceiverHash: "aa", currentVersion: 1, currentReceiverHash: nil))
        XCTAssertTrue(needsNotice(storedVersion: 1, storedReceiverHash: "", currentVersion: 1, currentReceiverHash: ""))
    }

    func testStoredWithoutReceiverHashShows() {
        XCTAssertTrue(needsNotice(storedVersion: 1, storedReceiverHash: nil, currentVersion: 1, currentReceiverHash: "aa"))
    }

    func testCorruptRecordDecodesToNilAndShows() {
        let corrupt = Data("not json".utf8)
        XCTAssertNil(AckRecordCodec.decode(corrupt))
        let stored = AckRecordCodec.decode(corrupt)
        XCTAssertTrue(needsNotice(storedVersion: stored?.noticeVersion, storedReceiverHash: stored?.receiverHash,
                                  currentVersion: 1, currentReceiverHash: "aa"))
    }

    func testReceiverHashIsStableHexAndNeverTheRawClaim() {
        let hash = ReceiverIdentity.claimed("synthetic-receiver-1").hash
        XCTAssertEqual(hash?.count, 64)
        XCTAssertEqual(hash, ReceiverIdentity.claimed("synthetic-receiver-1").hash)
        XCTAssertNotEqual(hash, ReceiverIdentity.claimed("synthetic-receiver-2").hash)
        XCTAssertFalse(hash?.contains("synthetic") ?? true)
        XCTAssertNil(ReceiverIdentity.unknown.hash)
        XCTAssertNil(ReceiverIdentity.claimed("").hash)
    }

    func testRecordHoldsOnlyVersionHashAndTimestamp() throws {
        let record = AckRecord(noticeVersion: 1, receiverHash: "aa", acknowledgedAt: Date(timeIntervalSince1970: 0))
        let json = try JSONSerialization.jsonObject(with: AckRecordCodec.encode(record)) as? [String: Any]
        XCTAssertEqual(Set(json.map { Array($0.keys) } ?? []), ["noticeVersion", "receiverHash", "acknowledgedAt"])
    }
}
