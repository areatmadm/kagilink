# Risk acknowledgement screen: sender app requirements (D023)

Status: requirements, 2026-11-07. Applies to the iOS/iPadOS, Android and macOS sender apps (spikes first, products later).
Text source: `docs/RISK_NOTICE.md` (DRAFT, not legally reviewed). This spec changes no decision; it makes D023 testable. Real-secret
transfer stays disabled (D012); the screen is built and tested with synthetic data only.

## When the notice must be shown

1. Before the first transfer ever made from this app installation.
2. Whenever the receiver changes. A receiver is identified by its **claimed** device identity once the protocol carries one
   (claims, never proof). Until a stable receiver identity exists, the KL-XXXX tag is per session and is NOT an identity, so the
   notice is shown **before every transfer** (fail-safe default).
3. Whenever the notice version (`NOTICE_VERSION`, bumped on any text change) differs from the one last acknowledged.
4. Never skipped by a setting, a "do not show again" option or a stored default.

## Behaviour

* Short on-screen text from `RISK_NOTICE.md` (ko or en by device language, other languages fall back to en). The full notice is one
  tap away and scrollable. The text lives in ONE resource file per platform, generated or copied verbatim from the document, so a
  wording change cannot drift between apps. The screen states that the text is a draft pending legal review until that is done.
* The acknowledgement control is a checkbox that starts **unchecked**; the continue button is disabled until it is checked. No
  pre-checked state, no auto-advance, no timer-based acceptance.
* The notice is separate from the per-transfer approval (AGENTS.md: each transfer requires explicit approval and consumes it). The
  acknowledgement never replaces that approval, and the approval never implies the acknowledgement.
* Wording rules: the receiver and any TPM/device result are **claims**. The UI must never use "safe", "verified safe", "secure PC"
  or equivalents (Korean: "안전함", "검증됨") for a PC. Boot-state or TPM information is shown as "the PC claims ...".
* Stored record (local, non-secret): `NOTICE_VERSION`, a hash of the claimed receiver identity (when available) and a timestamp. No
  personal data, no codes, no secrets, no telemetry. It is kept only to decide whether to show the screen again (rules 2 and 3).
  If the record is unreadable or corrupt, treat it as absent and show the screen.
* Fail closed: if the notice text cannot be loaded or the acknowledgement state cannot be determined, no transfer starts.
* Accessibility: works with screen readers (the checkbox and button have labels and state), supports large text and small screens,
  does not rely on colour alone.
* Not in scope: detecting keyloggers or malware on the receiver (no duty and no claim, D023 clarification).

## Tests (synthetic data only)

1. Unit tests of the decision function `needsNotice(storedVersion, storedReceiverHash, currentVersion, currentReceiverHash)`:
   first use; same version and receiver (no notice only when a stable identity exists); changed receiver; bumped version; corrupt
   record; unknown identity (must show).
2. UI test: button disabled while unchecked, enabled after checking, unchecking disables it again; the screen appears before the
   code entry/Start step; navigating back does not leave an acknowledged state behind.
3. String test: no forbidden words ("safe"/"verified safe"/"안전함"/"검증됨" used for a PC) in the resource files; the Korean and
   English short texts equal the strings in `docs/RISK_NOTICE.md` (a check script or test).
4. Failure tests: missing resource, corrupt storage, and version mismatch each end in "show the screen" or "no transfer", never in a
   silent skip.

## Reporting

Each platform session records the evidence (tests run, what is mock versus device) in `docs/IMPLEMENTATION_STATUS.md` and opens a PR;
the lead merges after CI. Do not claim legal sufficiency; say the text is a draft.
