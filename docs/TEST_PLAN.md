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

## Manual native feasibility matrix

Record OS build, update level, CPU/adapter model, driver, privilege, packaging,
API result and synthetic outcome. Avoid collecting usernames or device addresses.

| Scenario | Pass condition | Current evidence |
|---|---|---|
| Windows own edit | Complete SendInput count AND exact owned-control readback | Manual --self-input; separate from RDP |
| Focus steal | Abort when foreground/focus changes; no false success | NOT RUN; races remain |
| Credential UI | Explicit blocked/unsupported result at ordinary integrity | NOT RUN; documented restriction |
| mstsc prompt | Distinguish input delivery from actual RDP connection | NOT RUN; disabled |
| Secure desktop/locked OS | No injection attempted; unsupported | NOT RUN; no bypass permitted |
| Windows dongle + iOS/Android peripheral | Discovery, connect, write, subscribe, notify, disconnect, MTU limits | NOT RUN |
| iOS/macOS required hardware | Actual protected key generation/use plus denial paths | NOT RUN |
| Android StrongBox | Positive device, missing hardware, thrown failure, wrong level, denied auth | NOT RUN |
| Windows TPM/RAM | Verify 2.0/key use or accurately labelled RAM mode | NOT RUN beyond provider probe |

## Phase 1+ mandatory tests

SPAKE2 public vectors, wrong code, invalid group values, failed mutual confirmation,
context/role/version/purpose/identity tamper; HKDF/AES known-answer vectors, tag
tamper, prior session replay, directional key separation, counter overflow and nonce
uniqueness; global rate limit; disconnect/crash/cancel/drop/FFI cleanup; hardware API
failure; vault update/delete/lost-device behavior; fresh biometric approval per send.

Add structured and byte-parser fuzz targets only after wire format is frozen.
Run sanitizers where supported, static analysis, advisory/license scans, and
cross-OS vector/FFI interoperability. Inspect every secret-bearing log/file/temp/
clipboard/crash path. Zeroization tests must not claim arbitrary dumps/copies are
impossible. UI tests include screenreader, keyboard, large fonts and small screens.
