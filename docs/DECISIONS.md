# KagiLink decisions

## D001 — Phase 0 gate, 2026-11-04

Start with official-source review, synthetic Rust model and Windows feasibility
probe. Do not advance to production crypto/transfers before Phase 0 exit criteria.
No original files existed; no Git repository was present at initial inspection.

## D002 — BLE roles

Keep desktop Central and mobile Peripheral. Sender advertises, receiver scans.
The logical receiver creates the session/code; sender UI waits for connection.
This resolves discovery wording without silently reversing established roles.

## D003 — Windows credential input

Keep RDP/CredUI unsupported and real-secret transfer disabled. Current Windows
credential-dialog restrictions and focus races invalidate a blanket support claim.
An owned synthetic edit proves only that specific ordinary-window path. Do not
seek elevation, UIAccess, clipboard or other bypasses.

## D004 — Cryptographic library selection

No crypto dependency approved. RustCrypto SPAKE2 warns of missing audit/erasure;
review actual RFC compatibility, alternatives and memory behavior first. No mock
cryptographic substitute, custom PAKE, or implicit switch to SPAKE2+.

## D005 — Shared AI memory (explicit user instruction)

All agent-authored conversation memory/handoff notes live at shared/ai/ inside this
project in a separate private repository. Check owner identity before accessing
any contents; see AI_MEMORY.md. Do not use USER folders for agent memory. Preserve
original prompts only in private memory. App-managed chat history is outside the
scope of this file policy.

## D006 — Restricted-runner evidence

The local runner is Low integrity (RID 4096). Read-only inventory may report
diagnostics there, but exits incomplete and cannot prove normal-user feasibility.
Synthetic SendInput remains restricted to Medium integrity and was safely refused.
Do not remove input guards merely to obtain a test pass in the agent sandbox.

## D007 — Separate private memory and clean source history

Keep shared/ai/ ignored by the source repository, with its own private Git remote.
Source bootstrap policy permits memory access only after an authenticated
areatmadm identity/access check. Other contributors can work from public docs.
Publish only a clean source snapshot; preserve prior mixed history privately.
Never make a repository public merely after deleting files from its current tree.

## D008 — KagiLink identity migration, 2026-11-04

Rename the existing source and private-memory repositories in place to kagilink
and kagilink-ai-memory. Preserve repository IDs, commit ancestry, main branches,
visibility and permissions. This migration does not reset or recreate history.
KagiLink (카기링크) is the sole current product name; AreaTM remains the publisher.
Previous names are historical only. See MIGRATION_KAGILINK.md for verification.

Keep internal areatm-phase0-model crate name, workspace paths, native source
filenames and local workspace path for compatibility; none is a product display
name. Update Cargo repository metadata and the probe output filename/display
resources instead. No deployed app bundle IDs or external signing identities
exist. Do not rename the earlier private archive or rewrite historical records.

## D009 — Local workspace relocation, 2026-11-04

The local workspace is now S:\project\kagilink after the folder move. This
supersedes D008's local-path compatibility choice only. Keep both Git histories,
origin URLs, private-memory isolation, internal crate names and Phase 0 scope.
Build scripts resolve from their own location and require no hardcoded old path.
Post-move validation evidence is recorded in IMPLEMENTATION_STATUS.md.

## D010 — Native Phase 0 diagnostic targets, 2026-11-04

Prepare Windows Central, Android/iOS Peripheral and macOS Central/Peripheral
feasibility sources without advancing to production native apps. Diagnostic UUIDs
and fixed public request/reply values are isolated from PROTOCOL.md; never accept
secret input, treat a fixed reply as authentication or connect this path to input.
Keep one explicit-start attempt, a 60-second deadline and terminal cleanup. Native
hardware labels stay unverified until actual protected key operations are tested.

