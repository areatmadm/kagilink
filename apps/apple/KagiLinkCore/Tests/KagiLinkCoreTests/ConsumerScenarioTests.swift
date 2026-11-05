// Swift port of tests/c/consumer.c scenarios (D021). Synthetic data only.
// Failure counting follows D026: only attempts that already sent M3 use the three-failure budget.
import XCTest
import KagiLinkC
@testable import KagiLinkCore

private let rxID = Array("s-rx-device".utf8)
private let txID = Array("s-tx-device".utf8)
private let purpose = Array("synthetic-login".utf8)
private let claims = Array("claims".utf8)

private struct Flow {
    let rx: Receiver
    let code: [UInt8]
    var tx: Sender?
    var m3: [UInt8] = []

    /// Runs M1..M3 with the real code, or with `codeOverride` for the sender.
    static func toM3(codeOverride: [UInt8]? = nil) throws -> Flow {
        let rx = try Receiver(deviceID: rxID, claims: claims)
        let code = try rx.code().copyBytes()
        let m1 = try rx.beginAttempt()
        let tx = try Sender(code: codeOverride ?? code, m1: m1, deviceID: txID, purpose: purpose, claims: claims)
        var flow = Flow(rx: rx, code: code, tx: tx)
        flow.m3 = try rx.handleM2(tx.m2)
        return flow
    }
}

final class ConsumerScenarioTests: XCTestCase {
    func testABIAndWipe() throws {
        XCTAssertEqual(KagiLink.abiVersion, UInt32(KL_ABI_VERSION))
        try KagiLink.requireMatchingABI()
        var z: [UInt8] = [1, 2, 3, 4]
        Native.wipe(&z)
        XCTAssertEqual(z, [0, 0, 0, 0])
        kl_secure_wipe(nil, 0) // must not crash
        let secret = try Receiver(deviceID: rxID, claims: claims).code()
        XCTAssertEqual(secret.count, 6)
        secret.wipe()
        XCTAssertEqual(secret.count, 0)
    }

    func testFragmentedSecretRoundTripAndApprovalConsumed() throws {
        let flow = try Flow.toM3()
        let (m4, txCh) = try flow.tx!.finish(m3: flow.m3)
        let rxCh = try flow.rx.finish(m4: m4)
        let secret = Array("synthetic-secret-0123456789".utf8)

        // SECRET needs sender approval (and receiver readiness).
        XCTAssertThrowsError(try txCh.seal(.secret, plaintext: secret, maxFragmentPayload: 8)) {
            XCTAssertEqual(($0 as? KagiLinkError)?.status, .notApproved)
        }
        try txCh.approveOneTransfer()
        try rxCh.setReadyToReceive(true)
        let fragments = try txCh.seal(.secret, plaintext: secret, maxFragmentPayload: 8)
        XCTAssertGreaterThan(fragments.count, 1) // really fragmented

        var received: (kind: MessageKind, plaintext: SecretBuffer)?
        for (i, fragment) in fragments.enumerated() {
            let result = try rxCh.open(fragment: fragment)
            if i + 1 == fragments.count {
                received = result
            } else {
                XCTAssertNil(result)
            }
        }
        let message = try XCTUnwrap(received)
        XCTAssertEqual(message.kind, .secret)
        XCTAssertEqual(message.plaintext.copyBytes(), secret)
        message.plaintext.wipe()

        // Approval is consumed: a second SECRET is refused.
        XCTAssertThrowsError(try txCh.seal(.secret, plaintext: secret, maxFragmentPayload: 8)) {
            XCTAssertEqual(($0 as? KagiLinkError)?.status, .notApproved)
        }
    }

    func testWrongCodeAndFailureBudget() throws {
        let rx = try Receiver(deviceID: rxID, claims: claims)
        var wrong = try rx.code().copyBytes()
        wrong[0] = wrong[0] == UInt8(ascii: "0") ? UInt8(ascii: "1") : UInt8(ascii: "0")
        let m1 = try rx.beginAttempt()
        let tx = try Sender(code: wrong, m1: m1, deviceID: txID, purpose: purpose, claims: claims)
        let m3 = try rx.handleM2(tx.m2)
        XCTAssertThrowsError(try tx.finish(m3: m3)) {
            XCTAssertEqual(($0 as? KagiLinkError)?.status, .confirmationFailed)
        }
        // Pending state is consumed even on failure.
        XCTAssertThrowsError(try tx.finish(m3: m3)) {
            XCTAssertNotEqual(($0 as? KagiLinkError)?.status, .ok)
        }

        // D026: failure 1 is the attempt above (M3 sent, then a bogus M4); two more full attempts
        // that reach M3 and then get a bogus M4 destroy the code.
        var bogus = [UInt8](repeating: 0, count: 33)
        bogus[0] = 0x14
        for i in 0..<3 {
            if i > 0 {
                let m1b = try rx.beginAttempt()
                let s2 = try Sender(code: wrong, m1: m1b, deviceID: txID, purpose: purpose, claims: claims)
                _ = try rx.handleM2(s2.m2)
            }
            XCTAssertThrowsError(try rx.finish(m4: bogus))
        }
        XCTAssertThrowsError(try rx.code()) {
            XCTAssertEqual(($0 as? KagiLinkError)?.status, .tooManyFailures)
        }
    }

