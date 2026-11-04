# KagiLink platform support — 2026-11-04

API feasibility is distinct from hardware validation and shipped support.

| Platform | Intended role / APIs | Current support |
|---|---|---|
| Windows | Central, C++20/C++/WinRT GATT, normal-user portable EXE | Probe only; no product BLE or secret input |
| iOS/iPadOS | Foreground Peripheral, SwiftUI/Core Bluetooth/Keychain/SE | Research only; no Apple build/device test |
| Android | Peripheral, Kotlin/BLE/StrongBox/biometric | Research only; no SDK/device test |
| macOS | Sender Peripheral + receiver Central, SwiftUI/Core Bluetooth/SE | Research only; no Apple Silicon device test |

Receiver discovery UI must reflect central scanning vs peripheral advertising.
iOS background advertising can hide UUIDs from non-iOS scanners; first version
requires sender foreground. USB dongle with basic Bluetooth branding is not proof
of GATT compatibility. Radio off, denied access, no adapter, driver/API or GATT
failure must produce an explicit unsupported/unavailable state.

Android must generate a StrongBox key, verify STRONGBOX security level with the
appropriate supported API and successfully use it. A feature flag is insufficient;
no TEE/software fallback even though generic platform examples suggest fallback.
Apple must create/use Secure Enclave keys and user-auth access control; storage
uses Keychain/encrypted vault, not arbitrary strings in the Enclave. Noncompatible
hardware is blocked. TPM provider availability alone is not TPM 2.0 verification;
Windows RAM mode must be labelled and must not promise hardware protection.

Windows CredUI/mstsc automated secret input: **unsupported, disabled**. Current
Microsoft restrictions make ordinary non-elevated credential autofill unreliable
or blocked. No UIAccess/elevation workaround. See PHASE0_RESEARCH.md.