Use SwiftUI/Core Bluetooth shared source in apps/apple with distinct unsigned
iOS/iPadOS 18+ and Apple Silicon/macOS 14+ targets. Build the iOS target with the
explicit iOS SDK 27.1 for iPhone Duo. Use Kotlin/platform Android Views/BLE,
Android 14/API 34+ with the latest stable Android SDK (currently platform 37.2),
with signing disabled. These provisional diagnostic IDs do not finalize production
signing identity. The mobile minimum OS versions are required support floors. Android
tool versions and generated locks are reviewed separately from security-core
selection. See DEPENDENCY_REVIEW.md and PHASE0_BLE_PROBE.md.

Keep native builds and device evidence distinct from static source/project tests.
Windows CLI can be built here; Android SDK and Apple build/device validation remain
pending on a suitable host. No account, certificate, SDK license or publication
action is automated. Phase 0 exit criteria remain unchanged and unmet.

Mobile screens use Liquid Glass styling. Use Apple's native glass API only on
supported OS versions, with a system-material fallback on iOS 18; Android uses
platform-rendered translucent glass styling without importing an Apple framework
or third-party UI SDK. Respect accessibility contrast, reduced transparency and
text scaling. The visual material is not a claim of authentication or protection.
Adaptive layouts must handle iPhone Duo inner/outer displays, resizing, safe areas
and vertical system bars. Actual rendering and device behavior remain unverified.

## D011 — Windows Phase 0 diagnostic GUI, 2026-11-04

Add a native Win32 window in `kagilink.exe` while preserving the existing console
probe as `kagilink-phase0-probe.exe`. The two fixed GUI actions are read-only
inventory and the isolated public BLE diagnostic. Keep direct launch idle until a
user action, display results in the window, and expose cancellation. Do not expose
the CLI's input-injection experiment or accept arbitrary commands, targets or data.

Use the sibling probe process without a shell, bounded captured output and
nonblocking message-loop polling. Supervise the child with a kill-on-close job
and a 70-second outer deadline; the child's 60-second BLE deadline remains unchanged.
This provides a reviewable Phase 0 interface, not a Phase 4 product, secret-transfer
authorization or proof of peer identity/hardware/target-input safety.

## D012 — Safety-first SPAKE2 selection criteria, 2026-11-04 (proposed)

Owner direction: aim for the safest design, not government certification. KCMVP
(Korea) and FIPS 140-3/CNSA (US) validation are explicit non-goals for now; revisit
only by a new decision. Searches found no NIS or NSA/NIST recommendation of SPAKE2
or any PAKE, so no regulator guidance overrides the project policy.

No audited SPAKE2 library was found (DEPENDENCY_REVIEW.md, 2026-11-04). Therefore:

1. Do not enable real-secret transfer on any unaudited PAKE. Until an independent
   review of the exact locked versions exists, Phase 1 code is synthetic-data only.
2. Prefer RFC 9382 (P-256, SHA-256, HKDF, HMAC) so correctness can be checked against
   the RFC's published vectors. The Ed25519 python-spake2 variant has no RFC vectors.
3. Require two independent implementations to agree with the RFC vectors and with each
   other (for example a Rust crate and a separate reference) before selection.
4. Do not write our own PAKE. Wrap a reviewed crate; keep our code to framing,
   transcript binding, rate limits, lifecycle and zeroization.
5. Layer defenses so a PAKE flaw alone cannot reveal a secret: hardware-bound device
   keys, explicit per-transfer approval, one-time permission and AES-256-GCM under
   keys derived from the confirmed session.
6. Fuzz message parsing and framing, run cargo audit/deny on the full lock, and
   obtain external review of the PAKE integration before Phase 3 hardware work
   treats it as production.

Status: proposed pending owner confirmation of the library and suite; the choice of
native versus shared Rust core remains open. The SPAKE2 premise is under review: the
CFRG recommended CPace instead (see DEPENDENCY_REVIEW.md, CPace vs SPAKE2). Any switch
needs explicit owner approval because AGENTS.md currently requires SPAKE2.

## D013 — Use CPace instead of SPAKE2, 2026-11-04

