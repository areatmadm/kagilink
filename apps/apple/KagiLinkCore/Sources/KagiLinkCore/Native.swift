// The ONLY file in this module that touches raw pointers or the C ABI directly (D021).
// Everything above it works with [UInt8], SecretBuffer and OpaquePointer handles.
import KagiLinkC

/// Status codes from kagilink.h, kept numerically identical.
public struct KLStatus: RawRepresentable, Equatable, Hashable, Sendable, CustomStringConvertible {
    public let rawValue: Int32
    public init(rawValue: Int32) { self.rawValue = rawValue }
    public static let ok = KLStatus(rawValue: 0)
    public static let expired = KLStatus(rawValue: 1)
    public static let notActive = KLStatus(rawValue: 2)
    public static let tooManyFailures = KLStatus(rawValue: 3)
    public static let attemptInProgress = KLStatus(rawValue: 4)
    public static let malformed = KLStatus(rawValue: 5)
    public static let unsupported = KLStatus(rawValue: 6)
    public static let wrongState = KLStatus(rawValue: 7)
    public static let pake = KLStatus(rawValue: 8)
    public static let confirmationFailed = KLStatus(rawValue: 9)
    public static let badCode = KLStatus(rawValue: 10)
    public static let authentication = KLStatus(rawValue: 11)
    public static let framing = KLStatus(rawValue: 12)
    public static let limits = KLStatus(rawValue: 13)
    public static let counterExhausted = KLStatus(rawValue: 14)
    public static let notApproved = KLStatus(rawValue: 15)
    public static let closed = KLStatus(rawValue: 16)
    public static let randomness = KLStatus(rawValue: 17)
    public static let null = KLStatus(rawValue: 100)
    public static let buffer = KLStatus(rawValue: 101)
    public static let busy = KLStatus(rawValue: 102)
    public static let panic = KLStatus(rawValue: 103)
    public static let abi = KLStatus(rawValue: 104)
    public var description: String { "KLStatus(\(rawValue))" }
}

/// Heap buffer for secrets (code digits, plaintext). Wiped with kl_secure_wipe on release.
/// Host copies made from it (Strings, Data) are the caller's responsibility.
public final class SecretBuffer {
    private let storage: UnsafeMutablePointer<UInt8>
    public private(set) var count: Int
    private let capacity: Int

    init(capacity: Int) {
        self.capacity = max(capacity, 1)
        storage = UnsafeMutablePointer<UInt8>.allocate(capacity: self.capacity)
        storage.initialize(repeating: 0, count: self.capacity)
        count = 0
    }

    deinit {
        kl_secure_wipe(storage, capacity)
        storage.deallocate()
    }

    /// Read-only access for the duration of `body`; do not retain the pointer.
    public func withUnsafeBytes<R>(_ body: (UnsafeRawBufferPointer) throws -> R) rethrows -> R {
        try body(UnsafeRawBufferPointer(start: storage, count: count))
    }

    /// Copy of the contents. The copy is NOT wiped by this type.
    public func copyBytes() -> [UInt8] { Array(UnsafeBufferPointer(start: storage, count: count)) }

    /// Wipe now and set length to zero.
    public func wipe() {
        kl_secure_wipe(storage, capacity)
        count = 0
    }

    fileprivate func withMutable<R>(_ body: (UnsafeMutablePointer<UInt8>, Int) -> R) -> R {
        body(storage, capacity)
    }
    fileprivate func setCount(_ n: Int) { count = min(n, capacity) }
}

/// kl_status_t is imported as UInt32; the public status type is Int32 (values are all small).
private func s32(_ s: kl_status_t) -> Int32 { Int32(truncatingIfNeeded: s.rawValue) }

enum Native {
    static var abiVersion: UInt32 { kl_abi_version() }
    static let headerABIVersion: UInt32 = UInt32(KL_ABI_VERSION)

    static func wipe(_ bytes: inout [UInt8]) {
        bytes.withUnsafeMutableBufferPointer { kl_secure_wipe($0.baseAddress, $0.count) }
    }

    // MARK: output helper

