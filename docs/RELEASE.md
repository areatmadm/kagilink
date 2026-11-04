# KagiLink release plan — no release authorized

Pre-release blockers: Phase 0 gates, complete core/native apps, independent security
review, real devices, input safety and accurate support matrix. A runnable probe
is not a password-transfer product and must not be marketed as one.

Native diagnostic projects in apps/android and apps/apple are configured for unsigned builds.
Their provisional Phase 0 IDs are not product release identity. Required mobile
floors are Android 14 and iOS/iPadOS 18; Android must use the current stable SDK,
and iOS builds must use SDK 27.1 for iPhone Duo. These requirements do not prove
device support. Do not publish these as credential-transfer applications.
Actual Android SDK and Apple builds, locks on those SDKs, signed entitlement
behavior and real-device interoperability remain release blockers.
Liquid Glass/material fallbacks, contrast/reduced transparency, large text and
iPhone Duo inner/outer display layout must pass visual/device QA before release.

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

## KagiLink repository and artifact identity

Source and future releases: https://github.com/areatmadm/kagilink.
Research diagnostic window: kagilink.exe, with sibling CLI kagilink-phase0-probe.exe.
These two local diagnostic executables are not released products. Keep their
pairing/version and bounded fixed-action behavior in future reproducibility checks.
Release note template: ../.github/RELEASE_TEMPLATE.md.
No release/signing/upload workflow, Cosign expected workflow identity, download
link, store package identifier or published artifact exists yet. Future Sigstore
verification must bind the exact workflow under areatmadm/kagilink; freeze that
identity only when a reviewed release workflow exists. No secret values were
read/copied, and the repository was renamed in place to retain its settings.