Owner approved replacing the SPAKE2 requirement with CPace (draft-irtf-cfrg-cpace,
state "Sent to the RFC Editor"; CFRG-recommended balanced PAKE). Reasons: standards
position, published proofs, per-suite test vectors, smaller state machine
(DEPENDENCY_REVIEW.md, CPace vs SPAKE2). SPAKE2/RFC 9382 is no longer the required
protocol; reintroducing it needs a new approval. D012 safety-first rules continue to
apply, with CPace in place of SPAKE2 (RFC 9382 vectors become draft Appendix B vectors).

Constraints unchanged: code TTL 60 s, three failures, one-time code, explicit mutual
key confirmation (added by KagiLink because CPace has none), HKDF, AES-256-GCM,
transcript binding, synthetic data only until independent review. Candidate crate
`pakery-cpace` with the draft's CPACE-RISTR255-SHA512 suite (section 8.3, Appendix B.3).
Correction (same day): the earlier preference for P-256 was withdrawn because
`pakery-cpace`'s P-256 suite uses SHA-512 and its own DSI, i.e. it is NOT the draft's
CPACE-P256_XMD:SHA-256_SSWU_NU_ suite and the draft's positive vectors do not apply to
it (upstream states this in its tests). Ristretto255 is a prime-order group, so
no cofactor or invalid-curve handling is needed. Selection remains conditional on the
open gates in DEPENDENCY_REVIEW.md (external review, side-channel/erasure review).

## D014 — Phase 1 core: one shared Rust crate, frozen v1 handshake and transport, 2026-11-04

Owner decisions (interview, 2026-11-04):

1. The security core is ONE shared Rust crate, core/kagilink-core (package
   areatm-kagilink-core), used by all platforms through FFI later. Native
   reimplementation per platform was rejected. This closes the open "native versus
   shared Rust core" question noted under D012.
2. This iteration builds the handshake and AES-256-GCM transport: CPace, explicit
   mutual key confirmation, HKDF key schedule, framing, replay/nonce rules and session
   lifecycle. Synthetic data only.
3. After the core is complete, the Android, iOS, macOS and cloud sessions port FFI
   bindings and vector passes in parallel. They must not start before the Windows
   session instructs them.
4. Attempt limiting is enforced solely by the receiver (Windows/macOS), which creates
   and displays the code: 60 s monotonic TTL from first display, never extended; at
   most 3 failed attempts; the code is destroyed at the limit or on success. The sender
   cannot extend or reset it and must ask for a fresh code after any failure.

Frozen v1 design: see PHASE1_CORE_DESIGN.md and PROTOCOL.md (roles, messages M1-M4,
CPace inputs, transcript hash, HKDF labels, confirmation order, attempt lifecycle,
transport nonces/AAD/framing). Roles: sender = CPace initiator, receiver = responder,
independent of BLE role. Any other version or suite is rejected (no downgrade).

Pinned dependency set: aes-gcm =0.11.1, hkdf =0.13.0, hmac =0.13.0, sha2 =0.11.1,
pakery-cpace / pakery-core / pakery-crypto =0.6.2, zeroize 1, subtle 2.6, rand_core
0.10, getrandom 0.4. RustSec check: no advisory entries for hmac, hkdf, subtle or
getrandom; historical entries RUSTSEC-2023-0096 (aes-gcm, fixed in >=0.10.3) and
RUSTSEC-2021-0100 (sha2, fixed in >=0.9.8) are below the pinned versions. This is an
advisory lookup, not an audit or a cargo audit/deny run on the full lock.

Unchanged: D012 safety-first applies. Real-secret transfer stays disabled until an
independent review of the exact locked versions exists, and Phase 0 exit criterion 3
stays open. Verification (local, 2026-11-04): 29 transport unit tests and 38 handshake tests pass (debug
build); fmt and clippy -D warnings clean. This is synthetic-test evidence only; no audit,
fuzzing, side-channel or memory review has been done.

## D015 — Lead branch and succession, 2026-11-04

Owner instruction: the Windows session's `main` is the lead (reference) branch of KagiLink.
Other platform work aligns to Windows `main` (core, `docs/PROTOCOL.md`, this file); on
conflict the Windows side wins.

