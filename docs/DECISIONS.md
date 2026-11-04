# KagiLink decisions

## D001 — Phase 0 gate, 2026-11-04

Start with official-source review, synthetic Rust model and Windows feasibility
probe. Do not advance to production crypto/transfers before Phase 0 exit criteria.
No original files existed; no Git repository was present at initial inspection.

## D002 — BLE roles

Keep desktop Central and mobile Peripheral. Sender advertises, receiver scans.
The logical receiver creates the session/code; sender UI waits for connection.
This resolves discovery wording without silently reversing established roles.

## D003 — Windows credential input

Keep RDP/CredUI unsupported and real-secret transfer disabled. Current Windows
credential-dialog restrictions and focus races invalidate a blanket support claim.
An owned synthetic edit proves only that specific ordinary-window path. Do not
seek elevation, UIAccess, clipboard or other bypasses.

## D004 — Cryptographic library selection

No crypto dependency approved. RustCrypto SPAKE2 warns of missing audit/erasure;
review actual RFC compatibility, alternatives and memory behavior first. No mock
cryptographic substitute, custom PAKE, or implicit switch to SPAKE2+.

## D005 — Shared AI memory (explicit user instruction)

All agent-authored conversation memory/handoff notes live at shared/ai/ inside this
project in a separate private repository. Check owner identity before accessing
any contents; see AI_MEMORY.md. Do not use USER folders for agent memory. Preserve
original prompts only in private memory. App-managed chat history is outside the
scope of this file policy.

## D006 — Restricted-runner evidence

The local runner is Low integrity (RID 4096). Read-only inventory may report
diagnostics there, but exits incomplete and cannot prove normal-user feasibility.
Synthetic SendInput remains restricted to Medium integrity and was safely refused.
Do not remove input guards merely to obtain a test pass in the agent sandbox.

## D007 — Separate private memory and clean source history

Keep shared/ai/ ignored by the source repository, with its own private Git remote.
Source bootstrap policy permits memory access only after an authenticated
areatmadm identity/access check. Other contributors can work from public docs.
Publish only a clean source snapshot; preserve prior mixed history privately.
Never make a repository public merely after deleting files from its current tree.

## D008 — KagiLink identity migration, 2026-11-04

Rename the existing source and private-memory repositories in place to kagilink
and kagilink-ai-memory. Preserve repository IDs, commit ancestry, main branches,
visibility and permissions. This migration does not reset or recreate history.
KagiLink (카기링크) is the sole current product name; AreaTM remains the publisher.
Previous names are historical only. See MIGRATION_KAGILINK.md for verification.

Keep internal areatm-phase0-model crate name, workspace paths, native source
filenames and local workspace path for compatibility; none is a product display
name. Update Cargo repository metadata and the probe output filename/display
resources instead. No deployed app bundle IDs or external signing identities
exist. Do not rename the earlier private archive or rewrite historical records.

## D009 — Local workspace relocation, 2026-11-04

The local workspace is now S:\project\kagilink after the folder move. This
supersedes D008's local-path compatibility choice only. Keep both Git histories,
origin URLs, private-memory isolation, internal crate names and Phase 0 scope.
Build scripts resolve from their own location and require no hardcoded old path.
Post-move validation evidence is recorded in IMPLEMENTATION_STATUS.md.

## D010 — Native Phase 0 diagnostic targets, 2026-11-04

Prepare Windows Central, Android/iOS Peripheral and macOS Central/Peripheral
feasibility sources without advancing to production native apps. Diagnostic UUIDs
and fixed public request/reply values are isolated from PROTOCOL.md; never accept
secret input, treat a fixed reply as authentication or connect this path to input.
Keep one explicit-start attempt, a 60-second deadline and terminal cleanup. Native
hardware labels stay unverified until actual protected key operations are tested.

Use SwiftUI/Core Bluetooth shared source in apps/apple with distinct unsigned
iOS/iPadOS 18+ and Apple Silicon/macOS 14+ targets. Build the iOS target with the
explicit iOS SDK 27.1 for iPhone Duo. Use Kotlin/platform Android Views/BLE,
Android 14/API 34+ with the latest stable Android SDK (currently platform 37.2),
with signing disabled. These provisional diagnostic IDs do not finalize production
signing identity. The mobile minimum OS versions are required support floors. Android
tool versions and generated locks are reviewed separately from security-core
selection. See DEPENDENCY_REVIEW.md and PHASE0_BLE_PROBE.md.

Keep native builds and device evidence distinct from static source/project tests.
Windows CLI can be built here; Android SDK and Apple build/device validation remain
pending on a suitable host. No account, certificate, SDK license or publication
action is automated. Phase 0 exit criteria remain unchanged and unmet.

Mobile screens use Liquid Glass styling. Use Apple's native glass API only on
supported OS versions, with a system-material fallback on iOS 18; Android uses
platform-rendered translucent glass styling without importing an Apple framework
or third-party UI SDK. Respect accessibility contrast, reduced transparency and
text scaling. The visual material is not a claim of authentication or protection.
Adaptive layouts must handle iPhone Duo inner/outer displays, resizing, safe areas
and vertical system bars. Actual rendering and device behavior remain unverified.
