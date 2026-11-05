// Independent Swift reference of the BLE chunk framing (docs/BLE_GATT_SPEC.md section 5), kept ONLY to
// cross-validate the shared Rust codec (kl_frame_*). It was the spike's own implementation before ABI v2.

enum FrameError: Error { case limits, malformed }

enum Framing {
    static let maxMessage = 512
    static let minChunk = 6
    static let mtypeHandshake: UInt8 = 1
    static let mtypeChannel: UInt8 = 2
    private static let first: UInt8 = 0b001
    private static let last: UInt8 = 0b010
    private static let ack: UInt8 = 0b100
    private static let header = 2
    private static let prefix = 3

    static func chunk(mtype: UInt8, message: [UInt8], maxChunk: Int) throws -> [[UInt8]] {
        guard mtype == mtypeHandshake || mtype == mtypeChannel, !message.isEmpty,
              message.count <= maxMessage, (minChunk...512).contains(maxChunk) else { throw FrameError.limits }
        var stream: [UInt8] = [mtype, UInt8(message.count >> 8), UInt8(message.count & 0xFF)]
        stream.append(contentsOf: message)
        let room = maxChunk - header
        let count = (stream.count + room - 1) / room
        guard count <= 256 else { throw FrameError.limits }
        var out: [[UInt8]] = []
        for i in 0..<count {
            var flags: UInt8 = 0
            if i == 0 { flags |= first }
            if i == count - 1 { flags |= last }
            let piece = stream[(i * room)..<min((i + 1) * room, stream.count)]
            out.append([flags, UInt8(i)] + piece)
        }
        return out
    }

    /// Strict in-order reassembler; any error kills it for good (fail closed).
    struct Reassembler {
        private(set) var dead = false
        private var expectedSeq: UInt8 = 0
        private var inMessage = false
        private var mtype: UInt8 = 0
        private var total = 0
        private var data: [UInt8] = []

        mutating func push(_ chunk: [UInt8]) throws -> (mtype: UInt8, data: [UInt8])? {
            if dead { throw FrameError.malformed }
            do { return try step(chunk) } catch { dead = true; throw error }
        }

        private mutating func step(_ c: [UInt8]) throws -> (mtype: UInt8, data: [UInt8])? {
            guard c.count >= 3 else { throw FrameError.malformed }
            let flags = c[0], seq = c[1]
            guard flags & ~(first | last) == 0 else { throw FrameError.malformed } // reserved/ACK bits
            var body = c[2...]
            if flags & first != 0 {
                guard !inMessage, seq == 0, body.count >= 3 else { throw FrameError.malformed }
                let m = body[body.startIndex]
                guard m == Framing.mtypeHandshake || m == Framing.mtypeChannel else { throw FrameError.malformed }
                let len = Int(body[body.startIndex + 1]) << 8 | Int(body[body.startIndex + 2])
                guard len >= 1, len <= Framing.maxMessage else { throw FrameError.malformed }
                mtype = m; total = len; data = []; inMessage = true; expectedSeq = 0
                body = body.dropFirst(3)
            } else {
                guard inMessage, seq == expectedSeq else { throw FrameError.malformed }
            }
            guard seq == expectedSeq else { throw FrameError.malformed }
            guard data.count + body.count <= total else { throw FrameError.malformed }
            data.append(contentsOf: body)
            expectedSeq = expectedSeq &+ 1
            let isLast = flags & last != 0
            if data.count == total {
                guard isLast else { throw FrameError.malformed }
                inMessage = false
                let done = (mtype, data)
                data = []
                return done
            }
            if isLast { throw FrameError.malformed }
            return nil
        }
    }
}