Succession when the lead is unavailable: 1) Windows, 2) cloud session, 3) macOS session.
If none of them is available, work waits and makes no new decisions. Android and iOS are
not lead candidates.

This governs coordination only. It does not change any security policy, scope, phase gate
or the need for explicit owner approval in AGENTS.md.


## D016 — Core FFI boundary design (conditionally approved 2026-11-05)

Proposal only, awaiting owner approval: one `core/kagilink-ffi` crate (the only place with
`unsafe`) exposing a plain C ABI over opaque handles and caller-allocated buffers, header
generated by cbindgen, host-side approval/hardware/BLE, `kl_abi_version()` check, no
Rust-allocated memory returned, panics never unwind across the boundary. Details and open
risks: docs/FFI_DESIGN.md. Platform sessions stay on hold until the owner approves this and
the Windows (lead, D015) session instructs them. No code exists yet.


## D017 — Dependency advisory and license gate, 2026-11-05

`cargo audit` and `cargo deny check` (policy in `deny.toml`: advisories incl. yanked, wildcard
bans, allowed licenses Apache-2.0/MIT/BSD-3-Clause/Unicode-3.0, crates.io only) are the local
supply-chain gate for the pinned lock. Both passed on 2026-11-05. They check known advisories
and licenses only and are not an audit; independent review of the exact locked versions is
still required before real-secret use (D012). Wiring them into GitHub Actions is part of the
release-preparation work and is not done. Deterministic mutation tests were added; coverage-guided
fuzzing remains open.


## D018 — POSIX sh memory-access gate, 2026-11-05

Owner (lead session) approved `tools/test-ai-memory-access.sh` as an equivalent of
`tools/Test-AiMemoryAccess.ps1` so macOS/Linux sessions need not install PowerShell. Checks and
exit codes are identical: `gh` present, authenticated login exactly `areatmadm`, and
`areatmadm/kagilink-ai-memory` private, owned by areatmadm and pullable (exact, case-sensitive,
JSON boolean true). Any failure prints only the generic denial and exits 1. Verified on
Windows (Git Bash): allowed with the real login; denied with no `gh` on PATH; denied with an
invalid token. Also verified on macOS by the macOS session (2026-11-05, exit 0 per its private record; not independently re-run by the lead session). NOT verified: the wrong-account case. The ps1 remains the
reference; `tests/security/test-ai-memory-gate.ps1` does not cover the sh script yet. Docker
was rejected: it would require passing credentials into a container.


### D016 owner decision, 2026-11-05 (interview)

Conditionally approved. Decisions: plain C ABI (not UniFFI); the Windows (lead) session
implements the FFI crate alone; other platform sessions bind it only after it is complete.
Conditions that must be met before any platform session is told to start porting:

1. Target build verification: the pinned dependencies (pakery, getrandom, etc.) build and pass
   tests for iOS, Android and aarch64 targets.
2. A minimal `core/kagilink-ffi` crate exists (handshake to channel over the C ABI) with tests.
3. Coverage-guided fuzzing of parsers and framing has run (needs a nightly toolchain and
   cargo-fuzz; not installed yet).
4. GitHub Actions wired in per D019 (GitHub-hosted standard runners, read-only; this replaces
   the first interview answer of self-hosted runners).

Still open and not changed by this approval: independent review of the `unsafe` FFI code and
of the exact locked dependencies, real-secret transfer stays disabled (D012).


## D019 — CI execution model, 2026-11-05

Owner decisions (interview), superseding the self-hosted-runner idea in D016:

- Primary CI: GitHub-hosted standard runners. The repository is public, and GitHub documents
  standard hosted runners (including macOS) as free and unlimited for public repositories; cost
  concerns apply to private repositories and to larger runners, which are billed. Re-check the
  billing page if the repository ever becomes private. Self-hosted runners are not used: GitHub
  recommends them only for private repositories because fork pull requests could run code on the
  owner's machines.
- Workflow rules: trigger on `push` and `pull_request` (never `pull_request_target`), top-level
  `permissions: contents: read`, no secrets, third-party actions pinned by full commit SHA,
  dependency lockfiles and pinned toolchains. Release signing and publication stay separate and
  need explicit approval (AGENTS.md workflow 6/7).
