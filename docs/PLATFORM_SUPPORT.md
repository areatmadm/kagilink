# KagiLink platform support — 2026-11-04

API feasibility is distinct from hardware validation and shipped support.

| Platform | Intended role / APIs | Current support |
|---|---|---|
| Windows | Central, C++20/C++/WinRT GATT, normal-user portable EXE | GUI/CLI built; visible inventory/start/stop/no-sender flows observed; real GATT exchange NOT RUN |
| iOS/iPadOS | Foreground Peripheral, SwiftUI/Core Bluetooth/Keychain/SE | Diagnostic source; iOS/iPadOS 18+, build SDK 27.1 for iPhone Duo; build/device NOT RUN |
| Android | Peripheral, Kotlin/BLE/StrongBox/biometric | Diagnostic source; Android 14/API 34+, compile SDK 37.2 and target 37; SDK build/device NOT RUN |
| macOS | Sender Peripheral + receiver Central, SwiftUI/Core Bluetooth/SE | Diagnostic target source, Apple Silicon/macOS 14+; build/device NOT RUN |

Receiver discovery UI must reflect central scanning vs peripheral advertising.
iOS background advertising can hide UUIDs from non-iOS scanners; first version
requires sender foreground. USB dongle with basic Bluetooth branding is not proof
of GATT compatibility. Radio off, denied access, no adapter, driver/API or GATT
failure must produce an explicit unsupported/unavailable state.

Android platform 37.2 is the latest stable SDK selected from Google's repository
metadata for this change. iOS SDK 27.1 is an explicit build requirement; minimum
iOS runtime remains 18. Mobile Liquid Glass visuals use native Apple glass where
available and an iOS 18 material fallback; Android uses platform glass styling.
Test contrast/reduced transparency, large text, rotation and resizing, including
iPhone Duo inner/outer displays and vertical system bars, before claiming UI support.

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
