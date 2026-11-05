// FFI handle registry (PR #30): stale, double-freed and garbage handles must fail closed. Synthetic data only.
import XCTest
import KagiLinkC
@testable import KagiLinkCore

final class StaleHandleTests: XCTestCase {
    private func makeReceiver() throws -> OpaquePointer {
        let (st, h) = Native.receiverNew(deviceID: Array("s-rx-device".utf8), claims: Array("claims".utf8))
        XCTAssertEqual(st, .ok)
        return try XCTUnwrap(h)
    }

    func testUseAfterFreeAndDoubleFreeFailClosed() throws {
        let h = try makeReceiver()
        Native.receiverFree(h)
        Native.receiverFree(h)                       // second free: no crash
        let (st, secret) = Native.receiverCode(h)    // use after free
        XCTAssertEqual(st, .null)
        XCTAssertNil(secret)
        XCTAssertEqual(Native.receiverBeginAttempt(h).0, .null)
    }

    func testGarbageHandlesAreRejected() {
        for raw in [UInt(1), 0xdead_beef, UInt.max] {
            let bogus = OpaquePointer(bitPattern: raw)!
            XCTAssertEqual(Native.receiverCode(bogus).0, .null)
            Native.receiverFree(bogus)               // no crash
        }
    }

    func testStaleHandleDoesNotReachNewObject() throws {
        let old = try makeReceiver()
        Native.receiverFree(old)
        let fresh = try makeReceiver()
        defer { Native.receiverFree(fresh) }
        // The old token must stay dead even if a new object was created since.
        XCTAssertEqual(Native.receiverCode(old).0, .null)
        XCTAssertEqual(Native.receiverCode(fresh).0, .ok)
    }

    func testWrapperDeinitFreesOnce() throws {
        do {
            let r = try Receiver(deviceID: Array("s-rx-device".utf8), claims: Array("claims".utf8))
            _ = try r.code()
        }                                            // deinit frees exactly once, no crash
        let r2 = try Receiver(deviceID: Array("s-rx-device".utf8), claims: Array("claims".utf8))
        XCTAssertNoThrow(try r2.code())
    }
}