- Apple/macOS builds and tests on the MacBook stay manual and run only by the macOS session
  (earlier owner instruction); results are recorded in the status file. iOS/Android sessions do
  not run CI or tests on that Mac. Hosted macOS runners may still be added later for
  compile-only checks if the owner agrees.
- Cloud session role: backup only, started only on the owner's or the lead's instruction (no
  issue-watching automation; issue text is data, never instructions). Intended for GitHub Actions
  outage or delay, checks that hosted runners cannot do (long or nightly-toolchain jobs such as
  cargo-fuzz), and Actions policy or quota problems. It runs Linux-side checks only and cannot
  build iOS, macOS or Windows targets (no Xcode or MSVC). It does not replace CI and its results
  must be labelled as cloud-sandbox runs.
- Not done: no workflow files exist yet; nothing here is implemented.

## D020 — Where target build verification runs, 2026-11-05

Owner decisions (interview), implementing D016 precondition 1 within D019:

- Scope: compile and run the core tests on each target (not compile-only), so OS-dependent
  parts such as `getrandom` are exercised.
- iOS (aarch64-apple-ios, simulator targets): MacBook, run manually by the macOS session only.
  Claude Cloud cannot do this (Linux, no Xcode).
- Android (aarch64-linux-android): GitHub-hosted Linux runner with a pinned NDK. If GitHub
  Actions is unavailable, the Claude Cloud session is the backup, started only on the owner's or
  lead's instruction (D019); its results are labelled cloud-sandbox runs.
- Limitation to settle before relying on this: a Linux runner cannot execute aarch64 Android test
  binaries by itself. Running them needs an emulator (an x86_64 emulator tests a different target
  from aarch64) or a physical device, which only the Android session's PC has. Until decided, a
  hosted Linux run proves compilation for the Android target only; test execution on Android is
  not claimed.
- Resolution of the limitation (owner interview, same day): Android test execution uses two
  complementary methods. (a) Physical device over adb, run manually by the Android session on
  its PC: the aarch64 test binary is pushed to a single directory under /data/local/tmp, run,
  and removed; synthetic data only, no root, no signing change, no app install, USB debugging
  required, and the owner instructs when to run. This is the only method that covers both real
  aarch64 and Android's bionic libc. (b) aarch64 emulation with qemu on the hosted Linux runner,
  automatic: it checks aarch64 code generation but runs a normal Linux libc, so it does not prove
  Android behaviour. An x86_64 emulator was not chosen (different target, different AES/SHA
  acceleration paths). A pass in (b) alone must not be reported as Android-verified.
- Windows (x86_64-pc-windows-msvc) already runs natively on the lead session's machine.
- Nothing has been run for iOS or Android yet; no workflow files exist.


### D016 progress, 2026-11-05

Minimal `core/kagilink-ffi` (package `areatm-kagilink-ffi`, C ABI v1, hand-written
`include/kagilink.h`) is implemented by a subagent and checked by the lead session. Deviations
from FFI_DESIGN.md: (1) `kl_sender_finish` empties the pending state but the handle shell must
still be released with `kl_sender_free`; a second finish returns WRONG_STATE. (2) If the caller
buffer for `open_fragment` is too small, the completed message is held inside the channel and
collected by a zero-length call with a bigger buffer; a non-empty fragment while one is held
closes the channel. (3) `panic = "abort"` is not set: Cargo ignores profile settings in member
crates, so it must be set at the workspace root or with `-C panic=abort` when packaging; until
then `catch_unwind` is the only defence. (4) No `test-hooks` feature; TTL is covered by core
ManualClock tests, not through the FFI. Not done: symbol visibility, CFI/hardening flags, CI check
that test-hooks stays off, independent review of the `unsafe` code. D016 preconditions: 2 (minimal
FFI crate) met for Windows x86_64 only; 1 (targets) partly (see status file); 3 (coverage-guided
fuzzing) and a real C/C++ consumer test are open.


