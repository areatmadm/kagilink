# KagiLink implementation status — 2026-11-04 (Asia/Seoul)

Phase 0 is IN PROGRESS. Real-secret transfer is disabled. This is not release ready.

## Environment

Historical initial workspace: empty S:\project\pwreceiver; not a Git repository.
Current workspace: S:\project\kagilink, verified after the local folder move.
Source publication is separate from a product release. The source/memory Git split
uses a clean source history, an independent ignored private repository at
shared/ai/, and a private archive for earlier mixed history. See AI_MEMORY.md.
Original conversation prompts/transcripts are excluded from the source snapshot.
No release or binary was published.

- Source: https://github.com/areatmadm/kagilink — PUBLIC, existing source history preserved.
- Memory: areatmadm/kagilink-ai-memory — PRIVATE, only owner listed as collaborator.
- Earlier mixed history: areatmadm/pwreceiver-private-archive — PRIVATE.
- Source full local object history and remote tree contain no memory/original-prompt
  paths; no submodule/gitlink. Earlier local Git metadata preserved under ignored target/.
Rust/cargo 1.92.0, MSVC toolchain target; fmt/clippy installed. Visual Studio 2022
Community and Windows SDK 10.0.22621.0 / 10.0.26100.0 detected.

## Work

| Phase | State | Evidence / remaining work |
|---|---|---|
| 0 | In progress | Native diagnostic sources added; Windows CLI built, mobile/Apple SDK and device validation pending |
| 1 | Not started | Dependency/suite gate unresolved; no production PAKE/HKDF/AES/zeroization |
| 2 | Not started | No production BLE transport; separate fixed-public-value diagnostic source only |
| 3 | Not started | No verified StrongBox/SE/TPM key integration |
| 4 | Not started | No native product UI/vault |
| 5 | Unsupported/pending | RDP/CredUI disabled; no safe credential input evidence |
| 6 | Not started | No fuzzing/advisory/interoperability/security audit |
| 7 | Not started | No release/signing/store/server actions |

## Validation

| Check | Actual local outcome |
|---|---|
| cargo generate-lockfile --offline | PASS, no third-party dependencies |
| cargo test --workspace --locked --offline | PASS, 11 unit tests, 0 failures; 0 doc tests |
| cargo fmt --all -- --check | PASS |
| cargo clippy --workspace --all-targets --locked --offline -- -D warnings | PASS |
| apps/windows/build-probe.ps1 | PASS, MSVC C++20 /W4 /WX /MT; portable research EXE built |
| Historical Phase 0 probe --policy | PASS, real-secret transfer disabled and RDP unsupported |
| Historical Phase 0 probe --inventory | INCOMPLETE, diagnostic-only restricted process; native exit 2 |
| Historical Phase 0 probe --self-input | BLOCKED safely by integrity guard; no input performed |
| GitHub-hosted clean-source CI | PASS, run 41760663274: fmt, clippy, 11 model tests, 9 memory-gate cases |
| Private-memory identity gate | PASS, 9 mocked cases; actual owner allow and unavailable-auth deny verified |
| Real devices / crypto / interoperability / fuzzing | NOT RUN |

Inventory observed PROCESS_INTEGRITY_RID=4096 (Low), BLE LE and Central capability
reported, and TPM provider opened. This proves only access to those queries in
this restricted execution context, not normal-user medium-integrity feasibility,
active radio, GATT exchange, TPM 2.0, key protection or safe input. The input probe
requires Medium integrity and deliberately refuses this environment. No guard
was weakened to allow synthetic input at Low integrity. A user-run normal desktop
test is still required; --self-input can briefly take focus and is opt-in only.

Tests not explicitly recorded as run are NOT RUN. Synthetic model success is not
cryptographic evidence. No production memory zeroization has been implemented.

## Next gates and limitations

1. Run the built probe on a normal medium-integrity desktop and record results;
   perform opt-in own-window synthetic input only in a suitable test environment.
2. Real foreground mobile peripheral + Windows Central GATT exchange including
   MTU/disconnect/permission/radio-off tests; identify adapter/OS configurations.
3. Verify input safety separately for ordinary windows, CredUI and mstsc. Current
   official restrictions prevent claiming supported non-elevated credential input.
4. Resolve audited/reviewed SPAKE2 suite and memory lifecycle, then freeze vectors.
5. Only after Phase 0 exit, implement security core and native hardware adapters.

No real credentials have been used. No production memory-clearing guarantees,
hardware attestation, secure target input, interoperability or full offline
integrity have been established. Private handoff access instructions are in AI_MEMORY.md.

## KagiLink migration

Repository rename completed in place: same repository IDs, original main branch
HEADs and public/private visibility verified immediately after rename. Existing
history is preserved; no reset, replacement repository or force push. See
MIGRATION_KAGILINK.md for scope, compatibility decisions and unimplemented targets.
Migration commit ac4fc2f preserves the original source HEAD as an ancestor.
Repository IDs, default branches, permissions and owner-only collaborator lists
were rechecked through GitHub after the push; no visibility changes occurred.