    func testPreM3MalformedInputCostsNothing() throws {
        // D026: a malformed M2 (never reached M3) must not spend the failure budget.
        let rx = try Receiver(deviceID: rxID, claims: claims)
        for _ in 0..<5 {
            _ = try rx.beginAttempt()
            XCTAssertThrowsError(try rx.handleM2([0x00, 0x01, 0x02]))
        }
        XCTAssertNoThrow(try rx.code())
    }

    func testTamperClosesChannel() throws {
        let flow = try Flow.toM3()
        let (m4, txCh) = try flow.tx!.finish(m3: flow.m3)
        let rxCh = try flow.rx.finish(m4: m4)
        var fragments = try txCh.seal(.control, plaintext: Array("synthetic-control".utf8), maxFragmentPayload: 256)
        XCTAssertEqual(fragments.count, 1)
        fragments[0][fragments[0].count - 1] ^= 0x01 // flip a tag bit
        XCTAssertThrowsError(try rxCh.open(fragment: fragments[0]))
        XCTAssertTrue(rxCh.isClosed)
    }

    func testBadCodeLength() throws {
        let rx = try Receiver(deviceID: rxID, claims: claims)
        let m1 = try rx.beginAttempt()
        XCTAssertThrowsError(
            try Sender(code: Array("12345".utf8), m1: m1, deviceID: txID, purpose: purpose, claims: claims)
        ) { XCTAssertEqual(($0 as? KagiLinkError)?.status, .badCode) }
    }

    /// Direct C-level checks that the Swift wrapper cannot express (NULL pointers, size queries).
    func testNullAndSmallBuffersAtCLevel() {
        var r: OpaquePointer?
        XCTAssertEqual(kl_receiver_new(nil, 1, nil, 0, &r), KL_ERR_NULL)
        rxID.withUnsafeBufferPointer { id in
            claims.withUnsafeBufferPointer { cl in
                XCTAssertEqual(kl_receiver_new(id.baseAddress, id.count, cl.baseAddress, cl.count, nil), KL_ERR_NULL)
                XCTAssertEqual(kl_receiver_new(id.baseAddress, id.count, cl.baseAddress, cl.count, &r), KL_OK)
            }
        }
        var small = [UInt8](repeating: 0, count: 2)
        var n = small.count
        XCTAssertEqual(kl_receiver_code(r, &small, &n), KL_ERR_BUFFER)
        XCTAssertEqual(n, 6) // required size reported, nothing consumed
        var code = [UInt8](repeating: 0, count: 6)
        n = code.count
        XCTAssertEqual(kl_receiver_code(r, &code, &n), KL_OK)
        XCTAssertEqual(n, 6)
        XCTAssertEqual(kl_receiver_code(nil, &code, &n), KL_ERR_NULL)
        kl_secure_wipe(&code, code.count)
        kl_receiver_free(r)
        kl_receiver_free(nil)
        kl_sender_free(nil)
        kl_channel_free(nil)
        XCTAssertEqual(kl_channel_is_closed(nil), 1)
    }

    /// The growing-output helper must recover from a too-small first buffer using the size the
    /// library reports (a real kl_receiver_begin_attempt, M1 is far larger than 4 bytes).
    func testGrowingOutputRetriesOnSmallBuffer() {
        var r: OpaquePointer?
        rxID.withUnsafeBufferPointer { id in
            claims.withUnsafeBufferPointer { cl in
                XCTAssertEqual(kl_receiver_new(id.baseAddress, id.count, cl.baseAddress, cl.count, &r), KL_OK)
            }
        }
        defer { kl_receiver_free(r) }
        var calls = 0
        let (st, m1) = Native.withGrowingOutput(initial: 4) { out, len in
            calls += 1
            return Int32(truncatingIfNeeded: kl_receiver_begin_attempt(r, out, &len).rawValue)
        }
        XCTAssertEqual(st, .ok)
        XCTAssertEqual(calls, 2) // first call: KL_ERR_BUFFER with the required size, second succeeds
        XCTAssertGreaterThan(m1.count, 4)
    }
}