### D019 correction and fuzzing venue, 2026-11-05

Owner chose GitHub-hosted runners for coverage-guided fuzzing. This corrects D019: long jobs such
as cargo-fuzz do NOT need the Claude Cloud session. GitHub-hosted Linux runners handle them;
Claude Cloud stays only a backup if GitHub Actions is unavailable. Fuzzing runs only through
`.github/workflows/fuzz.yml`, started manually (`workflow_dispatch`): no push or pull_request
trigger, so fork code never reaches it. Targets are in `core/kagilink-core/fuzz` (own
workspace and lockfile, audited in core-hardening.yml): `hello_decode`, `handshake_messages`,
`channel_fragments`. Pinned nightly `nightly-2026-11-01`. D016 precondition 3 is met as
infrastructure; the amount of fuzzing run so far is recorded in IMPLEMENTATION_STATUS.md and is
small, not an assurance level.


## D021 — Binding spike phase opened, 2026-11-05

D016 preconditions are met within the recorded limits (targets verified on iPhone, Galaxy S24 and
aarch64 qemu; minimal FFI crate; fuzz infrastructure; hosted CI; a real C consumer, `tests/c/consumer.c`,
71 checks passing with MSVC locally and with gcc plus ASan/UBSan in CI). On the owner's go-ahead the
platform sessions may now start a BINDING SPIKE only: link the Rust core through `kagilink.h` from
Swift (Apple), JNI/Kotlin (Android) and C++ (Windows, lead) and reproduce the same handshake,
fragmented SECRET round trip, wrong-code and tamper checks with synthetic data. Because every
platform uses the one shared core, separate CPace ports and per-language vector ports are no longer
planned; the draft-21 vectors stay in `core/cpace-eval` (Rust). Not authorised by this decision: real
secrets, UI changes, BLE, hardware-key use, store or release steps, Apple/Google account actions.
Still open before any real-secret use (D012): independent review of the `unsafe` FFI code and of the
exact locked dependencies, symbol visibility and hardening flags, `panic=abort` packaging, and
fuzzing of the FFI functions.


## D022 — Windows receiver in Rust; TPM trust study, 2026-11-05

Owner decision (interview): the Windows receiver is written in Rust and uses `areatm-kagilink-core`
directly, so Windows has no FFI boundary and no C++ memory-safety surface in the product path. This
changes the AGENTS.md Windows line (was C++20/C++/WinRT). The existing C++ Phase 0 probe and GUI stay as
research tools. The Rust receiver will use the official `windows` crate for WinRT BLE and Win32; its
dependencies need the usual review (D012) before selection. Nothing is implemented yet.

Owner goal: people who cannot buy passkey-style hardware should be able to move a password once,
easily, including from internet cafe (PC bang) machines, and the receiver should check whether the PC
can be trusted using the TPM. What can honestly be claimed, and what is unknown:

- A TPM can prove that a key lives in a TPM and was created by that TPM (a quote or key attestation
  signed over a fresh nonce from the phone), and can report boot measurements (PCRs, Secure Boot
  state). It cannot prove that the running OS, a service, a keylogger, a screen capture tool or an
  administrator tool on the PC is benign. Boot measurements need a known-good reference to mean
  anything, and PC bang images are custom. The decrypted password still reaches the target field in
  that PC's memory (AGENTS.md: compromised receivers can expose credentials).
- Verifying a TPM statement offline needs the EK/AK certificate chain and trusted vendor roots held on
  the phone; some vendors publish EK certificates only online, and revocation checking needs the
  network. Whether this fits the offline-only policy is UNVERIFIED.
- Mutual direction: Android key attestation and Apple App Attest could let the PC check the phone, but
  Apple's attestation needs a one-time network step on the phone, which conflicts with strict offline
  operation. UNVERIFIED how far each fits.
- Whether a normal-user process can create a TPM-backed key and obtain attestation through the Windows
  Platform Crypto Provider / TBS without administrator rights, and what PC bang management software
  allows, are UNKNOWN. No PC bang TPM statistics were found (public search, 2026-11-05).

