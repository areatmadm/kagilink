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
