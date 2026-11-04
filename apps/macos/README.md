# KagiLink macOS sender/receiver — Phase 0 probe source

SwiftUI/Core Bluetooth: sender Peripheral, receiver Central, one role at a time.
Open `../apple/KagiLinkPhase0.xcodeproj`, target `KagiLink-macOS-Phase0` (Apple
Silicon/macOS 14+). Provisional diagnostic bundle ID: `in.minori.key.phase0.macos`.
The sender advertises for at most 60 seconds; the receiver scans for 10 seconds,
refuses ambiguous observed senders, then writes/reads fixed public test bytes.
Expiry, inactive transition, cancellation and API failure close the attempt.

Shared SwiftUI controls use native Liquid Glass on macOS 26+ and material surfaces
on macOS 14–25, with opaque bordered controls when Reduce Transparency or Increase
Contrast is enabled. Content reflows from the current window width and remains
scrollable at large text sizes. Resizing preserves the probe and its original
deadline; an inactive/background transition aborts instead of restarting it.

`sh apps/apple/build-probes.sh` first requires the exact iOS/iPadOS 27.1 SDK for
the sibling Duo target, then builds both unsigned targets. It does not change the
global Xcode selection. Use process-local `DEVELOPER_DIR` to select an existing
Xcode 27.1 installation. macOS SDK selection remains the selected Xcode's SDK;
the macOS deployment minimum remains 14.0.

No secret entry, vault, authentication, input injection or production transfer is
implemented. The sandbox entitlements request Bluetooth only. Secure Enclave
availability is explicitly unverified; protected key operations/user approval still
gate production. No account/certificate/provisioning changes occurred. See
[native BLE probes](../../docs/PHASE0_BLE_PROBE.md). Apple build, signed sandbox
behavior and real-device tests have NOT RUN on the Windows-only host.