Next step (feasibility, read-only, synthetic data): a small Rust Phase-0 style probe that reports TPM
presence/version, whether a TPM-resident key can be created as a normal user, and whether attestation
material is obtainable, run on the developer PC first and on volunteer PC bang machines only with the
owner's decision. Trust results will be labelled as claims until verified, as already required.


### D022 probe result on the developer PC, 2026-11-05

`apps/windows/rust/tpm-probe` (Rust, official `windows` crate =0.62.2, read-only) was run as a NORMAL
(non-elevated) user on the lead session's own PC (Gigabyte B450 desktop, Windows 11). Claims reported:
TPM 2.0 present through the TBS broker, hardware interface type, manufacturer string AMD (an AMD
firmware TPM); Secure Boot DISABLED (registry claim); TBS context, `TPM2_GetCapability` and
`TPM2_PCR_Read` (PCR 0 and 7, SHA-256) all allowed without administrator rights; the Platform Crypto
Provider opened and an unnamed ephemeral TPM P-256 key was created and freed as a normal user; the EK
public key was readable; the EK certificate property returned only 8 bytes, which is not a certificate
(meaning not confirmed; AMD firmware TPMs are known to need an online fetch for EK certificates, which
is recalled, not verified here). Consequences for the design: (1) the TPM is usable by a normal-user
portable process, which is the property the PC bang scenario needs; (2) an offline phone-side check of
the EK certificate chain cannot be assumed; (3) a policy that requires Secure Boot would reject this
very PC; (4) key attestation / quote with an attestation key was NOT tested yet. One probe bug was found
and fixed first: the TBS flag for TPM 2.0 is 0x4, not 0x2. This is one machine, not PC bang data.


### D022 attestation step result on the developer PC, 2026-11-05

