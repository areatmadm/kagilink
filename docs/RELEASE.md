# Release plan — no release authorized

Pre-release blockers: Phase 0 gates, complete core/native apps, independent security
review, real devices, input safety and accurate support matrix. A runnable probe
is not a password-transfer product and must not be marketed as one.

Prepare GitHub Actions with least-privilege permissions, reviewed commit-pinned
actions, pinned toolchains/dependencies and locked builds. SBOM, third-party
licenses/NOTICE, SHA-256, Sigstore/Cosign verification, build provenance and
repeatable versioned artifacts are required before a public release.

Windows: review [SignPath Foundation](https://signpath.org/) eligibility for OSS
Authenticode. A signature or Sigstore alone does not guarantee SmartScreen or
enterprise policy acceptance. No self-signing is represented as trusted publisher.
Apple: AreaTM organization signing, entitlements/privacy, App Store review and
macOS notarization. Android: verify actual AreaTM account type, current Play
testing requirements, signing and data-safety declarations before submission.
Website key.minori.in and GitHub Releases stay public; optional Cloudflare Access
protects operator management only. Core operation must never depend on them.

Account/certificate issuance, public GitHub creation/releases, store submission,
DNS/Cloudflare/site changes require the user's explicit approval at that stage.
These actions have not been performed.
