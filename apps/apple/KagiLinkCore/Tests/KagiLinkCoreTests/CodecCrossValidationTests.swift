// Cross-validation of the shared Rust BLE chunk codec (C ABI v2, wrapped by Codec/FrameReassembler)
// against the independent Swift reference (ReferenceFraming.swift): same input -> same chunks, same
// reassembled messages, and the same point of failure. Synthetic data only.
import XCTest
@testable import KagiLinkCore

private struct Lcg {
    var state: UInt64
    mutating func next() -> UInt64 { state = state &* 6364136223846793005 &+ 1442695040888963407; return state >> 33 }
    mutating func int(_ n: Int) -> Int { Int(next() % UInt64(n)) }
    mutating func bytes(_ n: Int) -> [UInt8] { (0..<n).map { _ in UInt8(truncatingIfNeeded: next()) } }
}

/// Feeds chunks to both implementations; returns the failing index (nil if neither failed) and the messages.
private func feedBoth(_ chunks: [[UInt8]]) throws -> (cIndex: Int?, refIndex: Int?, cMsgs: [[UInt8]], refMsgs: [[UInt8]], mt: ([UInt8], [UInt8])) {
    let c = try FrameReassembler()
    var ref = Framing.Reassembler()
    var cIndex: Int?, refIndex: Int?
    var cMsgs: [[UInt8]] = [], refMsgs: [[UInt8]] = []
    var cTypes: [UInt8] = [], rTypes: [UInt8] = []
    for (i, chunk) in chunks.enumerated() {
        if cIndex == nil {
            do { if let m = try c.push(chunk) { cMsgs.append(m.data); cTypes.append(m.mtype) } } catch { cIndex = i }
        }
        if refIndex == nil {
            do { if let m = try ref.push(chunk) { refMsgs.append(m.data); rTypes.append(m.mtype) } } catch { refIndex = i }
        }
    }
    return (cIndex, refIndex, cMsgs, refMsgs, (cTypes, rTypes))
}

final class CodecCrossValidationTests: XCTestCase {
    func testChunkOutputIdenticalForManyLengthsAndSizes() throws {
        var rng = Lcg(state: 1)
        var compared = 0
        for length in stride(from: 1, through: 512, by: 7) + [512] {
            for maxChunk in [6, 7, 20, 23, 50, 120, 185, 244, 512] {
                let msg = rng.bytes(length)
                let mtype = rng.int(2) == 0 ? Codec.mtypeHandshake : Codec.mtypeChannel
                let c = try Codec.chunk(mtype: mtype, message: msg, maxChunk: maxChunk)
                let r = try Framing.chunk(mtype: mtype, message: msg, maxChunk: maxChunk)
                XCTAssertEqual(c, r, "len \(length) chunk \(maxChunk)")
                compared += 1
            }
        }
        XCTAssertGreaterThan(compared, 600)
    }

    func testRejectionsAgree() {
        // bad mtype, empty, oversize, chunk size out of range: both refuse.
        for (mtype, len, chunk) in [(UInt8(9), 10, 20), (1, 0, 20), (1, 513, 244), (1, 10, 5), (1, 10, 513)] as [(UInt8, Int, Int)] {
            let msg = [UInt8](repeating: 7, count: len)
            XCTAssertThrowsError(try Codec.chunk(mtype: mtype, message: msg, maxChunk: chunk))
            XCTAssertThrowsError(try Framing.chunk(mtype: mtype, message: msg, maxChunk: chunk))
        }
    }

    func testRoundTripThroughBothReassemblers() throws {
        var rng = Lcg(state: 2)
        for _ in 0..<150 {
            let length = 1 + rng.int(512)
            let msg = rng.bytes(length)
            let chunks = try Codec.chunk(mtype: Codec.mtypeChannel, message: msg, maxChunk: 6 + rng.int(239))
            let out = try feedBoth(chunks)
            XCTAssertNil(out.cIndex); XCTAssertNil(out.refIndex)
            XCTAssertEqual(out.cMsgs, [msg]); XCTAssertEqual(out.refMsgs, [msg])
            XCTAssertEqual(out.mt.0, [Codec.mtypeChannel]); XCTAssertEqual(out.mt.1, [Codec.mtypeChannel])
        }
    }

    func testMutatedStreamsFailAtTheSameChunk() throws {
        var rng = Lcg(state: 3)
        var disagreements: [String] = []
        var failures = 0
        for iteration in 0..<600 {
            let msg = rng.bytes(1 + rng.int(300))
            var chunks = try Codec.chunk(mtype: Codec.mtypeHandshake, message: msg, maxChunk: 6 + rng.int(60))
            let i = rng.int(chunks.count)
            switch rng.int(9) {
            case 0: chunks.insert(chunks[i], at: i)                       // duplicate
            case 1: if chunks.count > 1 { chunks.remove(at: i) }          // missing
            case 2: if chunks.count > 1 { chunks.swapAt(i, (i + 1) % chunks.count) } // reorder
            case 3: chunks[i][0] |= 0b1000                                // reserved flag bit
            case 4: chunks[i][0] ^= 0b001                                 // toggle FIRST
            case 5: chunks[i][0] ^= 0b010                                 // toggle LAST
            case 6: if i == 0 { chunks[0][2] = 9 } else { chunks[i][1] &+= 1 } // unknown mtype / bad seq
            case 7: if i == 0 { chunks[0][3] = 0xFF; chunks[0][4] = 0xFF } else { chunks[i].removeLast() } // oversize len / truncated
            default: chunks[i][0] |= 0b100                                // ACK bit on a data chunk
            }
            let out = try feedBoth(chunks)
            if (out.cIndex == nil) != (out.refIndex == nil) || out.cIndex != out.refIndex || out.cMsgs != out.refMsgs {
                disagreements.append("iteration \(iteration): C fail=\(String(describing: out.cIndex)) ref fail=\(String(describing: out.refIndex))")
            }
            if out.cIndex != nil { failures += 1 }
        }
        XCTAssertEqual(disagreements, [], "implementations disagree")
        XCTAssertGreaterThan(failures, 150) // the mutations really produce errors
    }

    func testDeadReassemblerStaysDead() throws {
        let c = try FrameReassembler()
        XCTAssertThrowsError(try c.push([0b1000, 0, 1, 0, 1, 0]))
        XCTAssertTrue(c.isDead)
        let good = try Codec.chunk(mtype: Codec.mtypeHandshake, message: [1, 2, 3], maxChunk: 20)
        XCTAssertThrowsError(try c.push(good[0])) { XCTAssertEqual(($0 as? KagiLinkError)?.status, .closed) }
    }

    func testAckHelpers() throws {
        for seq in [UInt8(0), 1, 127, 255] {
            let ack = try Codec.ackChunk(seq: seq)
            XCTAssertEqual(ack.count, 2)
            XCTAssertEqual(try Codec.parseAck(ack), seq)
            // An ACK is not a data chunk: the reassembler must reject it.
            XCTAssertThrowsError(try FrameReassembler().push(ack))
        }
        XCTAssertNil(try Codec.parseAck([0b011, 0, 1, 0, 1, 0]))
        XCTAssertNil(try Codec.parseAck([0b100]))
    }
}
