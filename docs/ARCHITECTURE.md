# KagiLink architecture — Phase 0

Native apps own BLE, user authentication, secure storage and UI. A future Rust
security core owns cryptographic protocol state through a narrow, versioned C ABI.
No ABI, cryptographic implementation or product UI is shipped at this stage.

```text
Sender (iOS/Android/macOS, BLE Peripheral)
  explicit entry / authorized autofill / encrypted app vault
  hardware-backed key use + local approval
                  |
          offline BLE GATT
                  |
Receiver (Windows/macOS, BLE Central)
  code display -> authenticated session -> validated target -> one-use input
```

The central scans for the sender's foreground advertisement. The peripheral does
not scan for the central: sender UI says “waiting for receiver connection”, then
shows authenticated receiver information after the handshake. The receiver creates
the logical session and pairing code; BLE role and application role are distinct.

Future FFI: opaque single-owner handles; bounded byte slices; explicit destroy;
no Rust panic across ABI; no secret Debug/Display or implicit string copying.
Hardware APIs stay native. Hardware-wrapped vault keys and transient PAKE/session
keys have different lifecycles. A TPM does not make arbitrary Rust AES buffers
hardware resident. No host trust claim follows just from a peer's UI label.

`core/phase0-model` is a dependency-free simulation, not the future security core.
Its events are deliberately named simulate_* and never accept secret payloads.
Its assembler contains public test bytes only, and clear() is not zeroization.
Native Phase 0 probes use a separate unauthenticated diagnostic service to exchange
only two fixed 16-byte public values. Windows is a standalone Central executable;
Android/iOS are foreground Peripherals; macOS supports either role. SwiftUI Apple
targets share code in apps/apple; Android uses platform Views/BLE without AndroidX.
These are feasibility probes, not native product UI or the production BLE transport.
No diagnostic event is connected to the Rust model as authentication/approval.
See PHASE0_BLE_PROBE.md for its deliberately separate contract and limitations.

Windows adds a separate native diagnostic shell, `kagilink.exe`, around the existing
`kagilink-phase0-probe.exe` CLI. The GUI has two fixed child actions (inventory and
public BLE test), a read-only result area and cancel/close cleanup. Child execution
uses no shell or arbitrary command text. Timer polling keeps the window responsive;
captured output is bounded and a kill-on-close job plus outer deadline supervises
the process. No diagnostic event can authorize input or represent authenticated
transfer. The child's existing integrity checks and 60-second BLE lifetime remain
authoritative. This shell is Phase 0 diagnostic UI, not the Phase 4 product UI.

Mobile support floors are Android 14/API 34 and iOS/iPadOS 18. Android's compile
SDK tracks the current stable SDK (pinned platform 37.2 for this change); Apple's
iOS target requires SDK 27.1 to opt into iPhone Duo layout behavior. These build
requirements do not establish real-device support. Native Liquid Glass on supported
Apple OS versions falls back to system materials on iOS 18, while Android renders
its glass appearance with platform UI. Appearance stays separate from BLE/security
state, and adaptive layouts must retain usable controls at large text sizes.
