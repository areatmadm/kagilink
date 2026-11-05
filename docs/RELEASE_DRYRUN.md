# Release dry run (Phase 7 preparation)

Status: preparation only. Nothing here publishes, signs or attests anything. The
workflow `.github/workflows/release-dryrun.yml` is manual (`workflow_dispatch`) and was
NOT executed when it was written; no result is claimed here.

## What the dry run does

Permissions are `contents: read` only. No secrets, no `id-token`, no attestations.
Actions are pinned by full commit SHA (the same SHAs as `core-hardening.yml` and `fuzz.yml`).

1. `windows-receiver` (windows-latest)
   - Installs Rust 1.92.0 and runs `cargo build --release --locked -p areatm-kagilink-receiver`.
   - Cleans and builds a second time at the same path, then prints both SHA-256 values and
     whether they are identical. A difference is reported, not treated as failure (Rust/MSVC
     builds may embed paths or timestamps; no reproducibility claim is made until it is run).
   - Writes `SHA256SUMS.txt` and keeps the UNSIGNED exe plus the sums as a 7-day workflow
     artifact (not a release, not public download material).
2. `sbom` (ubuntu-latest)
   - Installs `cargo-cyclonedx` 0.5.9 with `--locked` (version taken from `cargo search`; its
     own dependency tree is not audited here) and generates CycloneDX JSON per workspace crate,
     printing each file's component count. The SBOM is not uploaded.

## Deliberately NOT done (each awaits explicit owner approval)

| Item | Why not now | Approval needed |
| --- | --- | --- |
| Sigstore/Cosign signing | Keyless signing needs `id-token` and writes to the public Rekor transparency log | Owner approval to publish signing records publicly |
| Build provenance / attestations | Needs `id-token` and `attestations: write`; public record | Same as above |
| GitHub Release | Public distribution | Owner approval of a release, after Phase 7 exit criteria |
| Windows Authenticode / SignPath Foundation | Needs an application, certificate issuance and secrets | Owner approval for account/certificate; signing does not guarantee SmartScreen acceptance |
| Apple signing and notarization | Needs Apple Developer account and certificates | Owner approval and account |
| Google Play publication | Store account and policy review (StrongBox-mandatory sender) | Owner approval; Play policy compliance review |
| Uploading SBOM/hashes to external services | External server change | Owner approval |
| End-user verification instructions | Meaningful only once signed artifacts exist | Written with the first approved release |

Real-secret transfer remains disabled; the dry run uses no credentials or real data.
