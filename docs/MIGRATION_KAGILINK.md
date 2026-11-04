# KagiLink identity migration — 2026-11-04

This is an in-place name migration of the existing AreaTM project, not a new
project or history reset. Product: KagiLink (카기링크). Publisher: AreaTM.
Website: https://key.minori.in. License: Apache-2.0. Stage remains Phase 0.

## Historical names and repository continuity

| Repository | Before (historical) | After | Visibility |
|---|---|---|---|
| Source | areatmadm/pwreceiver | areatmadm/kagilink | PUBLIC |
| AI memory | areatmadm/pwreceiver-ai-memory | areatmadm/kagilink-ai-memory | PRIVATE |

The historical product name was AreaTM BLE Secure Transfer. Use it only when
describing history. The earlier private archive retains its historical name;
it is not the active source or AI-memory remote and remains private.

Before rename: both local worktrees clean, main branches, authenticated areatmadm
with admin/pull/push access. Source repository ID 1445065839, main HEAD
3829b2e25070f39d8e2440477e87443ebeea9449. Private identity/HEAD evidence is stored
only in private memory. Both repositories had only main, no tags, and only the
owner as collaborator. Source had no releases or deployments; latest CI passed.

After rename, source ID and private ID were unchanged, default main branches and
all pre-migration branch HEADs/protection flags were unchanged. Updated commits
are descendants of those HEADs, not replacements. No repository was created or
deleted. Local origin URLs now point directly at the new names. About descriptions
were updated; source homepage now points to the existing official domain.

## Reference and compatibility review

- AGENTS.md is the sole common policy for Codex, Claude Code and Antigravity.
  CLAUDE.md remains only @AGENTS.md. No conflicting agent adapters were found.
- Current docs, source links, Cargo metadata and memory gate/setup point at the
  new repositories. The memory gate rejects historical repository metadata.
- Internal crate areatm-phase0-model, core/phase0-model, source filenames and the
  local workspace path initially retained their existing identity. The folder
  subsequently moved to S:\project\kagilink; see the relocation verification in
  IMPLEMENTATION_STATUS.md. No external FFI/package
  IDs, bundle IDs, store apps or deployed protocol identifiers exist to migrate.
- Windows output changes to kagilink-phase0-probe.exe, with KagiLink title, CLI
  help/version and executable product/copyright metadata. Build path remains
  target/phase0. Old ignored local binaries may remain; rebuild/use the new name.
- Native mobile/macOS apps and website consist of planning READMEs only. Updated
  the planned display names/site metadata; no actual app bundles or hosted site
  exist in this repository. No DNS or Cloudflare changes were made.
- Existing synthetic CI path stays .github/workflows/phase0.yml. Its display name
  changes only; no private checkout or private-content output is introduced.
- No release/signing workflow, published artifacts, signing workflow identity,
  production download links or release tags existed. Added a KagiLink draft
  release note template; no release or certificate issuance was performed.
- GitHub secret values were never read, printed or copied. Settings remain on
  the same repository IDs. Existing historical private archive is unchanged.

## Isolation and verification

Keep /shared/ai/ ignored, independently versioned and private. Owner authentication
must succeed before any memory access. No memory or original prompts may enter
source commits, public CI, artifacts or build inputs. Local file permissions and
offline/platform security limitations are unchanged; no integrity bypass added.

Executed checks and CI results are recorded in IMPLEMENTATION_STATUS.md. Private
hardware-backed auth, real BLE exchange, secure input and RDP are still unverified.

Migration source commit: ac4fc2f. GitHub Actions run 41762980276 succeeded at
https://github.com/areatmadm/kagilink/actions/runs/41762980276. The existing workflow
ID 406436318 remains unchanged. GitHub compare confirms pre-migration main is an
ancestor (ahead, not diverged). Post-push IDs, visibility and collaborator/admin
permissions match baseline; the public tree has no private-memory/original-prompt
paths. Existing hardware/input limitations remain explicitly incomplete.

## Handoff

Use the existing local workspace at S:\project\kagilink. Fetch/pull from
the updated origin URLs. Read AGENTS.md first, then README.md and
IMPLEMENTATION_STATUS.md. Only authenticated areatmadm may pass the updated memory
gate and read the independent private handoff. Historical private records and old
commits retain earlier names as history; do not rewrite or republish them.

## Changed source files

| Area | Files |
|---|---|
| Common identity/policy | AGENTS.md, README.md, NOTICE, CHANGELOG.md, CONTRIBUTING.md, SECURITY.md |
| Cargo metadata | Cargo.toml, core/phase0-model/Cargo.toml |
| CI/release notes | .github/workflows/phase0.yml, .github/RELEASE_TEMPLATE.md (new) |
| Native plans/site | apps/ios/README.md, apps/android/README.md, apps/macos/README.md, website/README.md |
| Windows probe | apps/windows/README.md, apps/windows/build-probe.ps1, apps/windows/probe.cpp, apps/windows/probe.rc (new) |
| Memory access/bootstrap tests | tools/Test-AiMemoryAccess.ps1, tools/Initialize-AiMemory.ps1, tests/security/test-ai-memory-gate.ps1 |
| Design/security docs | docs/ARCHITECTURE.md, docs/REQUIREMENTS.md, docs/THREAT_MODEL.md, docs/PROTOCOL.md, docs/PLATFORM_SUPPORT.md |
| Process/evidence docs | docs/AI_MEMORY.md, docs/PHASE0_RESEARCH.md, docs/DEPENDENCY_REVIEW.md, docs/TEST_PLAN.md, docs/RELEASE.md, docs/DECISIONS.md, docs/IMPLEMENTATION_STATUS.md, docs/MIGRATION_KAGILINK.md (new) |

Private README/index/handoff updates are committed only to the independent private
repository. CLAUDE.md, .gitignore, Cargo.lock, core protocol model, manifest and
historical commits remain unchanged.