    /// Calls `body` with an output buffer and its capacity; on KL_ERR_BUFFER retries once with the
    /// size the library reports (it writes nothing and consumes no state in that case).
    static func withGrowingOutput(
        initial: Int = 4096,
        _ body: (UnsafeMutablePointer<UInt8>?, inout Int) -> Int32
    ) -> (KLStatus, [UInt8]) {
        var capacity = initial
        for _ in 0..<2 {
            var buf = [UInt8](repeating: 0, count: capacity)
            var len = capacity
            let st = buf.withUnsafeMutableBufferPointer { body($0.baseAddress, &len) }
            if st == KLStatus.buffer.rawValue && len > capacity {
                wipe(&buf)
                capacity = len
                continue
            }
            if st != 0 { wipe(&buf); return (KLStatus(rawValue: st), []) }
            let out = Array(buf.prefix(len))
            wipe(&buf)
            return (.ok, out)
        }
        return (.buffer, [])
    }

    // MARK: receiver

    static func receiverNew(deviceID: [UInt8], claims: [UInt8]) -> (KLStatus, OpaquePointer?) {
        var out: OpaquePointer?
        let st = deviceID.withUnsafeBufferPointer { d in
            claims.withUnsafeBufferPointer { c in
                kl_receiver_new(d.baseAddress, d.count, c.baseAddress, c.count, &out)
            }
        }
        return (KLStatus(rawValue: s32(st)), out)
    }

    static func receiverCode(_ r: OpaquePointer) -> (KLStatus, SecretBuffer?) {
        let secret = SecretBuffer(capacity: 6)
        var written = 0
        let st: kl_status_t = secret.withMutable { p, cap in
            var len = cap
            let s = kl_receiver_code(r, p, &len)
            written = len
            return s
        }
        if st != KL_OK { return (KLStatus(rawValue: s32(st)), nil) }
        secret.setCount(written)
        return (.ok, secret)
    }

    static func receiverBeginAttempt(_ r: OpaquePointer) -> (KLStatus, [UInt8]) {
        withGrowingOutput { out, len in s32(kl_receiver_begin_attempt(r, out, &len)) }
    }

    static func receiverHandleM2(_ r: OpaquePointer, _ m2: [UInt8]) -> (KLStatus, [UInt8]) {
        withGrowingOutput { (out: UnsafeMutablePointer<UInt8>?, len: inout Int) -> Int32 in
            var outLen = len
            let st: kl_status_t = m2.withUnsafeBufferPointer { (m: UnsafeBufferPointer<UInt8>) -> kl_status_t in
                kl_receiver_handle_m2(r, m.baseAddress, m.count, out, &outLen)
            }
            len = outLen
            return s32(st)
        }
    }

    static func receiverFinish(_ r: OpaquePointer, _ m4: [UInt8]) -> (KLStatus, OpaquePointer?) {
        var ch: OpaquePointer?
        let st = m4.withUnsafeBufferPointer { kl_receiver_finish(r, $0.baseAddress, $0.count, &ch) }
        return (KLStatus(rawValue: s32(st)), ch)
    }

    static func receiverFree(_ r: OpaquePointer?) { kl_receiver_free(r) }

    // MARK: sender

    static func senderStart(
        code: [UInt8], m1: [UInt8], deviceID: [UInt8], purpose: [UInt8], claims: [UInt8]
    ) -> (KLStatus, [UInt8], OpaquePointer?) {
        var sender: OpaquePointer?
        let (st, m2) = withGrowingOutput { (out: UnsafeMutablePointer<UInt8>?, len: inout Int) -> Int32 in
            var outLen = len
            var created: OpaquePointer?
            let status = callSenderStart(code, m1, deviceID, purpose, claims, out, &outLen, &created)
            len = outLen
            if status == KL_OK { sender = created }
            return s32(status)
        }
        return (st, m2, st == .ok ? sender : nil)
    }

