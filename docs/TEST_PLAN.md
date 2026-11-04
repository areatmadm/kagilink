# KagiLink test plan

Use public synthetic values only. Never request or record a real RDP password.
The local test commands and actual outcomes are in IMPLEMENTATION_STATUS.md.

## Automatic Phase 0 checks

- Rust model: strict 60s deadline including exact boundary/clock regression;
  third failure terminal; confirmation/approval ordering; one-use permission;
  cancellation; bounded fragments, malformed metadata, prior session/message,
  duplicate/reordered/missing fragment cleanup and terminal failures.
- fmt + clippy with warnings denied. These do not verify cryptographic properties.
- Windows build + --policy checks the hard-disabled real-input policy.
- --inventory queries adapter/LE support and opens/closes TPM provider only.
  Low-integrity execution is diagnostic-only, exits 2 and cannot pass the
  normal-user feasibility criterion. --self-input refuses non-Medium execution.
- Probe executable: target/phase0/kagilink-phase0-probe.exe; --help and --version
  report KagiLink without accessing BLE, private memory or input APIs.
- `tests/windows/test-probe.ps1`: help/version/policy and rejection of unknown or
  extra arguments. Runs without BLE scanning, focus changes or input injection.
- Windows GUI: `target/phase0/kagilink.exe` opens a persistent diagnostic window;
  `kagilink-phase0-probe.exe` remains the CLI. Verify no automatic probe at startup,
  fixed action dispatch, read-only output, busy-state controls, responsive resize,
  cancellation/close cleanup, missing sibling executable and the 70-second outer
  supervisor limit. This does not replace the child's 60-second BLE deadline.
- `tests/windows/test-gui-runner.ps1` compiles synthetic child/process fixtures for
  invalid action rejection, both fixed commands/final output, the 16 KiB output
  cap, contained-child cancellation and explicit restart. It uses no BLE, GUI or
  input injection. It does not wait out the 70-second supervisor timeout.
- `tests/native/test_probe_contract.py`: cross-language diagnostic UUID/value
  consistency, default ATT budget, Android permissions/no backup, Apple privacy/
  sandbox plists and Xcode target/file references. Static checks are not compilation.
- `--ble-test` is opt-in and Medium-only; writes/reads fixed public values on a
  separate diagnostic service. No real exchange has been executed here. See
  PHASE0_BLE_PROBE.md for its contract, limits and cross-OS failure matrix.
- Android Gradle dependency resolution/locking can run without an SDK; it must not
  be reported as Android compilation, lint, device execution or security validation.
- Source checks also cover Android 14/API 34 floor, selected stable SDK pins,
  runtime dependency allowlist, iOS 18 deployment and SDK 27.1 build enforcement.
- `sh apps/apple/test-build-script.sh`: synthetic `uname`/`xcrun`/`xcodebuild`
  fixtures test exact device/simulator SDK selection, unsigned invocation and
  refusal of wrong SDK/unknown arguments. These mocks never compile an Apple app.

## Manual native feasibility matrix

Record OS build, update level, CPU/adapter model, driver, privilege, packaging,
API result and synthetic outcome. Avoid collecting usernames or device addresses.

| Scenario | Pass condition | Current evidence |
|---|---|---|
| Windows own edit | Complete SendInput count AND exact owned-control readback | Manual --self-input; separate from RDP |
| Windows diagnostic GUI | Direct launch, readable result/status, keyboard navigation, scaling and cancel/close without orphan child | Manual open, inventory/start/stop/restart/no-sender/details and one window-size screenshot observed; broad keyboard/DPI/high-contrast/screenreader and close/deadline cases NOT RUN |
| Focus steal | Abort when foreground/focus changes; no false success | NOT RUN; races remain |
| Credential UI | Explicit blocked/unsupported result at ordinary integrity | NOT RUN; documented restriction |
| mstsc prompt | Distinguish input delivery from actual RDP connection | NOT RUN; disabled |
| Secure desktop/locked OS | No injection attempted; unsupported | NOT RUN; no bypass permitted |
| Windows dongle + iOS/Android peripheral | Discovery, connect, write, subscribe, notify, disconnect, MTU limits | NOT RUN |
| iOS/macOS required hardware | Actual protected key generation/use plus denial paths | NOT RUN |
| Android StrongBox | Positive device, missing hardware, thrown failure, wrong level, denied auth | NOT RUN |
| Windows TPM/RAM | Verify 2.0/key use or accurately labelled RAM mode | NOT RUN beyond provider probe |
| Android 14 and latest stable Android | SDK build/lint, permissions, foreground/radio lifecycle and glass rendering | NOT RUN |
| iOS 18 material fallback | Usable contrast, text scaling, controls and foreground cleanup | NOT RUN |
| Native Apple Liquid Glass | Supported OS rendering, reduced transparency and VoiceOver | NOT RUN |
| iPhone Duo, iOS SDK 27.1 | Inner/outer display resize, rotation, safe areas, vertical system bars and large text | NOT RUN |

## Phase 1+ mandatory tests

CPace public vectors (draft Appendix B), wrong code, invalid group values, failed mutual confirmation,
context/role/version/purpose/identity tamper; HKDF/AES known-answer vectors, tag
tamper, prior session replay, directional key separation, counter overflow and nonce
uniqueness; global rate limit; disconnect/crash/cancel/drop/FFI cleanup; hardware API
failure; vault update/delete/lost-device behavior; fresh biometric approval per send.

Add structured and byte-parser fuzz targets only after wire format is frozen.
Run sanitizers where supported, static analysis, advisory/license scans, and
cross-OS vector/FFI interoperability. Inspect every secret-bearing log/file/temp/
clipboard/crash path. Zeroization tests must not claim arbitrary dumps/copies are
impossible. UI tests include screenreader, keyboard, large fonts and small screens.
