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
| 0 | In progress | Native diagnostic sources added; Windows GUI/CLI built and diagnostic window exercised, mobile/Apple SDK and device validation pending |
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

## CPace evaluation (Phase 1 entry work) — 2026-11-04

Decision D013 replaced SPAKE2 with CPace. `core/cpace-eval` runs `pakery-cpace` 0.6.2
(Ristretto255 + SHA-512) against draft-irtf-cfrg-cpace-21 Appendix B.3: 7 conformance
tests PASS, plus existing 11 model tests; fmt and clippy -D warnings PASS. This is
candidate evidence only. Phase 0 exit criterion 3 is NOT closed: no audit, no
side-channel/erasure review, no cargo audit/deny, no cross-language vectors. Phase 1
production code remains gated; real-secret transfer stays disabled.

## Phase 1 core prototype (core/kagilink-core) — 2026-11-04

Implemented per D014: CPace (pakery-cpace 0.6.2, Ristretto255) handshake with explicit
mutual key confirmation, HKDF-SHA-512 key schedule, receiver-enforced 60 s TTL and
three-failure budget, AES-256-GCM framed transport. `aes-gcm` is built with its
`zeroize` feature. Built in parallel (transport, tests and docs by separate agents,
handshake by the main session) and integrated here.

| Check | Actual outcome |
|---|---|
| cargo fmt --all | PASS |
| cargo clippy --workspace --all-targets -- -D warnings | PASS |
| cargo test --workspace (debug) | PASS: 29 transport unit tests, 38 handshake tests, 7 CPace draft vectors, 11 Phase 0 model tests |
| Two handshake tests initially failed because of test bugs (flipped a suite byte instead of a sid byte; used the wrong receiver's code); fixed in the tests, no implementation change | recorded for transparency |
| Coverage-guided fuzzing, constant-time/side-channel review, memory-dump review, independent audit | NOT RUN (see hardening addendum below for what was run) |
| Cross-language vectors, FFI, BLE mapping, hardware key use, real-secret transfer | NOT DONE; real-secret transfer remains disabled |

Known limits: `SecureChannel::new` and the key schedule copy 32-byte keys by value, so
transient stack copies are not tracked; `Aes256Gcm` is rebuilt per operation. Errors
The error `AttemptInProgress` is defined but the handshake counts an abandoned attempt as a plain
failure. Phase 0 exit criterion 3 stays open.

### Phase 1 hardening addendum — 2026-11-05 (D017)

| Check | Actual outcome |
|---|---|
| cargo audit (RustSec db, 1403 advisories, 47 crates in Cargo.lock) | PASS: exit 0, no vulnerabilities or warnings reported |
| cargo deny check (deny.toml: advisories, bans, licenses, sources) | PASS (advisories ok, bans ok, licenses ok, sources ok) |
| tests/mutation.rs (10 deterministic fixed-seed mutation tests, 300 rounds each) | PASS: mutated M1/M2/M3/M4 and transport fragments never completed a handshake or released a message; decoders never panicked and only accepted canonical encodings; random bytes never panicked |
| FFI boundary | DESIGN ONLY: docs/FFI_DESIGN.md, D016 (proposed); no code, owner approval pending |

The mutation tests are NOT coverage-guided fuzzing and give no coverage claim; cargo-fuzz
needs a nightly toolchain that is not installed. cargo audit/deny check known advisories and
licenses only; they are not a code audit. One test bug was fixed while writing them (the
fragment size could exceed the 256-fragment limit); no implementation change.

### D016 preconditions progress addendum — 2026-11-05

| Check | Actual outcome |
|---|---|
| CI fix: `cargo --offline` removed from phase0.yml (runs on main had been failing since the Phase 1 core landed because the runner could not download crates) | PASS on e1620cf and aef1dc7 |
| `cargo check -p areatm-kagilink-core` for aarch64-linux-android, aarch64-apple-ios, aarch64-apple-ios-sim, aarch64-apple-darwin, run on Windows | PASS (type-check only: no linking, no tests, no Xcode/NDK) |
| `cargo check -p areatm-kagilink-ffi` for aarch64-linux-android, aarch64-apple-ios, aarch64-apple-ios-sim, on Windows | PASS (type-check only) |
| `.github/workflows/core-hardening.yml` on GitHub-hosted runners (aef1dc7): cargo audit + cargo deny; core tests under qemu-aarch64 (29 unit + 38 handshake + 10 mutation); core library build for aarch64-linux-android | PASS. The qemu run uses a normal Linux libc, so it is NOT Android verification; the Android job is compile-only |
| core/kagilink-ffi: fmt, clippy -D warnings, workspace tests (9 FFI tests; total workspace 104 passing) on Windows x86_64 | PASS (run by the subagent and re-run by the lead) |
| kagilink.h syntax check with MSVC `cl /W4 /Zs` | PASS (syntax only; no real C/C++ consumer linked) |
| iOS simulator/device build and tests (macOS session), Android real-device adb run (Android session), coverage-guided fuzzing, FFI tests under qemu and on mobile targets | NOT RUN |

### Coverage-guided fuzzing addendum — 2026-11-05

| Check | Actual outcome |
|---|---|
| `.github/workflows/fuzz.yml` (manual, GitHub-hosted ubuntu, nightly-2026-11-01, cargo-fuzz with libFuzzer + AddressSanitizer), 3 targets: hello_decode, handshake_messages, channel_fragments | First run failed because cargo-fuzz defaulted to a musl target; fixed with `--target x86_64-unknown-linux-gnu` |
| 60 s smoke run | PASS: 18,182,317 / 519,588 / 105,831 executions, no crash |
| 900 s run per target | PASS: hello_decode 224,382,468; handshake_messages 5,384,071; channel_fragments 1,227,815 executions; no crash, no assertion failure, no sanitizer report |
| What this does not show | 15 minutes per target is small. The corpus is not saved between runs, no coverage report was reviewed, and fuzzing finds crashes and violated assertions, not logic or side-channel flaws. FFI functions are not fuzzed yet. The fuzz lockfile resolves some transitive versions independently of the root lockfile |

### Apple target verification addendum — 2026-11-05 (reported by the macOS session)

Results below were produced and reported by the macOS session on the MacBook; the lead session did
not re-run them. Synthetic data only.

| Check | Reported outcome |
|---|---|
| macOS native `cargo test --workspace --locked` (rustc 1.92.0, commit 7383359) | PASS: 104 tests, 0 failures |
| Release build of core and FFI for aarch64-apple-ios and aarch64-apple-ios-sim | PASS (build only) |
| Core test binaries (29 unit, 38 handshake, 10 mutation) installed and run on a physical iPhone 15 Pro, signed with an existing team development certificate through a temporary app; temporary app removed afterwards | PASS (reported) |
| iOS simulator `simctl spawn` run | NOT RUN: deferred by the owner (decision relayed by the macOS session, 2026-11-05) because real Bluetooth and Secure Enclave behaviour can only be checked on a physical device; physical-device runs are the chosen method, signed with the existing team development certificate for synthetic-data debug tests only |
| Not covered on the iPhone | the 7 CPace draft vectors, the 11 model tests, the 9 FFI tests, and any BLE, Secure Enclave or Keychain behaviour |

Signing side effects, unverified: the session ran `xcodebuild -allowProvisioningUpdates` and Xcode
fetched a wildcard development provisioning profile; the session could not tell whether that profile
was newly created or already existed, or whether the device registration changed. No certificate was
issued and no project file was edited. The project's `CODE_SIGNING_ALLOWED = NO` change is still not
started. The owner should check the Apple developer account for any new profile or device entry.

### Android target verification addendum — 2026-11-05 (reported by the Android session)

Produced and reported by the Android session on a physical device; the lead session did not re-run
it. Synthetic data only; owner instructed the run and approved the toolchain install.

| Check | Reported outcome |
|---|---|
| Core test binaries built for aarch64-linux-android (Rust 1.92.0, NDK 29.0.14206865 clang, release, locked) from commit 9027f43 in an isolated worktree | PASS (build) |
| Run on a physical Samsung SM-S921N, Android 16 (SDK 36), arm64-v8a, one directory under /data/local/tmp, removed afterwards; no root, no signing change, no app install | PASS: unit 29/29, handshake 38/38, mutation 10/10, 0 failures, 0 not run |
| What this covers | real aarch64 and Android's bionic libc, including OS randomness through getrandom |
| What it does not cover | StrongBox/Keystore behaviour, the 7 CPace draft vectors, the 9 FFI tests and the model tests on the device, BLE, and the manual device attestation question |

D016 precondition 1 (target build verification) is now met for iOS (device), Android (device) and the
aarch64 qemu job, within the limits above. The iOS simulator was deliberately not used (owner decision).

### Real C consumer test addendum — 2026-11-05

| Check | Actual outcome |
|---|---|
| tests/c/consumer.c (pure C11, only `kagilink.h` + static library): handshake, 4-part fragmented SECRET round trip, approval consumption, wrong code, receiver three-failure lockout, tamper closes channel, NULL and small-buffer handling, wipe | PASS: 71 checks, 0 failures |
| Windows: MSVC `cl /W4 /WX /std:c11`, linked with the release static library | PASS |
| CI (core-hardening.yml `c-consumer`): gcc `-std=c11 -Wall -Wextra -Wpedantic -Werror`, then the same program with AddressSanitizer and UBSan on the C side | PASS (both runs, 71 checks each) |
| What this does not show | Swift, JNI or C++ consumers; the Rust side is not sanitized in the ASan run; FFI functions are not fuzzed; no independent review of the `unsafe` code |

### Windows C++ consumer and FFI fuzzing addendum — 2026-11-05

| Check | Actual outcome |
|---|---|
| `core/kagilink-ffi/include/kagilink.hpp` (C++20 RAII wrapper: wiped `SecretBytes`, owning Receiver/Sender/Channel, grow-and-retry buffers) with `tests/cpp/consumer.cpp`, MSVC `/W4 /WX /std:c++20` | PASS locally and in CI (`cpp-consumer`, windows-latest): 55 checks, 0 failures; includes a 3000-byte message that exercises the held-message and retry paths, wrong code, tamper, and move semantics |
| Fuzz target `ffi_calls` (new): drives the C ABI exports with arbitrary bytes, lengths and output capacities; asserts no export ever returns KL_ERR_PANIC and that fuzz bytes never complete a handshake or authenticate | First run FAILED with LeakSanitizer reports. Cause was the fuzz harness: it did not free the sender and receiver handle shells of its channel section (a host duty per kagilink.h), not the library. Fixed in the harness |
| 300 s rerun of all four targets (GitHub-hosted, nightly-2026-11-01, ASan) | PASS: hello_decode 128,067,057; handshake_messages 2,297,091; channel_fragments 420,589; ffi_calls 388,564 executions; no crash, assertion failure or sanitizer report |
| What this does not show | The `ffi_calls` target is small and shallow compared with the others; the harness uses `unsafe` itself; 5-minute runs without a saved corpus are a smoke level; no independent review of the FFI `unsafe` code |

### D022 TPM probe addendum — 2026-11-05

| Check | Actual outcome |
|---|---|
| `apps/windows/rust/tpm-probe` builds on Windows; non-Windows builds a stub; fmt, clippy -D warnings, workspace tests (104 passing), cargo deny and cargo audit with the new crate | PASS |
| Run as a normal user on the lead PC | TPM 2.0 (AMD firmware TPM, hardware interface); Secure Boot disabled; TBS context, GetCapability, PCR read, ephemeral TPM P-256 key creation and EK public read all worked without administrator rights; EK certificate not available in NV (8-byte value) |
| Key attestation claim and platform claim (quote + boot log) through the Platform Crypto Provider as a normal user, on the lead PC | PASS (claims produced: key attestation 1703 bytes, platform claim 1187 bytes with a 4-byte PCR mask; OS self-check of the key claim OK). The AIK is not EK-activated; blobs not parsed |
| Offline verification on a phone, EK-certificate chain for this AMD firmware TPM, PC bang machines, other vendors' TPMs | NOT RUN / UNKNOWN |

### Windows Rust receiver skeleton addendum — 2026-11-05

| Check | Actual outcome |
|---|---|
| `apps/windows/rust/receiver` (lib + CLI) and `tpm-probe` refactored into lib + bin with `assess()`; fmt, clippy -D warnings, workspace tests, cargo deny, cargo audit | PASS (receiver adds 7 unit tests: D024 decision table, boot state display-only, gate needs acknowledgment, blocked PC cannot be acknowledged, loopback in both modes) |
| `kagilink-receiver check` on the lead PC as a normal user | TPM present, attestation claim produced, decision PROCEED; Secure Boot disabled is shown, not enforced |
| `kagilink-receiver loopback --accept` on the lead PC | OK: core handshake with a simulated phone, one approved SECRET transfer of 36 synthetic bytes, plaintext not shown |
| Cancel path (answer other than YES) | exit code 2, no transfer |
| Block and RAM-only decisions on a real PC | NOT RUN (covered only by unit tests; the lead PC has a working TPM) |
| BLE, text entry into a target window, phone-side verification, vault, installer/portable packaging | NOT DONE |

### Rust BLE central and own-window input probes addendum — 2026-11-05

| Check | Actual outcome |
|---|---|
| `receiver` BLE module (WinRT through the `windows` crate, safe Rust; implements the Phase 0 fixed PING/PONG contract; scan, exactly-one-sender rule, write with response, exact reply check; prints no addresses or names) | Builds, clippy clean. Run on the lead PC (Realtek BLE adapter, central role supported): an 8 s scan with no phone advertising gave NoSender; on 2026-11-06 the owner ran it with a phone sender started: 15 s scan, RESULT ExchangeOk (fixed request written with response, exact fixed reply read). The sender was an iOS phone running the Phase 0 app (exact model not recorded); this verifies only the Phase 0 fixed-string exchange from Rust, not fragmentation, MTU, notifications, disconnect behaviour, cryptography or the production protocol |
| `input-probe` (own-window synthetic Unicode input, opt-in `--send-to-own-window`, sends only if its own window is foreground, reads the text back from its own edit box) | Builds, clippy clean. First run without a user click: INCONCLUSIVE, nothing sent (Windows does not let a background program take the foreground; no workaround is attempted). Changed to wait up to 20 s for the user's click. Run by the owner with the click (2026-11-06): 56 of 56 events queued, edit box held all 28 characters, RESULT DELIVERED to our own window (normal integrity, same desktop) |
| Input into other windows, elevated windows, CredUI, secure desktop, Remote Desktop | NOT TESTED and out of the allowed scope of these probes |

Desk research on input constraints (public search 2026-11-05; community reports are not official
guidance): SendInput is subject to UIPI and is silently dropped when the target runs at a higher
integrity level, and its return value does not say why; Unicode packets (KEYEVENTF_UNICODE, VK_PACKET)
are reported by community posts to be lost in some Remote Desktop and console scenarios while scan-code
input sometimes works; no Microsoft statement about SendInput into an mstsc session was found. Together
this means RDP and credential-dialog input must stay unsupported (AGENTS.md) until evidence from a real
Remote Desktop target exists.

### BLE/GATT framing draft addendum — 2026-11-06

| Check | Actual outcome |
|---|---|
| docs/BLE_GATT_SPEC.md (D025) written | DRAFT, not reviewed by the platform sessions, not tested on devices |
| `ble_frame` codec: 10 unit tests (all message lengths 1..512 over chunk sizes 6..512, back-to-back messages, duplicate/gap/reorder, missing/double FIRST, reserved bits, unknown type, zero/oversize length, LAST flag/length disagreement, structural mutations); fmt, clippy -D warnings, cargo deny | PASS |
| New fuzz target `ble_frames` added to fuzz.yml | Compiles; NOT RUN yet (needs a manual dispatch of the fuzz workflow) |
| Real-device checks (MTU 23 and negotiated, Indicate delivery/order, disconnects, two advertisers, wrong code, 60 s expiry, Windows GattSession) | NOT RUN |

### D026 addendum (crowded places, Notify fallback, failure counting) — 2026-11-06

| Check | Actual outcome |
|---|---|
| handshake.rs: only attempts that already sent M3 count (bad M4 tag, second M2, abandonment after M3); malformed M2 and M4-before-M3 are free | Implemented. Core: 40 unit + 41 handshake tests; one old test replaced by four tests of the new rule; FFI test updated; C consumer 75 checks and C++ consumer 55 checks pass locally on MSVC; workspace total 125 tests, 0 failed; fmt, clippy -D warnings, cargo deny clean |
| ble_frame ACK chunk helpers for the Notify fallback (ack_chunk, parse_ack; data path rejects ACK flags) | Implemented with one added test |
| BLE_GATT_SPEC.md updated: scan-response tag `KL-XXXX`, signal-sorted list, user selection, Notify + ACK, no auto-connect | DRAFT, not reviewed by platform sessions, not run on devices |
| Scan-response local name carrying the tag on iOS/Android/Windows; Notify stop-and-wait; look-alike tag behaviour | NOT RUN |

### Receiver tag list and scan addendum — 2026-11-06

| Check | Actual outcome |
|---|---|
| `selection.rs` (pure logic: tag validation, name/service-data agreement, duplicate-tag blocking, signal bars, stable sort, no default choice) written by a subagent and re-checked by the lead | 12 unit tests; workspace total 142 passing, 0 failed; fmt, clippy -D warnings and cargo deny clean; no `unsafe` in `ble.rs` or `selection.rs` |
| `ble.rs` `scan_advertisements` (active scan, KL-V1 service filter, local name and 128-bit service-data parsing) and `ble-list [--seconds N] [--choose N]` CLI | Builds; 5 parser unit tests pass. The CLI prints only the tag, a signal bar and a reason, never an address, device name, grouping key or raw signal value (checked by reading the output code and by a grep) |
| Real run on the lead PC | `ble-list --seconds 6`: "No KagiLink senders found" (no phone advertises the new KL-V1 service yet). The path from a real advertisement to a printed row has NOT run on hardware |
| Connect step after selection, indication/notify subscription, M1..M4 over the new GATT service, the phones' advertising of the tag | NOT IMPLEMENTED |

## Next gates and limitations

1. Run the built probe on a normal medium-integrity desktop and record results;
   perform opt-in own-window synthetic input only in a suitable test environment.
2. Real foreground mobile peripheral + Windows Central GATT exchange including
   MTU/disconnect/permission/radio-off tests; identify adapter/OS configurations.
3. Verify input safety separately for ordinary windows, CredUI and mstsc. Current
   official restrictions prevent claiming supported non-elevated credential input.
4. Resolve audited/reviewed CPace suite (D013) and memory lifecycle, then freeze vectors.
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
| Apple unsigned compile (macOS host, Xcode 27.1, iOS SDK 27.1) | PASS, `build-probes.sh` (iphoneos27.1 and `--simulator` iphonesimulator27.1) built iOS and macOS Phase 0 targets with no compiler warnings, unsigned; fixed `accessibilityContrast` to `colorSchemeContrast`. Signed sandbox, runtime and device behavior still NOT RUN |
| macOS Phase 0 probe run (local ad-hoc signed copy with sandbox+Bluetooth entitlements) | User-reported success on 2026-11-04 (Receiver scan with 10 s timeout observed in logs). Peer device/OS, exact on-screen status and MTU/disconnect behavior not recorded; not independently verified by the agent. Developer-ID signing/notarization NOT RUN |
| iPhone 15 Pro install/launch (AreaTM team development signing) | PASS, signed Debug build installed and launched; screenshot shows the Phase 0 screen (public-string notice, test-ended state, Secure Enclave "reported, unverified"). A Windows run at Medium RID reported `SYNTHETIC_GATT_WRITE_READ_PASS` at about the same time, but peer identity is unverified (public UUID). Notifications, MTU, fragmentation, accessibility, Reduce Transparency/Increase Contrast and Duo layout NOT RUN |
| Liquid Glass appearance/accessibility/Duo folding | NOT RUN; source checks are not visual/device evidence |
| Actual cross-OS GATT exchange/notifications/MTU/disconnect | NOT RUN |
| New GitHub Actions Windows/static/shell-fixture checks | Prepared locally; not pushed or executed remotely |

The diagnostic can be spoofed/replayed and cannot prove proximity or device
integrity. Apple peripheral disconnect observation is incomplete; abandoned
public state expires. Device access/consent, cleanup timing and fixed reply delivery
still require real hardware. The receiving Windows executable remains a research
CLI at that checkpoint, not a completed receiver product UI. A subsequent diagnostic
GUI shell is recorded below. No account/certificate/provisioning,
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

The same output was reported a second time, confirmed by the user as coming from a
Samsung SM-S921N running the same debug APK (installed over wireless adb; foreground
launch with no crash-buffer entries). The screenshot's last line was cut off, so
`NOTIFICATION_MTU_FRAGMENTATION_NOT_TESTED` is only partly visible there, and the
device-to-line mapping rests on the user's statement, not captured logs.

This is the first recorded Android-peripheral to Windows-Central exchange. It
supersedes the earlier `NO_DIAGNOSTIC_SENDER` result only for that single fixed-value
round trip. It does not prove proximity, authentication, StrongBox/key behavior,
MTU/notification handling, disconnect/permission/radio-off behavior, other adapters
or OS versions, Android lint, TalkBack/large-font/visual layout, or any production
transfer. The Phase 0 gate remains open.

## Android StrongBox verification module — 2026-11-04

The StrongBox-mandatory policy is unchanged (StrongBox only; no TEE or software
fallback; fail closed). This adds an independent Phase 0 diagnostic module
`apps/android/strongbox` (`in.minori.key.phase0.strongbox`, no INTERNET permission, no
third-party dependencies, own lockfile). It is separate from the BLE diagnostic app and
is not approval for any password storage or transfer. It uses only random synthetic
data, requests every key with `setIsStrongBoxBacked(true)` and never retries as TEE or
software, and deletes every temporary key (`kagilink_phase0_sb_*`; a final check confirms
none remain). Only non-sensitive verdict lines are logged (tag `KagiStrongBox`).

Verdict vocabulary: SUPPORTED, UNSUPPORTED (StrongBox absent/unavailable), UNVERIFIABLE
(could not be evaluated), FAILED (key created but not StrongBox, or use failed).

Build: `:strongbox:assembleDebug` PASS; `:strongbox:lintDebug` PASS with 12 warnings
(manifest icon/allowBackup, SetTextI18n in the diagnostic UI) and no errors. The new
module has its own `gradle.lockfile` (written for this module only; existing locks
unchanged; it does not include the connected-test UTP configurations).

### Galaxy S24 (SM-S921N, Android 16 / API 36)

| Check | Verdict | Detail |
|---|---|---|
| Feature flag `FEATURE_STRONGBOX_KEYSTORE` | SUPPORTED | informational only |
| ECDSA P-256 signing key | SUPPORTED | `KeyInfo` level StrongBox; sign/verify ok; tampered message rejected |
| AES-256-GCM key | SUPPORTED | `KeyInfo` level StrongBox, 256-bit; round trip ok; tampered ciphertext rejected |
| Key attestation (EC) | SUPPORTED, offline limits below | challenge matched; chain of 4 with valid signatures and dates; attestation and KeyMint levels StrongBox; attestation version 300; root self-signed, SPKI SHA-256 `feb2ea7551ee316ed4bb443c8293b884dbfdea40b603ee3e4f4a897e4580fbae` |
| Key attestation (AES) | UNVERIFIABLE | platform returns no chain for a symmetric key |
| Temporary keys deleted | SUPPORTED | no `kagilink_phase0_sb_*` aliases remain |
| Overall (EC + AES + attestation) | SUPPORTED | non-biometric scope only |
| Biometric key creation (`BIOMETRIC_STRONG`, per-use, validity 0, invalidated by enrollment) | SUPPORTED | `KeyInfo` level StrongBox |
| Key use before authentication | SUPPORTED | refused |
| Key use after fresh `BIOMETRIC_STRONG` authentication | SUPPORTED | succeeded once; an immediate second use was refused and needs a new prompt |
| Temporary keys deleted (after biometric run) | SUPPORTED | no `kagilink_phase0_sb_*` aliases remain |

The biometric rows come from one run with the user's own enrolled fingerprint (user-
performed prompt, 2026-11-04 15:15 KST). Only the three result lines were recorded;
no biometric data is accessible to the app.