    private static func callSenderStart(
        _ code: [UInt8], _ m1: [UInt8], _ deviceID: [UInt8], _ purpose: [UInt8], _ claims: [UInt8],
        _ out: UnsafeMutablePointer<UInt8>?, _ outLen: inout Int, _ created: inout OpaquePointer?
    ) -> kl_status_t {
        code.withUnsafeBufferPointer { (cd: UnsafeBufferPointer<UInt8>) -> kl_status_t in
        m1.withUnsafeBufferPointer { (m: UnsafeBufferPointer<UInt8>) -> kl_status_t in
        deviceID.withUnsafeBufferPointer { (d: UnsafeBufferPointer<UInt8>) -> kl_status_t in
        purpose.withUnsafeBufferPointer { (p: UnsafeBufferPointer<UInt8>) -> kl_status_t in
        claims.withUnsafeBufferPointer { (c: UnsafeBufferPointer<UInt8>) -> kl_status_t in
            kl_sender_start(cd.baseAddress, cd.count, m.baseAddress, m.count,
                            d.baseAddress, d.count, p.baseAddress, p.count,
                            c.baseAddress, c.count, out, &outLen, &created)
        }}}}}
    }

    static func senderFinish(_ s: OpaquePointer, _ m3: [UInt8]) -> (KLStatus, [UInt8], OpaquePointer?) {
        var ch: OpaquePointer?
        let (st, m4) = withGrowingOutput { out, len in
            m3.withUnsafeBufferPointer { s32(kl_sender_finish(s, $0.baseAddress, $0.count, out, &len, &ch)) }
        }
        return (st, m4, st == .ok ? ch : nil)
    }

    static func senderFree(_ s: OpaquePointer?) { kl_sender_free(s) }

    // MARK: channel

    static func channelApprove(_ c: OpaquePointer) -> KLStatus {
        KLStatus(rawValue: s32(kl_channel_approve_one_transfer(c)))
    }

    static func channelSetReady(_ c: OpaquePointer, _ ready: Bool) -> KLStatus {
        KLStatus(rawValue: s32(kl_channel_set_ready_to_receive(c, ready ? 1 : 0)))
    }

    /// Returns the fragments (length prefix already removed).
    static func channelSeal(
        _ c: OpaquePointer, kind: UInt8, plaintext: [UInt8], maxFragmentPayload: Int
    ) -> (KLStatus, [[UInt8]]) {
        var count = 0
        let (st, blob) = withGrowingOutput(initial: 8192) { out, len in
            plaintext.withUnsafeBufferPointer {
                s32(kl_channel_seal(c, kind, $0.baseAddress, $0.count, maxFragmentPayload, out, &len, &count))
            }
        }
        guard st == .ok else { return (st, []) }
        var fragments: [[UInt8]] = []
        var off = 0
        for _ in 0..<count {
            guard off + 4 <= blob.count else { return (.framing, []) }
            let b0 = Int(blob[off]), b1 = Int(blob[off + 1]), b2 = Int(blob[off + 2]), b3 = Int(blob[off + 3])
            let n = (b0 << 24) | (b1 << 16) | (b2 << 8) | b3
            off += 4
            guard off + n <= blob.count else { return (.framing, []) }
            fragments.append(Array(blob[off..<(off + n)]))
            off += n
        }
        return (.ok, fragments)
    }

    static func channelOpenFragment(
        _ c: OpaquePointer, fragment: [UInt8]
    ) -> (KLStatus, SecretBuffer?, UInt8, Bool) {
        var secret = SecretBuffer(capacity: 4096)
        var kind: UInt8 = 0
        var complete: UInt8 = 0
        var written = 0
        var st: kl_status_t = secret.withMutable { p, cap in
            var len = cap
            let s = fragment.withUnsafeBufferPointer {
                kl_channel_open_fragment(c, $0.baseAddress, $0.count, p, &len, &kind, &complete)
            }
            written = len
            return s
        }
        if st == KL_ERR_BUFFER {
            // The completed message is held inside the channel; ask again with an empty fragment.
            secret = SecretBuffer(capacity: written)
            st = secret.withMutable { p, cap in
                var len = cap
                let s = kl_channel_open_fragment(c, nil, 0, p, &len, &kind, &complete)
                written = len
                return s
            }
        }
        if st != KL_OK { return (KLStatus(rawValue: s32(st)), nil, 0, false) }
        secret.setCount(written)
        return (.ok, secret, kind, complete == 1)
    }

    static func channelClose(_ c: OpaquePointer) -> KLStatus { KLStatus(rawValue: s32(kl_channel_close(c))) }
    static func channelIsClosed(_ c: OpaquePointer) -> Bool { kl_channel_is_closed(c) == 1 }
    static func channelFree(_ c: OpaquePointer?) { kl_channel_free(c) }
}