The probe now also tries attestation as a NORMAL user through the Platform Crypto Provider. Results
(claims, same single PC): an ephemeral identity key (AIK) could be created without administrator
rights; the key-attestation claim (AIK vouching for a second TPM-resident key, 1703 bytes, magic
KAST) was produced and passed the OS self-check on the same machine; the platform claim (TPM quote over
PCR 0 and 7 plus the boot log, 1187 bytes, magic ALPT) was produced once the PCR mask was passed as 4
bytes (a 3-byte mask failed with NTE_INVALID_PARAMETER). What this does and does not establish: the
Windows stack lets an unprivileged process obtain TPM-signed claims, so the mechanism is feasible for a
portable receiver. It does NOT yet give a phone any reason to trust the PC: the AIK was not activated
against an EK certificate (this PC's EK certificate is not in NV), the claim blobs were not parsed, no
phone-side verifier exists, offline verification of AMD firmware TPM roots is unresolved, and a quote
only attests boot state (here with Secure Boot disabled), not that the running system is free of
keyloggers. Next design questions for the owner: what the phone should require (hardware-bound key
only, a particular boot state, or both), whether an online EK-certificate step is acceptable for a
one-time enrolment, and whether to treat the check as a user-visible warning rather than a gate.


## D023 — Risk notice and user responsibility, 2026-11-05

Owner decision: KagiLink is a personal open-source project and the end user bears the responsibility
for using it. Draft text (Korean and English) is in docs/RISK_NOTICE.md; it is NOT lawyer-reviewed and
its enforceability is not promised. Consequences: the sending app shows the short notice and requires
explicit acknowledgment before the first transfer to a receiver and whenever the receiver changes
(AGENTS.md); documentation states that keyloggers on the receiving PC cannot be detected or prevented;
UIs never call a PC "safe"; device/TPM information is shown as a claim. The disclaimer does not relax
any technical requirement (PAKE, key confirmation, TTL, approvals, fail-closed behaviour) and does not
replace the recommendation to send only one-time or soon-to-be-changed passwords.


### D023 clarification: design effort versus detection duty, 2026-11-05

Owner direction: the design uses every available resource to be safe, but KagiLink has no duty to
detect keyloggers or other malware already installed on a receiving PC (software or hardware), and
it must not claim to. Meaning: (1) everything KagiLink itself controls stays at the strongest
practical level (PAKE, key confirmation, TTL, approvals, zeroization, hardware-backed keys where
available, fail-closed behaviour, minimal exposure of plaintext); (2) detection of pre-existing
compromise is out of scope and is not a requirement, a release gate or a test goal; (3) optional,
clearly labelled best-effort hints (for example reporting the TPM/boot-state claims) may be added
but are never presented as a safety verdict; (4) docs/RISK_NOTICE.md says so in plain words.


## D024 — TPM trust policy for the receiver (owner interview answers), 2026-11-05

Owner answers, recorded as given:
1. Criteria: hardware binding PLUS a displayed boot state. The phone requires the receiver's key to
   be TPM-resident; Secure Boot and PCR state are SHOWN to the user, not required.
2. Online EK certificate: allowed as an OPTIONAL feature.
3. Below the criteria: BLOCK the transfer.

Consequences and conditions written down so they are not lost:
- The online EK-certificate fetch is an exception to the offline-only policy. It must be optional,
  started by the user, one-time per receiver, outside the authentication/secret-transfer path, carry
  no secrets, and be described honestly: the EK public key is a stable hardware identifier and the
  vendor server that serves the certificate learns it and the requester's network address. No account,
  telemetry or analytics is introduced. The exact vendors and endpoints are not yet researched.
- Without the EK certificate chain the phone cannot tell a real TPM claim from a forged claim blob, so
  the "TPM-resident key" requirement is only a weak signal (it stops honest misconfiguration, not an
  attacker who controls the PC). The strong version needs the optional online step and a phone-side
  verifier that is not built yet. UIs must not present the weak version as proof.
- "Block" fixes the criterion as hardware binding, so a PC with no usable TPM cannot receive. This
  conflicts with AGENTS.md ("TPM 2.0 preferred; accurately labelled RAM-only temporary mode allowed")
  and may exclude older internet cafe PCs. UNRESOLVED until the owner decides whether RAM-only mode is
  removed for sensitive transfer; AGENTS.md is not changed by this record.
- Still unchanged from D023: no duty to detect already-installed keyloggers; the boot-state display
  is information, not a verdict.


### D024 resolution of the open conflict, 2026-11-05

Owner answer: RAM-only mode stays, with a warning only. Effective policy for a Windows receiver:
(a) TPM reports a hardware-bound key that passes the checks: proceed, showing boot state;
(b) a TPM is present but the claim fails or is invalid: BLOCK;
(c) no TPM is reported or usable: accurately labelled RAM-only mode with a strong, explicit warning
and the D023 acknowledgment. Honest limit: the phone cannot tell a PC without a TPM from a
compromised PC that says it has none, so (c) is reachable by anyone controlling the receiver and the
block in (b) mainly catches faults, not an attacker. The TPM check therefore is a trust signal for
honest setups, not a security boundary, consistent with D023 (no duty to detect compromise). AGENTS.md
keeps its RAM-only allowance and gets a pointer to this policy.


## D025 — BLE/GATT mapping draft and framing codec, 2026-11-06

Owner asked for the GATT specification. The lead session drafted docs/BLE_GATT_SPEC.md (framing v1)
and implemented its chunk codec in `core/kagilink-core/src/ble_frame.rs` (shared with the phones
through the C ABI later). Draft choices: receiver = Central, sender = Peripheral advertising only
after the user starts it; one service with two characteristics (RX write-with-response, TX indicate);
no reliance on BLE pairing/bonding/encryption (all security is application layer); strict in-order
chunk framing with a 2-byte header and a 3-byte first-chunk prefix, messages up to 512 bytes, working
at the 23-byte minimum MTU; one connection, 60 s session bound, any error drops the connection; no
code-derived advertising data. Fresh random UUIDs replace the Phase 0 diagnostic UUIDs, which must not
become production. Status: DRAFT. Verified so far: the codec's local synthetic tests only. Open
questions listed in section 9 of the spec (Indicate vs Notify fallback, address rotation, optional
display tag for choosing between several senders, platform implementations).
