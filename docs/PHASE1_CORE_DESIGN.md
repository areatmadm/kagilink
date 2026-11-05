# KagiLink Phase 1 core design (v1, synthetic data only)

Status: design frozen for v1 (D014). Code in core/kagilink-core is implemented; the local synthetic
test results are recorded in TEST_PLAN.md and IMPLEMENTATION_STATUS.md (2026-11-04). Nothing here is audited. Real-secret
transfer stays disabled (D012) and Phase 0 exit criterion 3 stays open.

## Scope and structure

One shared Rust crate (core/kagilink-core, package areatm-kagilink-core, no unsafe code)
holds the handshake, key schedule and encrypted transport for all platforms; other
platforms bind it through FFI later, after the Windows session instructs them to start.
Modules: wire (canonical encodings), code (six-digit code), keys (HKDF key schedule,
confirmation tags), handshake (ReceiverSession, SenderPending), transport (SecureChannel,
AEAD, fragmentation), clock (monotonic clock, manual clock for tests), error.

The wire format is specified in PROTOCOL.md; this document gives rationale, lifecycle
and limits. The core has no BLE, no hardware keys, no UI and no target input.

## Design decisions

- Roles: sender = CPace initiator A, receiver = responder B, independent of BLE
  central/peripheral. The receiver creates and displays the code and owns all limits.
- PAKE: CPACE-RISTR255-SHA512 via pakery-cpace 0.6.2 (D013). Ristretto255 is prime
  order, so no cofactor or invalid-curve handling is needed. The crate's P-256 suite is
  not the draft suite and is not used.
- Code: six ASCII digits, rejection sampling (u32 < 4_294_000_000, mod 10^6), used only
  as the CPace password. It never reaches the key schedule; only the CPace ISK does.
- Context binding: CI binds both full canonical hellos, hence roles, version, suite,
  nonces, device IDs, purpose and claims. sid is the receiver's session ID. Associated
  data labels are "KagiLink/sender" and "KagiLink/receiver". Re-check against
  handshake.rs when it lands.
- Explicit mutual key confirmation is added because CPace has none: HMAC-SHA-256 tags
  under HKDF-derived keys over the transcript hash TH (all four messages' contents).
  Sender verifies tag_r before sending tag_s; receiver verifies tag_s before releasing
  keys. Constant-time comparison (subtle).
- Key separation: confirmation keys and AEAD keys are distinct HKDF outputs, one AEAD
  key per direction, all bound to TH through salt and info.
- No negotiation: one version and one suite string, matched exactly.

## Receiver attempt lifecycle

States: Idle (code displayed, no attempt) -> AttemptOpen (M1 sent) -> AwaitingM4 (M3
sent) -> Consumed (success) or Dead (expired or three failures).

- The code TTL is 60 000 ms on a monotonic clock from creation, never extended.
- One attempt at a time. Each begin_attempt makes a fresh sid, nonce_r and CPace scalar.
- Failures counted (D026, was: every error): only attempts that already sent M3 - a bad M4
  tag, a second M2, or an attempt abandoned by begin_attempt after M3. Errors before M3
  (malformed or invalid M2, M4 before M3) drop the attempt without cost. At the third failure,
  or at expiry, the code is zeroized and the session is dead.
- Success consumes the code (zeroized) and returns the SecureChannel. A dead or
  consumed session returns NotActive, Expired or TooManyFailures.
- The sender has its own 60 s guard from start(); the receiver stays authoritative and
  the sender cannot reset it. After any failure the sender needs a fresh code.

## Transport

Per-direction AES-256-GCM key; nonce = 4 zero bytes || u64 BE counter; strictly
increasing, no random nonces, terminal at 2^20 messages per direction. The receiver
accepts only the expected sequence number. AAD binds version, direction, kind, session
ID, sequence and ciphertext length. Messages <=4096 bytes are encrypted whole, then
ciphertext||tag is split into <=256-byte fragments (adaptable downward, <=256 fragments)
with header seq | kind | idx | count. Reassembly is strict, in-order, single in-flight;
any anomaly closes the channel and releases no plaintext. SECRET send consumes one local
approval; SECRET receive needs receiver readiness. The channel expires at the original
code deadline; keys are zeroized on close.

## Threat-model notes and honest limits

- Online guessing: six digits give an attacker at most 3 guesses in 10^6 per code, only if
  the receiver limits are enforced and the attacker cannot reconnect without a fresh code.
  An active attacker present during the first attempt gets one guess per attempt
  consumed. There is no global backoff across codes yet.
- Offline brute force against the recorded transcript is not possible under the standard
  CPace assumptions (see the draft's security analysis); this rests on the draft, not on
  anything verified here.
- Offline authentication proves neither device integrity nor physical proximity. Device
  IDs and claims are unverified assertions.
- A compromised receiving computer exposes decrypted plaintext.
- Ristretto255 is not on the draft's RECOMMEND list (X25519 and P-256 are), and
  pakery-cpace has no independent audit. Appendix B vector conformance of the locked
  crate is not yet demonstrated by this repository's tests.
- No side-channel review and no zeroization review. ZeroizeOnDrop is best effort; no
  claim of immunity to swap, dumps or copies.
- CPace is not quantum-resistant. CNSA 2.0 / ML-KEM is not addressed and is recorded as
  a future consideration, not a promise.
- Dependencies are pinned (D014) but have had only an advisory lookup, not an audit.

## Not covered by this iteration

BLE transport and ATT mapping, hardware-backed keys and approval, target input safety and
RDP, FFI bindings, fuzzing, cross-language vectors, constant-time review, memory-dump
review, global backoff and the vault.

## Open items and exit

Phase 1 exits only when documented criteria in TEST_PLAN.md are executed with recorded
results, vectors are agreed by two independent implementations, fuzz targets run, and
external review of the PAKE integration is obtained. None is done as of this document.
