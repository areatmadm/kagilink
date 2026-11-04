# KagiLink iOS/iPadOS sender — Phase 0 probe source

SwiftUI + foreground Core Bluetooth Peripheral. Open
`../apple/KagiLinkPhase0.xcodeproj`, target `KagiLink-iOS-Phase0` (iOS/iPadOS 18+).
Build SDK is exactly iOS/iPadOS 27.1 for iPhone Duo adaptation. Both Xcode target
configurations select `iphoneos27.1`; `sh apps/apple/build-probes.sh` checks the
selected SDK and refuses a missing/wrong version before building. Use an existing
Xcode 27.1 via process-local `DEVELOPER_DIR` if necessary; no global Xcode change
or download is performed. `--simulator` selects exactly `iphonesimulator27.1`.
Provisional diagnostic bundle ID: `in.minori.key.phase0.ios`. The app advertises only
a diagnostic service and answers a fixed public request. Expiry, cancellation,
inactive/background transition and invalid requests end the attempt. No secret
entry, vault, authentication or production transfer is implemented.

Controls use native Liquid Glass on iOS/iPadOS 26+ and public SwiftUI material
surfaces on iOS/iPadOS 18–25. Reduce Transparency or Increase Contrast selects
opaque, bordered controls. The scrollable layout reflows from the current window
width, uses semantic Dynamic Type and preserves the same probe/deadline during
resizing; it never infers a device from orientation, idiom or screen dimensions.
Content respects system safe areas, including Duo's vertical bars. Only the
decorative background extends outside those areas.

Follow [Apple's Duo preparation guidance](https://developer.apple.com/iphone-duo/prepare/)
and [Liquid Glass guidance](https://developer.apple.com/documentation/swiftui/applying-liquid-glass-to-custom-views).
Required Mac QA includes Duo folded/unfolded/partially folded and resizing while
idle/advertising, vertical system bars, largest text, VoiceOver, dark appearance,
Reduce Transparency and Increase Contrast. Simulator layout success does not
verify BLE or Secure Enclave hardware. Duo support is a source design target,
not a claim of a tested device result.

See [native BLE probes](../../docs/PHASE0_BLE_PROBE.md) for unsigned build commands
and device tests. Apple compilation and device tests have NOT RUN on this Windows
host. No account, certificate or provisioning profile was created. Secure Enclave
availability is an unverified capability label, not key-use evidence. Actual
protected key operations, per-transfer approval, denial/tamper paths and hardware
tests remain required before any sensitive transfer.
