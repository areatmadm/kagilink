# KagiLink cryptographic dependency review — preliminary, 2026-11-04

No cryptographic runtime dependencies are selected/installed. The synthetic model
uses std only. No claim of a clean vulnerability scan is made.

## Native diagnostic tooling — 2026-11-04

Windows uses the existing MSVC/Windows SDK and OS WinRT libraries. Apple source
uses only SwiftUI/Core Bluetooth/CryptoKit platform frameworks, no Swift packages.
The iOS build requires SDK 27.1 and deploys to iOS/iPadOS 18+; the macOS target
deploys to Apple Silicon/macOS 14+. Actual Xcode compilation and hardware validation
remain pending on Mac. [Apple's Duo guidance](https://developer.apple.com/iphone-duo/prepare/)
explains SDK 27.1's layout behavior. Native Liquid Glass calls are availability-guarded;
iOS 18 uses system material and no external glass library.

Android research configuration pins AGP 9.5.0 with its built-in Kotlin 2.2.10 integration,
Gradle 9.8.2 and SDK build tools 37.0.0. The selected latest stable platform package
is `platforms;android-37.2`, revision 2; compile API is 37 minor 2, target is 37 and
minimum is 34 (Android 14). These are research pins, not approval for production
cryptography. [Google's SDK metadata](https://dl.google.com/android/repository/repository2-3.xml)
provides the stable package evidence and [AGP 9.5 release notes](https://developer.android.com/build/releases/agp-9-5-0-release-notes)
describe compatibility. [Kotlin release history](https://kotlinlang.org/docs/releases.html)
provides upstream maintenance context; the built-in Kotlin version is tied to the
pinned AGP toolchain rather than independently claiming the newest compiler.
End-to-end platform support and a complete build-tool advisory/license review
remain unresolved until SDK QA.

Gradle 9.8.2 is pinned with official binary distribution SHA-256
`21bd5da2de49b642904077f38c0efdc7778e92b3ed0889392dca61b97e389607`
from [Gradle's checksum reference](https://gradle.org/release-checksums/).
Actual SDK-independent dependency-resolution outcomes are recorded separately in
IMPLEMENTATION_STATUS.md. Generated root buildscript and app lockfiles are source
candidates; lock generation is not APK compilation, lint or device evidence.
Final Gradle 9.8.2 `buildEnvironment` and all app dependency reports completed with
exit 0 and no unresolved `FAILED` rows; a follow-up offline runtime resolution also
passed without rewriting locks. [Built-in Kotlin guidance](https://developer.android.com/build/migrate-to-built-in-kotlin)
explains why no separate Kotlin Android plugin is applied.

The current app debug/release runtime lock contains Kotlin stdlib 2.2.10 and
JetBrains annotations 13.0 only. Scoped public GitHub advisory API queries for
those exact Maven coordinates returned zero matches on this date. This covers
neither the full build-tool graph nor unpublished issues, and is not a complete
vulnerability audit. Future checks must use the exact final lock versions.
[Kotlin license](https://github.com/JetBrains/kotlin/blob/v2.2.10/license/LICENSE.txt),
[annotations license](https://github.com/JetBrains/java-annotations/blob/master/LICENSE.txt)
and [Gradle license](https://github.com/gradle/gradle/blob/v9.8.2/LICENSE) are
Apache-2.0. Full transitive build-tool license/NOTICE/SBOM and advisory checks are
still required before release. Build-tool-only AGP/Kotlin analytics/telemetry
libraries appearing in the lock are not app dependencies or shipped SDKs. No build
scan was requested; the app manifest grants no network permission. Android glass
styling uses platform code and adds no UI runtime SDK. Validate resolved APK
contents and offline behavior on the first SDK/device build.

| Candidate | Evidence | Decision / remaining gate |
|---|---|---|
| RustCrypto spake2 | [Upstream](https://github.com/RustCrypto/PAKEs), [crate](https://docs.rs/spake2/latest/spake2/) | Apache-2.0/MIT; upstream audit/erasure warnings; not approved for real secrets |
| RustCrypto aes-gcm | [Documentation](https://docs.rs/aes-gcm/latest/aes_gcm/) | Apache-2.0/MIT, historical audit noted; verify selected release, CPU constant-time behavior and zeroization |
| RustCrypto hkdf/sha2/hmac | [HKDF](https://docs.rs/hkdf/latest/hkdf/) | Candidate; exact versions, advisories, target and API review pending |
| zeroize | [Documentation](https://docs.rs/zeroize/latest/zeroize/) | Apache-2.0/MIT; optimizer-resistant erasure, not copy/swap immunity; candidate |
| OS entropy / getrandom | OS CSPRNG or reviewed Rust adapter | Exact dependency and failure behavior pending; no weak fallback |

RustCrypto PAKEs explicitly warns about lack of formal review and secret erasure.
Repository activity and a recognized author do not equal cryptographic audit.
Compare selected SPAKE2 group/encoding/confirmation behavior against RFC 9382
instead of assuming the crate name proves RFC compatibility.

Before adoption: inspect exact release source and recent maintenance, compare
audit scope to that version, query RustSec advisories for the complete locked
dependency graph, check Apache compatibility/NOTICE obligations, test Windows,
Android and Apple targets, inspect secret clones/drop/error paths and freeze
reference vectors. Record version, checksums, evidence and review date here.
Pin direct versions and Cargo.lock; use cargo audit/deny plus fuzzing in Phase 1.
A dependency-free Phase 0 test is not a PAKE or AES security validation.

## SPAKE2 candidate review — 2026-11-04 (Phase 0 exit criterion 3)

Outcome: **no candidate is approved.** This is evidence gathering, not a selection.
Phase 1 production crypto stays gated until a candidate and its caveats are accepted
in DECISIONS.md.

| Candidate | Facts observed (crates.io / GitHub API / upstream README, 2026-11-04) | Gap against policy |
|---|---|---|
| RustCrypto `spake2` | Latest stable 0.4.0 (2023-07-28); 0.5.0-pre.0 (2026-01-25, MSRV 1.85); 0.3.0 yanked; MIT OR Apache-2.0; RustCrypto/PAKEs repo pushed 2026-10-28, not archived; only `Ed25519Group`, documented as compatible with python-spake2 | README states it has **never received an independent audit**; it is not an RFC 9382 suite (its README links IETF draft-10), so RFC 9382 vectors, key schedule and confirmation do not apply to it; erasure/lifecycle caveat remains |
| `pakery-spake2` 0.1.0 (pakery workspace 0.6.2, 2026-03-07) | MIT OR Apache-2.0; claims RFC 9382 test-vector validation, `forbid(unsafe_code)`, constant-time compares, zeroization; about 3.8k downloads of the workspace crate; single upstream repository | Self-reported only; young, small user base, no audit found; vector claim must be reproduced by us |
| `spake2plus` | SPAKE2+ (RFC 9383), not SPAKE2; README says no third-party review | Different protocol; out of scope unless the design changes |

Other facts: RFC 9382 is informational. Its suites are P-256/P-384/P-521 (SHA-256/512,
HKDF, HMAC or CMAC) and edwards25519/448; roles A/B (symmetric M=N variant if roles
cannot be assigned); TT binds identities, both messages, K and w; Ka/Ke split from
Hash(TT); explicit confirmation MACs are mandatory; w must come from a memory-hard
function and the RFC leaves parameters and online-guess rate limiting to the
protocol. KagiLink's own limits (TTL 60 s, three failures, one-time code) therefore
remain our responsibility. RustSec advisory-db has no entry named for `spake2`,
`hkdf` or `zeroize` as of this check; `aes-gcm` (RUSTSEC-2023-0096) and `sha2`
(RUSTSEC-2021-0100) have historical entries whose applicability to the final locked
versions is not yet checked. An empty advisory result is not an audit.

Open decisions needed before Phase 1 code: (1) accept an unaudited library with
mitigations, or fund/seek review; (2) choose RFC 9382 P-256 suite (maps to platform
P-256 on Apple/Android/Windows) versus the Ed25519 python-spake2 variant; (3) native
Swift/Kotlin implementations versus one shared Rust core via FFI, which changes the
audit surface. None of these is decided here.

## CPace vs SPAKE2 — 2026-11-04 (safety-first comparison)

Reason: the CFRG PAKE selection recommended **CPace** (balanced) and OPAQUE
(augmented) for IETF use; SPAKE2 predates that process and RFC 9382 is informational.
KagiLink needs a balanced PAKE (one-time code, no server). Sources: IETF datatracker
and crates.io/GitHub APIs, queried 2026-11-04. Nothing here is a security audit.

| Item | CPace | SPAKE2 (RFC 9382) |
|---|---|---|
| Standing | CFRG recommended; `draft-irtf-cfrg-cpace-21` is in state "Sent to the RFC Editor" (publication pending, no RFC number seen) | RFC 9382, informational, not selected by the CFRG process |
| Recommended suites | CPACE-X25519-SHA512, CPACE-P256-SHA256, CPACE-X448-SHAKE256, CPACE-P384, CPACE-P521; Ristretto255/Decaf448 in section 8.3 | P-256/384/521 and edwards25519/448 with HKDF and HMAC or CMAC |
| Vectors | Appendix B vectors per suite, including invalid-input and low-order-point cases | RFC appendix vectors |
| Analysis | Published proofs (AHH21, BGHJ24); only a Sage reference implementation is named in the draft | Proof for prime-order groups; no wider analysis by the CFRG process |
| Key confirmation | Not built in; the draft recommends adding it when forward secrecy is required. KagiLink policy already requires explicit mutual confirmation, which we must add | Built in (cA/cB MACs) |
| Rust candidates | `pakery-cpace` 0.6.2 (updated 2026-10-26, MIT OR Apache-2.0, Ristretto255 and P-256 suites); `pake-cpace` 0.1.7 (ISC, last update 2023-12, written against early drafts, not evaluated) | `pakery-spake2`; RustCrypto `spake2` 0.4.0 (Ed25519, unaudited, not RFC 9382) |
| Maintenance risk | pakery is a single-maintainer workspace (about 2 GitHub stars, 4.7k crate downloads) with fuzz targets, dudect timing tests and `deny.toml` in its repository; none of this is an independent audit | same workspace |

Findings: no audited implementation of either protocol was found. CPace has the
stronger standards position and a smaller state machine, but a younger library
ecosystem. Final choice requires our own check that the chosen crate reproduces the
latest draft's Appendix B vectors (the draft changed across revisions 17 to 21).
Adopting CPace would change the AGENTS.md wording "Require SPAKE2" and needs the
owner's explicit approval plus a DECISIONS.md entry; this file does not make that change.

### CPace evaluation result — 2026-11-04

Independent conformance run: `core/cpace-eval` (publish = false, dev-dependencies only)
pins `pakery-cpace`/`pakery-core`/`pakery-crypto` `=0.6.2` and tests the
CPACE-RISTR255-SHA512 suite against draft-irtf-cfrg-cpace-21 Appendix B.3. The vectors
were extracted from the draft text by `tools/extract-cpace-vectors.py` (draft text
SHA-256 `ed2772c26c21d43a199d490c1...` is embedded in the generated file), not copied
from the crate's own tests.

| Check | Outcome |
|---|---|
| generator_string and generator g | PASS |
| transcript_ir / transcript_oc | PASS |
| optional session-id output (both modes) | PASS |
| End to end with fixed scalars: Ya, Yb and ISK, both initiator/responder and symmetric modes, both parties agree | PASS |
| Wrong password yields different keys | PASS |
| Identity share rejected (responder and initiator) | PASS |
| Malformed shares (empty, 31/33 bytes, non-canonical 0xff..) rejected | PASS |
| cargo fmt, clippy -D warnings, workspace tests (7 new + 11 existing) | PASS |

Finding that changes the plan: the crate's **P-256 suite is not the draft's** (SHA-512
and DSI `CPaceP256`, per upstream's own test notes), so the draft's P-256 positive
vectors cannot validate it. Use the Ristretto255 suite. X25519 and P-256 (draft form)
have no tested Rust path here.

Not done: negative-point vectors from B.3.10/B.3.11, parallel-execution edge cases,
fuzzing, dudect/timing review, zeroization/memory review of the crate, cargo audit/deny
(tools not installed on this host), non-Rust implementations (Swift/Kotlin) and any
independent audit. The Ristretto255 suite is described by the draft but is not in its
RECOMMENDED list (X25519 and P-256 are), which is a residual standards-position
caveat. Two lines differ in the upstream crate: its lib.rs says draft-18 while the code
and tests cite draft-21; reproduced vectors match -21.
