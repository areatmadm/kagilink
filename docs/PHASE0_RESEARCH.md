# KagiLink Phase 0 official-source review — checked 2026-11-04

This is an API/documentation review, not a certification or real-device result.

## Windows

[Microsoft BLE desktop guidance](https://learn.microsoft.com/en-us/windows/win32/bluetooth/about-bluetooth)
directs desktop GATT use to WinRT/C++/WinRT. [BLE overview](https://learn.microsoft.com/en-us/windows/uwp/devices-sensors/bluetooth-low-energy-overview)
describes Central/Peripheral roles. An unpackaged normal-user probe must still
verify adapter, policy, device access and full GATT operation on target hardware.
No packaging/consent behavior is assumed from documentation alone.

[SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)
is constrained by integrity levels. It reports inserted events, and cannot diagnose
UIPI specifically. This does not verify target delivery or authentication.
[January 2026 credential-dialog changes](https://support.microsoft.com/en-us/servicing/os/windows/docs/2026/01/new-behavior-restricting-certain-applications-to-autofill-credentials-introduced-by-the-windows-janu)
restrict accepted input sources. Inference for this project's ordinary-user/no-bypass
requirements: CredUI and mstsc cannot be claimed supported. Keep real secret transfer
disabled until input safety is independently demonstrated; restrictions are not
permission to seek elevation or UIAccess. Own-window synthetic input is a separate test.

[CNG providers](https://learn.microsoft.com/en-us/windows/win32/seccertenroll/cng-key-storage-providers)
identify the Platform Crypto Provider. Opening it only checks provider access;
TPM 2.0/key generation/nonexportability/key use are separate required tests.

## Apple

[Core Bluetooth advertising](https://developer.apple.com/documentation/corebluetooth/cbperipheralmanager/startadvertising(_:))
documents restricted advertising data/background behavior. Use foreground-only
mobile Peripheral and desktop Central first. Verify actual GATT exchange on devices.
[Secure Enclave key protection](https://developer.apple.com/documentation/security/protecting-keys-with-the-secure-enclave)
requires supported hardware and Enclave-generated keys; use hardware key operations
and protected vault storage as distinct components. Simulator API presence cannot
prove actual hardware backing or offline integrity.

## Android

[Keystore/StrongBox](https://developer.android.com/privacy-and-security/keystore)
describes feature detection, StrongBox-backed generation and key security level.
Project policy is stricter than generic fallback examples: reject generation/use
failure and any non-StrongBox key. Confirm biometry-bound operation and unsupported
devices. [BLE permissions](https://developer.android.com/develop/connectivity/bluetooth/bt-permissions)
must be tested against chosen target SDK; permissions and actual advertising/GATT
server callbacks are required, not just compile success.

## Phase 0 exit criteria

1. Portable receiver builds and probes WinRT adapter/GATT availability at normal
   integrity; foreground sender advertises and exchanges synthetic GATT messages.
2. Record ordinary edit, CredUI, mstsc, secure desktop and focus-change behavior
   separately using synthetic data. Do not disable protections to get a pass.
3. Select a reviewed SPAKE2 implementation/suite with compatibility vectors,
   license/maintenance/advisory/memory-lifecycle evidence (DEPENDENCY_REVIEW.md).
4. Define supported configurations and fail-closed target/approval gates.

These criteria are NOT all met. Phase 1 production implementation remains gated.
