# KagiLinkRiskAck

Risk acknowledgement screen and decision logic for the KagiLink sender apps (D023,
[`docs/RISK_ACK_SCREEN_SPEC.md`](../../../docs/RISK_ACK_SCREEN_SPEC.md)). Pure Swift/SwiftUI, iOS 18 / macOS 14, no
Rust or UIKit dependency, so iOS/iPadOS and macOS share one implementation. Synthetic data only; the text is a DRAFT.

- `RiskNoticeText.swift`: the single wording resource (ko/en), copied verbatim from `docs/RISK_NOTICE.md`. Bump `RiskNotice.version` on any change.
- `NoticeDecision.swift`: `needsNotice(...)`, the local record (version, receiver hash, timestamp only) and its storage.
- `RiskAckGate.swift`: fail-closed state machine; `consumeForTransfer()` makes one acknowledgement single use.
- `RiskAckView.swift`: the SwiftUI screen (checkbox starts unchecked; Continue disabled until checked).

Use: `gate.evaluate(receiver:)` before the code entry/Start step, show `RiskAckView` unless `gate.state == .acknowledged`,
and call `gate.consumeForTransfer()` when starting. Tests: `swift test` in this directory (they read `docs/RISK_NOTICE.md`).
Apps compiled with plain `swiftc` (the spike) include the sources directly; the `canImport` guard covers both layouts.
