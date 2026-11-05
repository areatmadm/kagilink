// Thin Swift wrapper over the KagiLink C ABI (D021 spike). Synthetic data only.
// No pointer handling here: see Native.swift. Handles are single-threaded (not Sendable).

public struct KagiLinkError: Error, Equatable, CustomStringConvertible {
    public let status: KLStatus
    public var description: String { "KagiLinkError(\(status))" }
}

public enum MessageKind: UInt8 {
    case control = 1
    case secret = 2
    case result = 3
}

public enum KagiLink {
    /// Bindings must refuse to run when the library and header ABI versions differ.
    public static func requireMatchingABI() throws {
        guard Native.abiVersion == Native.headerABIVersion else { throw KagiLinkError(status: .abi) }
    }

    public static var abiVersion: UInt32 { Native.abiVersion }
}

private func check(_ status: KLStatus) throws {
    if status != .ok { throw KagiLinkError(status: status) }
}

public final class Channel {
    fileprivate var handle: OpaquePointer?

    fileprivate init(_ handle: OpaquePointer) { self.handle = handle }
    deinit { Native.channelFree(handle) }

    private func live() throws -> OpaquePointer {
        guard let h = handle else { throw KagiLinkError(status: .null) }
        return h
    }

    /// Sender: allow exactly one SECRET seal. Call only after the platform's own user approval.
    public func approveOneTransfer() throws { try check(Native.channelApprove(try live())) }

    /// Receiver: true only when a verified safe input target exists.
    public func setReadyToReceive(_ ready: Bool) throws { try check(Native.channelSetReady(try live(), ready)) }

    /// Encrypts and fragments one message; returns the fragments without length prefixes.
    public func seal(_ kind: MessageKind, plaintext: [UInt8], maxFragmentPayload: Int) throws -> [[UInt8]] {
        let (st, fragments) = Native.channelSeal(try live(), kind: kind.rawValue, plaintext: plaintext,
                                                 maxFragmentPayload: maxFragmentPayload)
        try check(st)
        return fragments
    }

    /// Feeds one fragment. Returns the plaintext (wipe-on-release) only when a message completed.
    public func open(fragment: [UInt8]) throws -> (kind: MessageKind, plaintext: SecretBuffer)? {
        let (st, secret, kind, complete) = Native.channelOpenFragment(try live(), fragment: fragment)
        try check(st)
        guard complete, let secret, let k = MessageKind(rawValue: kind) else { return nil }
        return (k, secret)
    }

    public func close() throws { try check(Native.channelClose(try live())) }
    public var isClosed: Bool { handle.map(Native.channelIsClosed) ?? true }
}

public final class Receiver {
    private var handle: OpaquePointer?

    public init(deviceID: [UInt8], claims: [UInt8]) throws {
        try KagiLink.requireMatchingABI()
        let (st, h) = Native.receiverNew(deviceID: deviceID, claims: claims)
        try check(st)
        handle = h
    }

    deinit { Native.receiverFree(handle) }

    private func live() throws -> OpaquePointer {
        guard let h = handle else { throw KagiLinkError(status: .null) }
        return h
    }

    /// Six ASCII digits. Display only while active; the buffer wipes itself on release.
    public func code() throws -> SecretBuffer {
        let (st, secret) = Native.receiverCode(try live())
        try check(st)
        guard let secret else { throw KagiLinkError(status: .null) }
        return secret
    }

    public func beginAttempt() throws -> [UInt8] {
        let (st, m1) = Native.receiverBeginAttempt(try live())
        try check(st)
        return m1
    }

    public func handleM2(_ m2: [UInt8]) throws -> [UInt8] {
        let (st, m3) = Native.receiverHandleM2(try live(), m2)
        try check(st)
        return m3
    }

    public func finish(m4: [UInt8]) throws -> Channel {
        let (st, ch) = Native.receiverFinish(try live(), m4)
        try check(st)
        guard let ch else { throw KagiLinkError(status: .null) }
        return Channel(ch)
    }
}

public final class Sender {
    private var handle: OpaquePointer?
    public let m2: [UInt8]

    /// `code` is the six ASCII digits entered by the user.
    public init(code: [UInt8], m1: [UInt8], deviceID: [UInt8], purpose: [UInt8], claims: [UInt8]) throws {
        try KagiLink.requireMatchingABI()
        let (st, m2, h) = Native.senderStart(code: code, m1: m1, deviceID: deviceID, purpose: purpose, claims: claims)
        try check(st)
        self.m2 = m2
        handle = h
    }

    deinit { Native.senderFree(handle) }

    /// Verifies M3. The pending state is consumed on every outcome; a second call is WRONG_STATE.
    public func finish(m3: [UInt8]) throws -> (m4: [UInt8], channel: Channel) {
        guard let h = handle else { throw KagiLinkError(status: .null) }
        let (st, m4, ch) = Native.senderFinish(h, m3)
        try check(st)
        guard let ch else { throw KagiLinkError(status: .null) }
        return (m4, Channel(ch))
    }
}

/// BLE chunk codec (docs/BLE_GATT_SPEC.md section 5) through the shared Rust implementation.
public enum Codec {
    public static let mtypeHandshake = Native.mtypeHandshake
    public static let mtypeChannel = Native.mtypeChannel

    /// Splits one message into chunks of at most `maxChunk` bytes (use min(ATT MTU - 3, 244)).
    public static func chunk(mtype: UInt8, message: [UInt8], maxChunk: Int) throws -> [[UInt8]] {
        let (st, chunks) = Native.frameChunk(mtype: mtype, message: message, maxChunk: maxChunk)
        try check(st)
        return chunks
    }

    public static func ackChunk(seq: UInt8) throws -> [UInt8] {
        let (st, bytes) = Native.ackChunk(seq)
        try check(st)
        return bytes
    }

    /// The acknowledged sequence number if `chunk` is exactly an ACK chunk, else nil.
    /// Filter ACKs with this before feeding chunks to a FrameReassembler (it rejects them).
    public static func parseAck(_ chunk: [UInt8]) throws -> UInt8? {
        let (st, seq) = Native.parseAck(chunk)
        try check(st)
        return seq
    }
}

/// Strict in-order reassembler for ONE direction of ONE connection. Any error is final: drop the
/// connection and discard the session. The returned data is handshake or AEAD material; wipe it.
public final class FrameReassembler {
    private var handle: OpaquePointer?

    public init() throws {
        try KagiLink.requireMatchingABI()
        let (st, h) = Native.reassemblerNew()
        try check(st)
        handle = h
    }

    deinit { Native.reassemblerFree(handle) }

    public var isDead: Bool { handle.map(Native.reassemblerIsDead) ?? true }

    /// Feeds one chunk; returns the message when this chunk completed it.
    public func push(_ chunk: [UInt8]) throws -> (mtype: UInt8, data: [UInt8])? {
        guard let h = handle else { throw KagiLinkError(status: .null) }
        let (st, complete) = Native.reassemblerPush(h, chunk)
        try check(st)
        guard complete else { return nil }
        let (tst, mtype, data) = Native.reassemblerTake(h)
        try check(tst)
        return (mtype, data)
    }
}
