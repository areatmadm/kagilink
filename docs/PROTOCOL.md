# KagiLink protocol constraints — draft, not an interoperable implementation

No production wire protocol or cipher suite is finalized. No mock event constitutes
cryptographic authentication. Phase 0 model fields are NOT a wire specification.
PHASE0_BLE_PROBE.md defines a separate fixed-public-value diagnostic service only.
Its UUIDs and unauthenticated messages MUST NOT be used for production data or
treated as this protocol's handshake, confirmation or approved transfer.

## Required handshake

Receiver (central) creates a fresh random session ID and six-digit code using OS
CSPRNG with unbiased sampling, displays it for at most 60 monotonic seconds, and
connects to a foreground advertising sender. The user enters the code on sender.
Never advertise or transmit the code directly. Sender=A, receiver=B are fixed PAKE
roles, independently of BLE read/write direction.

Exchange bounded versioned hello data with fresh random contributions from both
peers, transient device IDs, exact roles, requested transfer purpose and security
claims. Canonical, length-delimited context must bind these fields plus both PAKE
shares into explicit mutual key confirmation. Preserve the selected PAKE's defined
transcript/key schedule; do not replace RFC internals with a home-made hash.
Reject unsupported versions/suites instead of downgrade. Receiver identity/TPM
labels are authenticated peer claims unless separately backed by verified evidence.

RFC 9382 compliance is a selection gate: an implementation named spake2 is not
automatically compatible with its group/encoding/key-confirmation vectors. The
selected reviewed suite, full encoding and golden vectors must be recorded before
cross-platform implementation. Do not substitute SPAKE2+ without approved design.

After verified mutual confirmation, derive distinct directional AES-256 keys and
confirmation/control keys using HKDF with domain-separated labels and context.
Do not use the six-digit code as an AES key or export PAKE's raw group element.
Use 96-bit GCM nonces derived from per-key direction/counter allocation, enforce
unique use and terminal overflow; no random-nonce collision assumptions. Exact
labels/counter encoding will be frozen with vectors in Phase 1.

## One-use delivery

One explicit local hardware-backed user approval authorizes one secret transfer
for the authenticated receiver/purpose/target. Enforce readiness on receiver before
release. Control and data envelopes authenticate session, version, role, purpose,
message kind, sequence and total length. Encrypt the complete bounded message,
then fragment ciphertext/tag; authenticate the reassembled envelope before any use.
Do not accept fragment metadata as authorization or release partial plaintext.

Draft bounds: message <=4096 bytes, fragment payload <=256 bytes and <=256
fragments; adapt downwards to negotiated ATT write/notification limits including
headers (support small MTUs). One in-flight message, strict order, bounded deadline.
Duplicate/missing/reordered or conflicting fragments abort; initial transport will
not implement application retransmission. Restart means a fresh session/keys/code.

Confirmation/tag/context/order failures close the current attempt and discard all
ephemeral state. Receiver tracks at most three failed fresh PAKE attempts for the
displayed code within its original 60s deadline; retry does not extend TTL. Closing
the code session consumes it. New session requires a fresh code and global
backoff, never reuse of a closed code, PAKE scalar or traffic key. The exact
retry framing/backoff must be frozen and tested before implementation.

Expire the complete first-version session at the original 60s deadline; no hidden
extension after authentication. Cancellation, disconnect, timeout, protocol/input
error and completion destroy permission/state. An authenticated result distinguishes
received, input attempted, input verified, failed and unknown. SendInput count alone
cannot mean password accepted or RDP connected. ACK loss must not retry input.

## References

- [SPAKE2 RFC 9382](https://www.rfc-editor.org/rfc/rfc9382.html)
- [HKDF RFC 5869](https://www.rfc-editor.org/rfc/rfc5869.html)
- [AES-GCM SP 800-38D](https://csrc.nist.gov/pubs/sp/800/38/d/final)
