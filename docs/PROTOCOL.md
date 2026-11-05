# KagiLink protocol v1 — frozen Phase 1 core design, synthetic data only

Protocol version 1 is frozen for the Phase 1 core (core/kagilink-core; D014; design
rationale and lifecycle in PHASE1_CORE_DESIGN.md). It is implemented but NOT verified
against independent vectors, NOT reviewed externally, and is used with synthetic data
only (D012). It is not an interoperable release protocol until Phase 1 exit criteria
are met. No mock event constitutes cryptographic authentication. Phase 0 model fields
are NOT a wire specification. PHASE0_BLE_PROBE.md defines a separate fixed-public-value
diagnostic service only. Its UUIDs and unauthenticated messages MUST NOT be used for
production data or treated as this protocol's handshake, confirmation or approved
transfer. The BLE/ATT mapping of the messages below is drafted in BLE_GATT_SPEC.md (D025, unverified on real devices).

## Roles and code

Sender = CPace initiator A; receiver = CPace responder B, independently of BLE
central/peripheral and read/write direction. The receiver creates the code, displays it
for at most 60 monotonic seconds and is the only party that limits attempts. Never
advertise or transmit the code. The code is six ASCII digits drawn from the OS CSPRNG
by rejection sampling (u32 < 4_294_000_000, then mod 10^6) and is used only as the
CPace password (PRS); it is never a key.

## Handshake messages

All integers are big endian. lv = u16 length || bytes. Parsers reject truncation,
trailing bytes, over-length fields, unknown fields, unknown versions and unknown suites.
The suite string (`wire::SUITE`, KagiLink-P1/CPACE-RISTR255-SHA512/HKDF-SHA512/
HMAC-SHA256/AES-256-GCM) must match exactly; there is no negotiation and no downgrade.

| Msg | Direction | Encoding |
|---|---|---|
| M1 | receiver -> sender | HelloR: "KL", version 1, type 1, lv(suite), sid[16], nonce_r[32], lv(device_id <=32), lv(claims <=64) |
| M2 | sender -> receiver | 0x12 \| lv(HelloS, <=512) \| Ya[32]; HelloS: "KL", version 1, type 2, lv(suite), nonce_s[32], lv(device_id <=32), lv(purpose <=32), lv(claims <=64) |
| M3 | receiver -> sender | 0x13 \| Yb[32] \| tag_r[32] |
| M4 | sender -> receiver | 0x14 \| tag_s[32] |

sid and nonce_r are fresh per attempt; nonce_s and the CPace scalars are fresh per
attempt on both sides. Device IDs, purpose and claims are unverified peer claims;
receiver identity/TPM labels are authenticated peer claims unless separately backed by
verified evidence.

## PAKE and key schedule

PAKE is CPace (draft-irtf-cfrg-cpace; D013), suite CPACE-RISTR255-SHA512 (draft
section 8.3, Appendix B.3), crate pakery-cpace 0.6.2. Inputs: PRS = the six digits;
CI = lv(HelloR bytes) || lv(HelloS bytes) as built by the handshake, binding both full
canonical hellos (roles, version, suite, nonces, device IDs, purpose, claims); sid =
HelloR.sid; ADa = "KagiLink/sender"; ADb = "KagiLink/receiver"; mode
initiator-responder. The CI construction is finalized when handshake.rs lands and must
be re-checked against this text then. CPace has no key confirmation; KagiLink adds it.
Conformance to the draft's Appendix B vectors is a selection gate and is not yet
demonstrated for the locked crate; record vectors before cross-platform ports. Do not
substitute SPAKE2, SPAKE2+ or OPAQUE without approved design.

- TH = SHA-512("kagilink/v1/transcript" || for each of HelloR, HelloS, Ya, Yb: u32 BE
  length || bytes).
- PRK = HKDF-Extract(salt = TH, IKM = ISK), ISK = 64-byte CPace output (HKDF-SHA-512).
- Expand with info = u16 len || label || u16 len || TH, 32 bytes each, for labels
  kagilink/v1/conf-key/receiver, kagilink/v1/conf-key/sender,
  kagilink/v1/aead-key/sender-to-receiver, kagilink/v1/aead-key/receiver-to-sender.
