import CryptoKit
import Foundation

/// What is known about the receiver. Identity is a CLAIM, never proof. Until the protocol carries a stable
/// receiver identity, the per-session KL-XXXX tag is not an identity, so use `.unknown`.
public enum ReceiverIdentity: Equatable, Sendable {
    case unknown
    case claimed(String)

    /// SHA-256 hex of the claimed identity; nil when unknown or empty. The raw claim is never stored.
    public var hash: String? {
        guard case .claimed(let claim) = self, !claim.isEmpty else { return nil }
        return SHA256.hash(data: Data(claim.utf8)).map { String(format: "%02x", $0) }.joined()
    }
}

/// Decision function from docs/RISK_ACK_SCREEN_SPEC.md. Returns true when the notice must be shown.
/// The notice is skipped ONLY when the stored version equals the current version AND a stable receiver
/// identity exists on both sides and matches. Unknown identity, first use, corrupt record, changed receiver
/// and bumped version all show the notice.
public func needsNotice(storedVersion: Int?, storedReceiverHash: String?,
                        currentVersion: Int, currentReceiverHash: String?) -> Bool {
    guard let storedVersion, storedVersion == currentVersion else { return true }
    guard let current = currentReceiverHash, !current.isEmpty,
          let stored = storedReceiverHash, !stored.isEmpty else { return true }
    return stored != current
}

/// Local, non-secret record: notice version, receiver hash (when known) and a timestamp. Nothing else.
public struct AckRecord: Codable, Equatable, Sendable {
    public let noticeVersion: Int
    public let receiverHash: String?
    public let acknowledgedAt: Date

    public init(noticeVersion: Int, receiverHash: String?, acknowledgedAt: Date) {
        self.noticeVersion = noticeVersion
        self.receiverHash = receiverHash
        self.acknowledgedAt = acknowledgedAt
    }
}

public protocol AckStorage: Sendable {
    func load() throws -> Data?
    func save(_ data: Data) throws
}

public struct UserDefaultsAckStorage: AckStorage, @unchecked Sendable {
    private let defaults: UserDefaults
    private let key: String

    public init(defaults: UserDefaults = .standard, key: String = "kagilink.riskAck.record") {
        self.defaults = defaults
        self.key = key
    }

    public func load() throws -> Data? { defaults.data(forKey: key) }
    public func save(_ data: Data) throws { defaults.set(data, forKey: key) }
}

enum AckRecordCodec {
    static func encode(_ record: AckRecord) throws -> Data {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        return try encoder.encode(record)
    }

    /// Unreadable or corrupt data decodes to nil, which the caller treats as "no record" (show the screen).
    static func decode(_ data: Data?) -> AckRecord? {
        guard let data else { return nil }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return try? decoder.decode(AckRecord.self, from: data)
    }
}
