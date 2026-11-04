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