- tag_r = HMAC-SHA-256(conf-key/receiver, u16 len || "kagilink/v1/confirm/receiver" || TH);
  tag_s likewise with the sender key and label "kagilink/v1/confirm/sender".

Order: the sender verifies tag_r (M3) before sending tag_s (M4); the receiver verifies
tag_s before releasing keys or the channel. Tag comparison is constant time. The code
and the raw ISK/group element never become AES keys. Confirmation/tag/context/order
failures close the attempt and discard all ephemeral state.

## Attempt limits (receiver authoritative)

Code TTL is 60 000 ms on a monotonic clock from creation (first display), never
extended. One attempt at a time. Any failure after M2 is accepted for processing
(malformed M2, invalid share, wrong state, bad M4 tag, attempt abandoned by a new
begin_attempt while one is open) counts as a failed attempt. At three failures or at
expiry the code is zeroized and the session is dead; success consumes the code. A new
session requires a fresh code, session ID, nonce and PAKE scalar; a closed code,
scalar or traffic key is never reused, and the sender must ask for a fresh code after any
failure. The sender keeps its own 60 s guard from start(); it cannot extend or reset the
receiver's budget. A global backoff beyond this per-code limit is not yet specified.

## Encrypted transport

One AES-256-GCM key per direction. Nonce = 4 zero bytes || seq (u64 BE), a strictly
increasing per-direction counter; no random nonces; terminal exhaustion at 2^20
messages per direction. The receiver accepts only seq == expected, so replay, reorder
and gaps fail. AAD = [version=1][direction: 1 sender-to-receiver, 2 receiver-to-sender]
[kind][session_id 16][seq u64 BE][ciphertext_len u32 BE]. Kinds: CONTROL=1, SECRET=2,
RESULT=3.

The complete bounded message is encrypted first, then ciphertext||tag is fragmented.
Fragment header = seq u64 | kind u8 | idx u16 | count u16. Draft bounds: message
<=4096 bytes, fragment payload <=256 bytes (adaptable downward for small ATT MTUs
including headers) and <=256 fragments. One in-flight message, strict in-order
reassembly. Any duplicate, missing, reordered, conflicting or oversized fragment, or
any tag failure, closes the channel and releases no plaintext. Fragment metadata is
never authorization. The initial transport has no application retransmission; restart
means a fresh session, keys and code.

## One-use delivery

A SECRET send requires one explicit local approval, consumed by the seal; a SECRET
receive requires receiver readiness. In the production design this approval is a
hardware-backed local user approval for the authenticated receiver/purpose/target;
the core only models the one-use permission, and hardware-backed approval, readiness
of the actual target and input safety are NOT covered by the Phase 1 core.

The channel expires at the ORIGINAL code deadline; there is no hidden extension after
authentication. Cancellation, disconnect, timeout, protocol/input error and completion
destroy permission and state; keys are zeroized on close. An authenticated result
distinguishes received, input attempted, input verified, failed and unknown. SendInput
count alone cannot mean password accepted or RDP connected. ACK loss must not retry
input.

## Not covered by v1 core

BLE GATT/ATT mapping and MTU negotiation, hardware-backed keys and approval, target
input safety, RDP delivery, global backoff across codes, post-quantum protection (CPace
is not quantum-resistant; ML-KEM/CNSA 2.0 is a future consideration, not a promise).

## References

- [CPace draft-irtf-cfrg-cpace](https://datatracker.ietf.org/doc/draft-irtf-cfrg-cpace/)
- [SPAKE2 RFC 9382](https://www.rfc-editor.org/rfc/rfc9382.html) (not selected; D013)
- [HKDF RFC 5869](https://www.rfc-editor.org/rfc/rfc5869.html)
- [AES-GCM SP 800-38D](https://csrc.nist.gov/pubs/sp/800/38/d/final)