Offline attestation limits: this proves chain consistency only (signatures, validity,
challenge, levels). Revocation/status freshness (Google's attestation status list)
cannot be fully checked offline, and no network is used, so a revoked or compromised
intermediate/leaf would not be detected. No trusted root is embedded or pinned, so the
chain is NOT shown to terminate in a genuine Google root; the root SPKI hash is
recorded only for a future pinning decision. Attestation does not prove the device is
untampered at use time.

### Redmi Pad Pro (2405CRPFDL)

No StrongBox run. `pm list features` shows no `android.hardware.strongbox_keystore`
(only `hardware_keystore=100`), so it is treated as UNSUPPORTED for sensitive transfer
on that evidence alone; the module was not installed or run on it. Its SoC (Snapdragon
7s Gen 2) is user-confirmed.

Not verified: biometric behavior beyond the single run above (repeat runs, a failed
or cancelled prompt, fallback to device credential being disabled), other Galaxy/Android builds, behavior
after biometric enrollment changes, key use under load or after reboot, and any
production key lifecycle.

## Windows diagnostic GUI — 2026-11-04

Native Win32 diagnostic shell is built as `target/phase0/kagilink.exe`;
the existing CLI remains `kagilink-phase0-probe.exe`. The window has explicit
inventory/BLE-test actions, stop, Korean result text and read-only public diagnostic
output. It starts no probe on launch and has no secret input or injection control.
The shell supervises only the two fixed sibling CLI commands with bounded output,
nonblocking polling, job cleanup and a 70-second outer deadline. The child's
integrity guard and 60-second BLE lifetime are unchanged.

| GUI check | Actual outcome |
|---|---|
| Native GUI/CLI build | PASS, MSVC C++20 /W4 /WX; GUI PE subsystem 2 and KagiLink/AreaTM/kagilink.exe metadata checked |
| Existing CLI regression | PASS, 6 noninteractive checks after GUI build integration |
| GUI subprocess lifecycle fixtures | PASS, 5 synthetic groups: invalid action rejection, fixed actions/final output, output cap, contained-child cancellation and explicit restart; no BLE/UI/input in these fixtures |
| Native source/project consistency | PASS, 7 static checks; not native device evidence |
| GUI launch and initial window | User manually opened the GUI after automated launch hit GetCursorPos access denied (0x80070005); ready window then observed with Computer Use |
| Inventory button | PASS in visible GUI: BLE LE/Central reported, TPM provider caveat shown; key use/TPM 2.0 still unverified |
| BLE start, busy state and stop | PASS: start buttons disabled while running, stop enabled; stop produced cancelled status and restored start controls |
| Explicit restart and no-sender result | PASS: restarted BLE search completed its 10-second scan and displayed no diagnostic sender; no actual peer exchange |
| Details toggle and captured output | PASS: read-only bounded public diagnostic codes displayed, including Medium RID 8192 |
| GUI screenshot/initial visual inspection | PASS in the observed 926 x 813 capture, with no clipped/overlapping text; local ignored captures recorded below |
| GUI close/destructor cleanup, missing sibling, 70-second timeout | NOT RUN as end-to-end GUI cases; fixture cancellation does not establish every lifecycle path |
| Keyboard, DPI/scaling, high contrast and screenreader matrix | NOT RUN |
| Real BLE peer/credential input/RDP | NOT RUN; unsupported secret input remains disabled |

Observed local screenshots: `target/phase0/screenshots/adapter-result.jpg`,
`target/phase0/screenshots/ble-running.jpg` and
`target/phase0/screenshots/ble-result-details.jpg`. They remain ignored local
artifacts, not public-source or CI uploads. The GUI was left open after inspection.
These captures establish only the displayed diagnostic states and this window
size; they do not prove cross-device exchange, authentication or full accessibility.

### Windows Glass restyle (visual only)

The GUI was restyled to match the Android/Apple Glass design (teal gradient backdrop,
frosted dark glass panels with rim, pill controls; GDI+, `gdiplus.lib`/`uxtheme.lib`
added to the GUI link line; high-contrast fallback to system colors). No behaviour,
security policy or child-process logic changed.

| Check | Actual outcome |
|---|---|
| GUI build | PASS, MSVC C++20 /W4 /WX |
| Runner lifecycle fixtures | PASS, 5 synthetic groups |
| Visual capture of ready, adapter-result and details-expanded states (940 x 820) | PASS by inspection, no clipped/overlapping text; captured via PrintWindow into a scratch folder, not kept in the repository |
| Hover state, high-contrast mode, DPI scaling, screen-reader, keyboard matrix | NOT RUN (hover state not implemented) |
| BLE connection test button in the restyled GUI | NOT RUN |

This is Phase 0 diagnostic UI. Android/Apple SDK compilation, real-device exchange,
hardware key operations, production security-core selection and native product UI
remain incomplete. No release/signing/store/server action is authorized by this
diagnostic implementation.

### D021 binding spikes: Swift (macOS/iOS) and Kotlin/JNI (Android), reported 2026-11-06

Both spikes call the shared C ABI (`kagilink.h`) from the native language and port the
`tests/c/consumer.c` scenarios. Results are as REPORTED by the platform sessions; the lead
session has checked the PR file lists and CI state but has not rerun the device tests.
Neither PR is merged (merge is the owner's decision).

| Spike | PR | Reported result | Notes / limits |
|---|---|---|---|
| Android Kotlin/JNI (`apps/android/ffi-spike`) | #5 | Galaxy S24 (SM-S921N, Android 16): 78 checks, 0 failures, exit 0; library exports only 18 JNI symbols | Run via `app_process` from /data/local/tmp, no app install. NULL-pointer-with-nonzero-length cases cannot occur from Kotlin (array becomes (NULL,0)); replaced by NULL output handle, zero handle and size-query checks, so 78 is not 1:1 with the 75 C checks. `panic=abort` not applied (catch_unwind fallback only). StrongBox, Keystore, BLE NOT verified. |
| Apple Swift (`apps/apple/KagiLinkCore`) | #6 | macOS `swift test`: 8 scenarios pass; iPhone 15 Pro: same 8 scenarios, 50 checks pass; 7 existing contract tests pass | On device it ran in a scratch app shell with an XCTest shim, not real XCTest (package tests need a host app on device). iOS simulator not run (owner decision). Built with `panic=abort`, so the KL_ERR_PANIC path is unreachable and untested. `kl_status_t` imports as UInt32 in Swift; the wrapper converts to Int32. Signing: existing AreaTM team, existing Apple Development certificate and existing wildcard profile; no new certificate, device registration or upload. |

Common finding: no ABI or header problem found by either binding. Scenarios covered on both: handshake,
fragmented SECRET round trip, approval consumption, wrong code, D026 failure counting (Swift: five
pre-M3 errors cost no budget, three post-M3 failures then TOO_MANY_FAILURES), tamper closes the
channel, small buffers. Still NOT verified: BLE transport, Keystore/Secure Enclave key use,
`panic=abort` on Android, real XCTest on device, review of the FFI `unsafe`.

### Windows receiver: session driver, fake phone and GATT connect code (2026-11-06)

| Item | State |
|---|---|
| `session.rs`: transport-agnostic `GattLink` trait and `run_attempt` (chunking, reassembly, Notify ACK, 10 s first-chunk and 5 s message timeouts, M1-M4, SECRET, RESULT) | implemented; fake-phone tests PASS |
| `fake_sender.rs`: in-process KL-V1 phone using the real core sender; behaviours honest, wrong code, garbage M2, tampered M4, tampered SECRET, silent, drop after M3, duplicate chunk, skipped chunk, oversize length; Notify mode enforces stop-and-wait ACK | PASS (7 integration tests: chunk sizes 20-244 in Indicate and Notify, D026 counting through the driver, budget exhaustion, consumed code) |
| `selection.rs`: opaque `ConnectTarget` for the chosen row (Debug redacted, same refusals as `choose`) | PASS |
| `ble_link.rs`: WinRT connect, uncached KL-V1 discovery, Indicate preferred else Notify, subscribe before first write, write-with-response, disconnect detection | COMPILES ONLY. NOT RUN against any peripheral: no phone advertises the production service yet. |
| CLI `fake-session [--behaviour B] [--chunk N] [--notify]` | runs (honest OK; wrong code and tampered M4 fail closed) |
| CLI `ble-receive`: risk notice, code shown first, active scan, user picks a row, connect, one attempt | COMPILES ONLY, NOT RUN |

Workspace: 150 tests pass, clippy `-D warnings` clean. The fake phone proves driver and framing logic, not real BLE behaviour (MTU, indication delivery, scan-response merging all remain unmeasured). Real-secret transfer stays disabled (D012).

### First end-to-end BLE exchange: iPhone 15 Pro to Windows receiver (reported 2026-11-06)

Reported by the macOS session (PR #8, not yet merged; the lead session has not repeated the run):
iPhone 15 Pro (KL-V1 peripheral spike, foreground) to the Windows Rust receiver (`ble-receive`
driver over `ble_link`), synthetic data, one run: M1 153 B, M2, M3 65 B, M4, SECRET fragment, RESULT
(11 B) all exchanged; Windows printed "Transfer OK: 33 bytes received in 1 fragments"; about 0.6 s from
subscribe to RESULT.

| Measurement | Reported value |
|---|---|
| Local name `KL-XXXX` visible in Windows `ble-list` (active scan, app in foreground) | yes, 3 observations (2 plus the exchange run) |
| iOS `maximumUpdateValueLength` | 512 (ATT MTU 515); chunk size capped at 244 by the spec |
| Indicate send queue | one `updateValue` returned false, `peripheralManagerIsReady` fired once, queue drained (retry path works) |
| iOS peripheral API | reports subscription only (no connect event) and cannot disconnect the central |

Consequences recorded: advertising stops at subscribe on iOS; on error the iOS sender answers with an
error, removes the service and stops advertising, and the Windows central must drop the link (the Rust
driver already disconnects on every error). Not verified: background advertising, MTU 23 small chunks,
Notify+ACK (iOS uses Indicate), two advertisers or a copied tag, error paths on the real radio,
repeat runs. Code delivery to the phone was the practical bottleneck when the tester could not see the
laptop screen.
Open proposal accepted by the lead: expose the BLE chunk codec through the C ABI so platforms stop
re-implementing it (the Swift spike re-implemented spec section 5 in `Framing.swift`).

### BLE chunk codec through the C ABI, ABI v2 (D027, 2026-11-06)

| Item | State |
|---|---|
| New exports: `kl_frame_chunk`, `kl_reassembler_new/push/take/is_dead/free`, `kl_ack_chunk`, `kl_parse_ack`; header, C++ RAII wrapper (`frame_chunk`, `Reassembler`, `ack_chunk`, `parse_ack`) and `KL_ABI_VERSION` 2 updated; Android spike `EXPECTED_ABI` set to 2 | implemented |
| `cargo fmt --all`, `cargo clippy --workspace --all-targets --offline -- -D warnings` | clean |
| `cargo test --workspace --offline` and `--release` | PASS (FFI integration tests 15, was 8: round trip for chunk sizes 20..244 plus 6 and 512 over 13 message lengths, duplicate/gap/reorder, reserved and ACK bits, oversize declared length and oversize chunk, empty and header-only chunks, unknown mtype, missing LAST, dead-for-good, NULL and tiny-buffer and size-query cases, held-message BUFFER behaviour, ACK round trip, constants equal to core) |
| `tests/cpp/run-cpp-consumer.ps1` (MSVC /W4 /WX, C++20) | PASS: 653 checks, 0 failures |
| `tests/c/consumer.c` | compiled and run locally with MSVC `/W4 /WX /std:c11` only (extra check): 8944 checks, 0 failures. The project gcc `-Wall -Wextra -Wpedantic -Werror` build and the gcc AddressSanitizer/UBSan run, and `tools/check-ffi-exports.sh` (needs `nm`), did NOT run locally; they run in CI only. The header-vs-`lib.rs` export lists were compared by regex locally: identical, 26 functions |
| Fuzz target `ffi_calls` extended with the codec calls | type-checks (`cargo check` on the fuzz crate); NOT fuzzed (needs the pinned nightly and cargo-fuzz in CI) |
| Swift binding, Android JNI shim | NOT updated or rebuilt: they do not wrap the new functions yet; the Swift package reads the header ABI constant, the Android spike constant was bumped, neither re-run on a device |
| Real BLE use of the codec API | NOT tested; independent review of the new `unsafe` still outstanding |

### Independent review of the FFI crate and bindings (2026-11-06)

Verdict: acceptable for synthetic-data spikes only. NOT run: Miri or ASan over the FFI tests, a
concurrent free-versus-call test, any real-device test. ABI version stays 2 (no signature change).

| # | Finding | State |
|---|---|---|
| 1 | Free races with another call / double free are undefined behaviour (handles are raw pointers) | doc-only: header, `Native.kt` and `FFI_DESIGN.md` now state exclusive-free and host ownership patterns; library-side handle table with generation tokens REQUIRED before real secrets, not implemented |
| 2 | A completed held plaintext could be returned from a closed or expired channel | fixed: `kl_channel_open_fragment` returns `KL_ERR_CLOSED` and drops the held message; `held` is cleared after every close (seal, expiry, counter exhaustion, close). Rust regression test |
| 3 | `kl_channel_approve_one_transfer` / `kl_channel_set_ready_to_receive` returned `KL_OK` on a closed channel | fixed: return `KL_ERR_CLOSED`. Rust test, C and C++ consumer checks |
| 4 | `kl_channel_is_closed` ignored the deadline | fixed: new tiny core method `SecureChannel::poll_closed` (evaluates the deadline, closes on expiry), core unit test. FFI cannot test expiry (system clock) |
| 5 | Poisoned mutex skipped zeroization in close | fixed: `kl_channel_close` recovers a poisoned mutex (`PoisonError::into_inner`) |
| 6 | `kl_reassembler_free` used a blocking lock and skipped wipe on poison | fixed: `get_mut` on the owned Box, poison recovered; `kl_channel_free` likewise wipes via `get_mut` |
| 7 | JNI `out_end` wrote the length slot on every status | fixed: length written only on `KL_OK` / `KL_ERR_BUFFER`, else 0 (not compiled here, see below) |
| 8 | JNI did not validate `kind` / `maxFrag`; `SetLongArrayRegion` failure leaked a handle | fixed: `kind` 1..3 and `maxFrag` 1..256 else `KL_ERR_LIMITS`; `handle_out` checks `ExceptionCheck` and frees the handle on failure (not compiled here, see below) |
| 9 | Header text did not match D026 failure counting | fixed (docs) |
| 10 | In and out buffers may not overlap (undocumented) | doc-only: stated in the header and `FFI_DESIGN.md` |
| 11 | Free semantics wording suggested a concurrent call during free was safe | fixed (docs): see #1 |

The Android JNI shim and Kotlin binding changes were not built or run in this environment (no NDK or
JVM harness here); they are small and reviewed by reading only.

### Retest with the C ABI codec (ABI v2): iPhone 15 Pro to Windows receiver, 2026-11-06

Owner ran the Windows receiver (`ble-receive`, pick row 1) against the rebuilt iPhone app. PC output (owner-reported):
"Transfer OK: 33 bytes received in 1 fragments" (synthetic data). Phone side reported by the macOS session: build is the ABI v2
version on main 5726482 plus the PR #18 branch, the spike uses the Rust codec through `Codec`/`FrameReassembler` (not the Swift
reference framing); subscribe gave `maximumUpdateValueLength` 512 (ATT MTU 515), chunk 244; M1 153 B, M3 65 B, one 40 B channel
message in and an 11 B RESULT confirmed; subscribe to RESULT about 0.58 s, clean end about 1.0 s later; one `updateValue` returned
false and `peripheralManagerIsReady` fired once; zero protocol or codec error logs. Same sequence and sizes as the earlier run with
the Swift codec. Signing reused the existing AreaTM development certificate and wildcard profile (no new issuance, registration or upload).
Not verified: background advertising, MTU 23 small chunks, two advertisers or a copied tag, error paths on the real radio, repeat runs.

Android retest with the same ABI v2 codec (Galaxy S24, build from main 5726482), reported by the Android session from the PHONE
log only; the Windows-side output of these two runs was not shown to the lead session, so the PC side is UNCONFIRMED here.
Indicate: RESULT 11 B received, MTU 517, 3 chunks of 244 B, 3 send callbacks and 0 failures (mean 73.1 ms, max 129.1 ms),
0 ACKs, advertise to connect 5088 ms, connect to CCCD 2054 ms. Notify+app ACK: RESULT 11 B received, MTU 517, 3 chunks of 244 B,
3 callbacks and 0 failures (mean 5.3 ms, max 7.3 ms), 3 app ACKs, advertise to connect 7492 ms, connect to CCCD 2184 ms. No errors;
MTU history [517] in both modes; numbers match the pre-codec build within noise. Not measured: RPA rotation, timeouts, wrong code,
tampering, disconnect paths. One run per mode.

Windows side of the two Android runs (owner-run `ble-receive`, pick row 1, outputs owner-pasted): both printed
"Transfer OK: 30 bytes received in 1 fragments" (tags in the list showed signal Medium for the first run and Weak for the second).
The mapping of each output to Indicate or Notify+ACK was not stated. With this, the Android retest is confirmed on both ends:
PC output and phone log agree (30 B received on the PC, 11 B RESULT on the phone). Still one run per mode.

### Fault injection on the real radio: iPhone 15 Pro to Windows receiver, 2026-11-06 (reported)

Test-only fault modes (PR #21, compiled only with `FAULT_INJECTION=1`) against the Windows `ble-receive` before this change; owner supplied
the PC output, the macOS session compared it with the phone log. Results: garbage M2 -> `Core(WrongState)` (a wrong-type message fails
before M3, so it is free under D026); M4 with one bit flipped -> `Core(ConfirmationFailed)`; drop after M3 -> `Link(Disconnected)`;
duplicate, skipped (twice) and reordered chunk -> `Core(Framing)` (4 runs); tampered SECRET fragment -> `Core(Authentication)`, no
"Transfer OK"; M2 delayed 6 s -> TRANSFER OK. The last one is not a defect: the receiver waits up to 10 s for the first chunk of a
handshake message (`FIRST_CHUNK_WAIT`, the phone runs CPace) and the 5 s limit applies from the FIRST chunk; the expectation in the test
request was wrong. The spec now says so (section 6); a delay of more than 10 s is the timeout case and is NOT yet tested on a radio.
The PC took about 3.1-3.3 s to drop the link after an error (the phone saw "central unsubscribed" each time). The failure budget was
NOT visible in the PC output, so the expected counts (0 for garbage M2, 1 for tampered M4 and dropped-after-M3) were not verified on the
radio. Change in this PR: `ble-receive` prints "Failure budget used: n/3" after every attempt (a new core getter,
`failures_including_open`, includes an attempt abandoned after M3 that is counted when the next attempt begins), and after a failed attempt
it offers to retry with the SAME code, so the budget can be checked across attempts. The GATT session is now released on disconnect
(SetMaintainConnection(false) + Close), which may shorten the 3 s drop; UNVERIFIED on a radio. Android fault modes (PR #22) are installed and
not yet run. Android's fault code is runtime-gated by the debuggable flag (present in release builds, disabled there); it must become
compile-time-gated before any product build.

### FFI handle registry (review finding 1 closed), 2026-11-06

Implemented in `core/kagilink-ffi` (no exported signature change, ABI stays 2): handles are now opaque slot+generation tokens
resolved through a global registry (`Mutex<slab of Arc>`), never dereferenced. Every export clones the `Arc` and releases the registry
lock before touching the object; every free removes the slot and bumps the generation, then erases the secrets under the object's own
mutex (waiting for an in-flight call) before dropping the last `Arc`. Stale, doubled, forged, NULL and wrong-type handles return
`KL_ERR_NULL` (frees are no-ops). `try_lock`/`KL_ERR_BUSY` for concurrent non-free calls is unchanged. Lock order: the registry lock is a
leaf (see `docs/FFI_DESIGN.md`, "Handle registry"). Header, `Native.kt`, `NativeHandle.kt` and FFI_DESIGN text updated; the
"REQUIRED before real secrets" note is replaced by the implemented design and its remaining limits (generation wrap, live-handle cap,
trusted caller buffers, process-global registry).

| Check | Result |
|---|---|
| cargo fmt, clippy `--workspace --all-targets -D warnings` | clean |
| `cargo test --workspace` debug and `--release` | 166 passed, 0 failed each (FFI test file: 23 tests) |
| New Rust tests: double free no-op and use-after-free errors for all four handle kinds; wrong-type token rejected and not freed; forged/garbage tokens (0, 1, usize::MAX, stack and heap addresses, generation-bit flips) rejected by every export; token reuse after free does not resolve to the old object; 200-iteration thread hammer on one channel token (seal/approve/is_closed/close with racing double free) and 200-iteration reassembler hammer | PASS, repeated 6x release and 3x debug without a failure |
| MSVC `cl /W4 /WX` C11 `tests/c/consumer.c` against the release static library | PASS: 8946 checks, 0 failures |
| `tests/cpp/run-cpp-consumer.ps1` | PASS: 653 checks, 0 failures |
| `tools/check-ffi-exports.sh` logic | not run here (needs Linux `nm -D`); the header still mentions exactly the 26 exported `kl_xxx(` names and lib.rs has 26 `#[no_mangle]` |

NOT run: Miri, ASan/TSan or any sanitizer (not available locally), loom, a 32-bit target build (the 32-bit bit layout, 12 index and
20 generation bits, is written but untested), or any host binding rebuild (Kotlin, Swift, JNI sources only had comments changed).
Two parallel-test bugs were found and fixed in the new tests themselves (an index-bit flip of a live token can legitimately alias another
live object because the tests share the process-global registry); the library behaved correctly. Zeroization on a free that overlaps an
in-flight call is by design (free waits, then erases) but has no dedicated deterministic test; the existing zeroize tests still pass.

### Fault injection on the real radio: Galaxy S24 to Windows receiver, 2026-11-06 (reported)

Android session report; the owner supplied the PC outputs and the session matched them with the phone log (KLV1) by order and time. There is no
direct key between the two logs, so the pairing is an inference. Results (both GATT modes where noted): garbage M2 -> `Core(WrongState)`, budget 0/3
(expected); drop after M3 -> `Timeout`, budget 1/3 then 2/3 (expected count); skipped chunk and reordered chunk -> `Core(Framing)`, 0/3 (expected;
the duplicate-chunk run is valid for Indicate only, the Notify run was spoiled by code expiry); M1 left unanswered -> `Timeout`, 0/3 (expected);
M2 after 6 s -> Transfer OK 30 B in both modes (expected: a delay under the 10 s first-chunk wait is accepted); M2 after 11 s -> `Timeout`, 0/3
(expected); tampered SECRET fragment -> `Core(Authentication)`, no delivery, budget stays 1/3 (Notify run only; the Indicate run was spoiled by a
mistyped code). NOT run: tampered M4 (b) on Android, and the same-code repetition to 3/3 with code destruction (the drop-after-M3 series reached
2/3 before the code expired).
Defect found by the Android session and fixed here: when the code expired while an attempt abandoned after M3 was still open, the displayed budget
fell from 2/3 to 1/3. Cause: `kill()` dropped the open attempt without counting it. Fix: `kill()` counts an attempt that is open after M3 (D026); two
core regression tests. A dropped link after M3 surfaced as `Timeout` (about 10 s wait) rather than `Link(Disconnected)` on Windows for Android;
not investigated further.

### Real-radio fault injection series, 2026-11-06 (Galaxy S24 and iPhone 15 Pro to a Windows receiver)

All results below are as REPORTED by the platform sessions and the test-station session from owner-run or session-driven runs; the lead session did not run the radio tests itself. Data is synthetic. Pairing of PC output and phone logs is by time (UTC, about 1-2 s clock skew).

**Android (debug APK v12, automated by `tools/ble-fault-harness.ps1` over adb on the test-station PC; signal "Weak" in every run):**
15 of 15 scenarios PASS in the final full run: none (Indicate and Notify) Transfer OK; garbage M2 `Core(WrongState)` 0/3; tampered M4 `Core(ConfirmationFailed)` 1/3 (both modes); drop after M3 `Link(Disconnected)` 1/3; duplicate, skipped and reordered chunk `Core(Framing)` 0/3; no answer to M1 `Timeout` 0/3; M2 delayed 6 s Transfer OK (by design, the receiver waits up to 10 s for a first chunk); M2 delayed 11 s `Timeout` 0/3; tampered SECRET `Core(Authentication)` 0/3 (handshake already succeeded, code consumed, budget not used); same code with tampered M4 three times: 1/3, 2/3, 3/3 and "The code is no longer active". Phone-side outcomes agree.

**iPhone (fault build, manual with session-driven PC input):**
tampered M4 with the same code `Core(ConfirmationFailed)` 1/3, then 2/3 (the first try of this series stopped there: operator latency within the 60 s code lifetime); re-run later the same day: 1/3, 2/3, 3/3 within 48 s and again within 46 s, and after the receiver fix (PR #53) `The code is no longer active` right after 3/3 (see the re-run addendum below); M2 delayed 11 s `Timeout` 0/3; drop after M3 `Link(Disconnected)` 1/3; skipped chunk `Core(Framing)` 0/3; tampered SECRET `Core(Authentication)` 0/3. Phone logs (macOS session) agree: the failure budget counted exactly the attempts where the phone had received M3 (D026).

**Defects and wrong expectations found by these runs (all fixed on main unless noted):**
1. Receiver pick prompt failed under a harness: Windows PowerShell 5.1 prefixed stdin with a UTF-8 BOM, which Rust `trim()` keeps. Fixed (`clean_input`, PR #42).
2. A list row without a tag (scan response arriving after the first packet) was shown and refused. The receiver now listens up to 3 x 3 s more (PR #38).
3. GATT discovery failures were reported as `NotKagiLink`; now the real status (`Gatt(Unreachable)`) with 3 discovery retries (PR #44), one full connect retry (#49), and the GATT service and link are released earlier (#50, effect not proven alone).
4. Displayed failure budget fell from 2/3 to 1/3 on expiry with an after-M3 attempt open (core `kill()`, PR #26).
5. repeat-b3 second attempt failed with `Gatt(Unreachable)`: the Android app's cleanup (server closed 1.5 s after cancel with the old link still up) was the cause side; fixed on the phone by closing every connection and waiting for disconnect callbacks up to 3 s before closing the server (PR #48, #51). The "stale disconnect ignored" guards added in #51 were NOT exercised in the passing run, so the mechanism that killed the new session is plausible but not proven. The Windows-side release changes (#49, #50) did not fix it alone.
6. My test expectations were wrong twice (6 s M2 delay, budget after tampered SECRET); the spec and the harness now say the right thing.

**Not covered:** background advertising; MTU 23; two advertisers (two phones advertised at the same time only by accident); Notify on iOS (iOS uses Indicate); radio range (all runs Weak signal at the test station); other Windows adapters; Android background-session behaviour (the app fails closed when not in the foreground).

**Process notes:** the test-station PC runs the same receiver from a clean clone (docs/WINDOWS_TEST_STATION.md); the owner-seat variant allows the owner's sign-in there. Test hooks (adb `TEST_START`, auto-approve, fault modes) exist only in the Android debug source set and the iPhone `FAULT_INJECTION=1` build; Android's release check runs locally, not in CI (the module is not built by CI).

## Addendum 2026-11-06: iPhone same-code test 1 re-run (academy station)

Real radio, iPhone 15 Pro with the tamperM4 test build, synthetic code. The same code was tried three times: the receiver showed
`Failure budget used` 1/3, 2/3, 3/3 with `Core(ConfirmationFailed)` each time, all within 48 s of code creation. A first run in
tamperSecret mode correctly used 0/3 (D026). Defect found: after 3/3 the receiver still asked "Retry with the same code?" although
the code was already destroyed (Android printed "The code is no longer active"). Fixed: the receiver now prints that line and ends
without the retry prompt when the code is inactive. Re-observed after the fix (main 1b44e32, three tamperM4 sessions matched to the phone log): 1/3, 2/3, 3/3, then the "no longer active" line and no retry prompt. Raw phone logs are in the private memory.

## Addendum 2026-11-06: Phase 5 start, RDP/target-input feasibility (docs only)

Added docs/PHASE5_RDP_INPUT_FEASIBILITY.md (dated Microsoft Learn sources, findings table, test plan, fail-closed rules) and proposed
D029 (not adopted). Proven: only own-window ordinary-edit delivery with exact readback (Rust `input-probe`, owner click). Not proven:
other apps, UIA ValuePattern, mstsc/CredUI (readback and login proof are impossible without a separate test RDP server, and password
fields cannot be read back); secure desktop and elevated windows are unsupported. The prebuilt C++ probe `--self-input` at Medium
integrity (RID 8192, binary of 2026-11-04, not rebuilt) returned `OWNED_EDIT_SYNTHETIC_FAILED_OR_FOCUS_CHANGED` without a user click
(foreground not granted, nothing sent): inconclusive, not a delivery result. Policy unchanged: RDP/CredUI unsupported, real-secret
transfer disabled. No RDP session, credential UI, elevation or real secret was used.

## Addendum 2026-11-06: macOS receiver spike (Core Bluetooth Central) against the iPhone 15 Pro, reported by the macOS session

Prototype only (`apps/apple/KagiLinkReceiverSpike`): Apple Silicon Mac as BLE Central through the shared Rust core over the C ABI (Swift `Receiver`, `Codec`,
`FrameReassembler`, `Channel`), synthetic data, real-secret transfer disabled (D012), ad-hoc signed for local use. Same session rules as the Windows
receiver (60 s code, three failures with D026 counting, 10 s first-chunk wait, 5 s message completion, user-chosen advertiser, no auto-connect). The
phone tag was typed by a harness acting for the user; the code was shown in the app window only and never logged.

| Scenario (iPhone 15 Pro) | Mac receiver result |
|---|---|
| none (normal build) | Transfer OK: 33 bytes received in 1 fragments, budget 0/3 (chunk 244, indicate) |
| tampered M4, same code, three tries (fault build) | `Core(ConfirmationFailed)` with budget 1/3, 2/3, 3/3, then core `code()` inactive ("no longer active"); the iPhone showed M3 received and M4 flipped for each try |
| tampered M4, tries too slow for the 60 s code | `Core(Expired)`: operator latency, three earlier runs reached 1/3, 2/3 and the third try started after expiry |

Limits: the budget shown is derived by the Swift driver (the C ABI has no counter query); only these scenarios were run on the Mac receiver, the other fault
modes were not; the Mac was not tested at ATT MTU 23, with two advertisers or in the background. A Bluetooth permission prompt had to be accepted by the user once.
macOS app-bundle runs (not signed with a team certificate) say nothing about Developer ID, notarization or Secure Enclave. Nothing here proves device integrity.

### `kl_receiver_failures` read-only failure getter, ABI v3 (D030, 2026-11-06)

| Item | Status |
|---|---|
| `kl_receiver_failures` in `core/kagilink-ffi`, `kagilink.h`, `kagilink.hpp` (`Receiver::failures`), `KL_ABI_VERSION` 3, Android spike `EXPECTED_ABI` 3 | implemented |
| FFI tests: 0/3 initially, open attempt counted only after M3, 3/3 after budget spent, after success, NULL out pointers, NULL/stale/wrong-type/forged handles; unit test for expiry with a manual clock | passed locally (synthetic data) |

Not done: Swift/macOS spike not switched to the getter yet; no Android or Apple rebuild was run for this change.