| Migration regression check | Result |
|---|---|
| cargo fmt --all -- --check | PASS |
| cargo clippy --workspace --all-targets --locked --offline -- -D warnings | PASS |
| cargo test --workspace --locked --offline | PASS, original 11 model tests retained |
| tests/security/test-ai-memory-gate.ps1 | PASS, 10 cases including historical-name rejection |
| Updated private-memory gate + initializer | PASS, actual areatmadm access to renamed private repository |
| Windows MSVC C++20 /W4 /WX build | PASS, kagilink-phase0-probe.exe |
| New probe --help / --version / --policy | PASS (exit 0), KagiLink text and disabled real-secret transfer |
| Windows product resource | PASS, KagiLink/AreaTM/version/original filename/copyright verified |
| New probe --inventory | Same limited diagnostic result: Low RID 4096, LE/Central/provider reported, exit 2 |
| New probe --self-input | Same safe refusal at Low integrity, exit 2; no keystrokes injected |
| Source/private pre-migration commit ancestry | PASS, both original HEADs remain ancestors |
| Source history/ignore isolation | PASS, no private-memory/original-prompt paths or gitlinks |
| Renamed-repository GitHub Actions | PASS, run 41762980276 on ac4fc2f; original workflow ID 406436318 preserved |

Local Rust tooling reported an incremental-cache hard-link fallback to copying;
all commands exited successfully. Real-device, Medium-integrity input and RDP
support remain unverified. No additional production security capability is claimed.

GitHub confirms an empty set of private-memory/original-prompt paths in the public
tree. Source candidate files and local links passed review with no credential
pattern matches; this is a bounded check, not a guarantee against every secret
format. Changed file list and handoff/compatibility notes: MIGRATION_KAGILINK.md.

## Local folder relocation verification — 2026-11-04

- Source and independent private-memory Git roots resolve under S:\project\kagilink;
  both worktrees were clean before this verification. Both `git fsck --full`
  checks passed, and origin URLs retain the KagiLink repository names.
- Source still ignores shared/ai/, tracks no files there and contains no gitlinks.
- Rust tests: 11 passed, including a fresh build in target/relocation-check;
  fmt and clippy passed offline with the locked dependency set.
- Memory-gate mock tests: all 10 passed. Actual owner/private-repository access
  passed on a network-enabled retry after the restricted runner denied access.
- Windows probe rebuilt from the new path with MSVC /W4 /WX. Help, version and
  policy exited 0. Inventory exited 2 at Low integrity (RID 4096), with the same
  limited LE/Central/provider diagnostics; this remains incomplete feasibility.
- No active source/build-script reference to the old absolute workspace path
  was found. Historical migration notes remain preserved.
- Rust reported the existing hard-link-to-copy cache fallback; checks succeeded.
  The restricted shell also emitted an InitializeDefaultDrives warning without
  preventing these checks. Interactive input, real devices and RDP were not tested.

## Native feasibility targets — 2026-11-04

Windows Central, Android/iOS foreground Peripheral and macOS Central/Peripheral
diagnostic sources now use the isolated fixed-public-value service documented in
PHASE0_BLE_PROBE.md. This is Phase 0 work, not completion of the native product
apps or approval to enable real-secret transfer. No PAKE, key, vault, credential
input, pairing code, peer authentication or production wire protocol was added.

Required mobile floors are Android 14/API 34 and iOS/iPadOS 18. Android selects
the latest stable platform package 37.2 (revision 2), target 37, build tools 37.0.0,
AGP 9.5.0 and Gradle 9.8.2 with built-in Kotlin. The Apple iOS target selects SDK
27.1 explicitly for iPhone Duo, while retaining iOS 18 as its deployment floor.
Native Apple glass is guarded for supported OS versions; iOS 18 uses system
material and Android uses platform glass styling. Adaptive/accessibility code is
source-only until device rendering is checked. macOS remains Apple Silicon/macOS 14+.

| Check for native diagnostic changes | Actual outcome |
|---|---|
| Windows MSVC C++20 /W4 /WX build | PASS, new diagnostic client compiled |
| Windows noninteractive CLI regression | PASS, 6 checks; no scan or input injection |
| Direct `--ble-test` in restricted runner | Safe refusal at Low RID 4096, exit 2; no scan |
| Rust locked/offline model tests | PASS, 11 tests; fmt and clippy also passed |
| Memory-access gate mock suite | PASS, 10 cases |
| Native source/project consistency | PASS, 7 checks; UUIDs, permissions, locks, mobile floors/SDK settings, glass guards, Apple plists/project references |
| Public Markdown local links | PASS, final integrated check across 31 files, including OS READMEs; private-memory paths excluded |
| Apple build-helper shell checks | PASS, shell syntax and 4 synthetic-tool cases for SDK selection/unsigned invocation/rejection; no Xcode compiler invoked |
| Android final Gradle project/dependency configuration | PASS, Gradle 9.8.2 root/app locks generated; no unresolved dependency rows; offline runtime resolution passed without lock rewriting |
| Android exact runtime advisory lookup | Zero public GitHub advisory matches for Kotlin stdlib 2.2.10 and annotations 13.0; scoped lookup only, not a full audit |
| Android APK compilation/lint/device | NOT RUN, Android SDK absent on this host |
| Apple compilation/signed sandbox/device | NOT RUN, Xcode and Apple devices unavailable on this host |
| Liquid Glass appearance/accessibility/Duo folding | NOT RUN; source checks are not visual/device evidence |
| Actual cross-OS GATT exchange/notifications/MTU/disconnect | NOT RUN |
| New GitHub Actions Windows/static/shell-fixture checks | Prepared locally; not pushed or executed remotely |

