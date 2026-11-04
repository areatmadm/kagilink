# Architecture — Phase 0

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
Windows probe is a standalone feasibility executable with no BLE data transport.
