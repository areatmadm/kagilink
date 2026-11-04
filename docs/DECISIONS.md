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