The diagnostic can be spoofed/replayed and cannot prove proximity or device
integrity. Apple peripheral disconnect observation is incomplete; abandoned
public state expires. Device access/consent, cleanup timing and fixed reply delivery
still require real hardware. The receiving Windows executable remains a research
CLI, not a completed receiver product UI. No account/certificate/provisioning,
SDK-license acceptance, device installation, store/release or server action occurred.

## Windows executable smoke test — 2026-11-04

- Rebuilt the EXE with MSVC C++20 /W4 /WX. Product metadata reports KagiLink,
  AreaTM, 0.0.0-phase0 and kagilink-phase0-probe.exe.
- Noninteractive CLI suite: 6 checks passed in both restricted and normal-user
  execution. Negative cases now require refusal/usage output and reject diagnostic
  scan/input execution markers, rather than checking exit code alone.
- Normal-user read-only inventory ran at Medium integrity (RID 8192), exit 0:
  LE/Central capabilities reported and TPM provider opened. No TPM key or TPM 2.0
  verification occurred. This adds normal-user query evidence to the earlier
  restricted-runner observations; it does not complete the Phase 0 gate.
- Normal-user `--ble-test` ran for 10,191 ms, reported NO_DIAGNOSTIC_SENDER and
  exited 2. This is the expected unavailable result with no diagnostic sender
  observed; no GATT exchange, peer authentication or successful transfer is proven.
- No interactive self-input, credential UI or RDP test was run. No elevation,
  pairing, system settings, certificates or real credentials were involved.
- Tested EXE SHA-256:
  `f2b5a7fe549e6fed512c7098aca13399ceb682ce331865109adaf6140edd0400`.

## Android sender build and first BLE diagnostic result — 2026-11-04

Built and run on a Mac host with the pinned tools installed on an external volume:
Android SDK platform 37.2 (revision 2), build tools 37.0.0, Gradle 9.8.2 (distribution
SHA-256 matched the published value). SDK licenses were accepted by the user for this
installation. No signing identity exists; the unsigned debug APK was signed locally
with a throwaway debug keystore only so it could be installed. It is not a release
artifact and the key must not be reused.

- `:app:assembleDebug` PASS (one existing `setDecorFitsSystemWindows` deprecation
  warning). `:app:lintDebug` NOT RUN.
- First launch on a Xiaomi 2405CRPFDL (wireless adb) crashed in
  `MainActivity.onCreate` with a NullPointerException: `window.insetsController` was
  read before `setContentView` created the decor view. Fixed by moving the
  `setSystemBarsAppearance` call after `setContentView`; nothing else changed. After
  the fix the app stayed in the foreground with no new crash-buffer entries.

User-reported diagnostic output (screenshots; command line, host and timestamps were
not captured). `PROCESS_INTEGRITY_RID=8192` matches the Windows probe at Medium
integrity, so this is attributed to the Windows `--ble-test` receiver:

| Reported line | Meaning recorded here |
|---|---|
| `SYNTHETIC_GATT_WRITE_READ_PASS` | One fixed public-value write/read round trip succeeded |
| `SCANNING_DIAGNOSTIC_SERVICE_10_SECONDS` | Scan window for the diagnostic service was 10 s |
| `NOTIFICATION_MTU_FRAGMENTATION_NOT_TESTED` | Notifications and MTU fragmentation remain untested |
| `PHASE0_PUBLIC_UNAUTHENTICATED_BLE_TEST`, `NO_PEER_IDENTITY_OR_HARDWARE_VERIFIED`, `NOT_AUTHENTICATION_NOT_PRODUCTION_PROTOCOL` | No authentication, peer identity or hardware verification |
| `REAL_SECRET_TRANSFER_DISABLED`, `RDP_INPUT_UNSUPPORTED` | Real-secret transfer and RDP input stay disabled/unsupported |

This is the first recorded Android-peripheral to Windows-Central exchange. It
supersedes the earlier `NO_DIAGNOSTIC_SENDER` result only for that single fixed-value
round trip. It does not prove proximity, authentication, StrongBox/key behavior,
MTU/notification handling, disconnect/permission/radio-off behavior, other adapters
or OS versions, Android lint, TalkBack/large-font/visual layout, or any production
transfer. The Phase 0 gate remains open.
