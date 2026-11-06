# KL-V1 macOS receiver spike (BLE Central, Phase 0 prototype)

Spike only: Apple Silicon, macOS 14+, Core Bluetooth Central, synthetic data only, real-secret transfer disabled (D012).
Uses the shared Rust core through the C ABI (`KagiLinkCore` Swift wrapper: `Receiver`, `Codec`, `FrameReassembler`, `Channel`).
Session rules match the Windows receiver: code TTL 60 s and the three-failure budget (D026) live in the core, first chunk of the
next message within 10 s, a message completes within 5 s of its first chunk, the user (or a harness acting for the user) picks
the advertiser from a signal-sorted list, never auto-connects, duplicate/disagreeing tags are not selectable.
Nothing here proves device integrity or safety; names, tags and hardware results are claims.

```sh
sh apps/apple/KagiLinkCore/build-xcframework.sh
sh apps/apple/KagiLinkReceiverSpike/build-and-run.sh --run --stdout /tmp/mac-rx.log -- --choose-file /tmp/choose.txt
```

The app is ad-hoc signed for local use (no certificate, account or device identifier), and started through `open` so macOS asks
for Bluetooth permission for the app itself (one-time user click). The 6-digit code is shown in the window only; it is never
written to the log. Received plaintext is never logged (length only). The failure budget shown is derived by this driver (attempts
that sent M3 and did not finish) because the C ABI has no counter query; the core remains authoritative (`code()` stops working
once it is destroyed).
