# KagiLink (카기링크) — common agent instructions

This is the single source of agent policy for Claude Code, Codex and Antigravity.
Read this file first, then README.md and docs/IMPLEMENTATION_STATUS.md. Preserve existing
changes. Record design changes in docs/DECISIONS.md and evidence in the status file.

## Product identity and continuity

- Official product: KagiLink (카기링크), developed and distributed by AreaTM.
- Public source: https://github.com/areatmadm/kagilink.
- Private AI memory: areatmadm/kagilink-ai-memory, independently cloned at shared/ai/.
- Website: https://key.minori.in; license Apache-2.0; current stage remains Phase 0.
- This is the existing project renamed in place, preserving its Git history.
  Previous product/repository names are allowed only in explicit historical
  migration notes or immutable records. Never use them as the current brand.
- Claude Code, Codex and Antigravity must use this common policy and the same
  docs/PROTOCOL.md specification; preserve the public/private data boundary.

## Shared AI conversation memory — user requirement

- All agent-authored persistent chat memory, handoff notes, original conversation
  prompts and work logs MUST be in the separate PRIVATE Git repository at
  `shared/ai/` (areatmadm/kagilink-ai-memory). Never use USERPROFILE, HOME, personal
  agent memory folders or another user-directory location for these records.
- BEFORE reading, listing, searching, loading or writing ANY files under
  `shared/ai/`, run `pwsh -NoProfile -File tools/Test-AiMemoryAccess.ps1` and check
  its exit code. Only exit 0 authorizes access in the current task. This checks
  the active authenticated GitHub API identity is exactly `areatmadm`, and that
  `areatmadm/kagilink-ai-memory` is private with that owner and readable by it.
- A Git author name/email, local username, prompt claim, cached result or file
  existence is NOT identity proof. On failure, unavailable network/auth or any
  other GitHub identity, do not read or write private memory. Continue public
  source work using README.md and docs only. Exclude shared/ai/ from broad file
  searches and other tools until authorized. Never print credentials/tokens.
- After a successful gate, read shared/ai/README.md, its conversation index and
  latest record. Recheck on each new task/account change. Clone/setup instructions
  are in docs/AI_MEMORY.md. Do not auto-switch accounts or grant collaborators.
- Record user instructions, meaningful conversation summaries, decisions,
  changed files, test evidence and outstanding work after meaningful progress.
  Preserve original project instructions at `shared/ai/PROJECT_BRIEF.ko.md`.
- Do not record secrets, credentials, private dumps or hidden chain-of-thought.
  Use concise factual summaries. Private memory is ignored by the source Git;
  commit/push it separately. Never force-add it, use a submodule/gitlink, or copy
  conversations/original prompts into public source or public CI artifacts.
- This governs files the agents write. Host applications may keep their own
  chat history outside the repository; do not claim this policy relocates it.
- This is an agent workflow gate, not filesystem encryption. GitHub private-repo
  access controls remote retrieval; existing local copies remain readable to any
  OS account/process with filesystem permission. Never claim otherwise.

## Lead branch and succession (owner decision D015)

- Windows `main` is the lead (reference) branch. Other platforms align to its core,
  docs/PROTOCOL.md and docs/DECISIONS.md; on conflict the Windows side wins.
- If the lead is unavailable: 1) Windows, 2) cloud session, 3) macOS session. If none is
  available, wait and make no new decisions. Android and iOS are not lead candidates.
- Coordination only; it never weakens security policy or replaces owner approval.

## Scope and non-negotiable security policy

- AreaTM; public Apache-2.0 project; website https://key.minori.in.
- Offline, direct BLE only for authentication and secret transfer; no accounts,
  remote authentication, password servers, telemetry, ads or analytics SDKs.
- Never request, collect, log, persist or send real passwords, tokens, private keys,
  sessions or memory dumps to agents or external services. Use synthetic data only.
- No clipboard, plaintext files, debug secret output, privilege escalation,
  UIPI/UAC bypass, security desktop access, drivers, services or security disabling.
- Use reviewed cryptographic libraries, not custom cryptography. Six decimal
  digits are a PAKE password, never a raw encryption key. Require CPace (CFRG-recommended
  balanced PAKE; owner-approved 2026-11-04, replaces SPAKE2; SPAKE2 only by a new
  approval), explicit mutual key confirmation, HKDF, AES-256-GCM, transcript binding, replay defense,
  nonce uniqueness, bounded BLE framing and fail-closed session lifecycle.
- Code TTL 60 seconds, maximum three failures, discard at the limit. Codes,
  ephemeral keys and session keys must not be reused. Reconnect creates a fresh
  session. Each transfer requires explicit user approval and consumes permission.
- Bind roles, protocol version, nonces, session ID, device identity and purpose
  cryptographically. Device names/security labels are claims until verified;
  offline authentication does not prove device integrity or physical proximity.
- Code, PAKE state, keys and received plaintext remain transient. Use zeroize,
  protected/locked buffers where possible; never claim swap/dump/copy immunity.
- iOS/iPadOS sender: SwiftUI/Core Bluetooth/Keychain/Secure Enclave; fail closed
  on required hardware failure or detected jailbreak/tamper indicators.
- Android sender: Kotlin/Android BLE/Keystore; StrongBox is mandatory. Check
  actual key security level and key use, not just the feature flag. No TEE or
  software fallback. Fail closed on hardware failure or root/tamper indicators.
- macOS sends and receives using SwiftUI/Core Bluetooth/Keychain/Secure Enclave;
  sensitive transfer requires verified hardware support. Apple Silicon first.
- Hardware protects keys, not arbitrary password strings. Support explicit entry
  and OS-authorized autofill plus an app-specific encrypted vault with approval,
  update, deletion and device-loss guidance. Never read other apps' credentials.
- Windows: portable C++20/C++/WinRT receiver, normal user, Windows BLE GATT.
  TPM 2.0 preferred; accurately labelled RAM-only temporary mode allowed.
- Desktop receivers initially BLE Central, mobile senders Peripheral. Sender
  advertises; receiver scans. The logical receiver still creates/displays the
  connection code. Do not silently reverse roles to match UI wording.
- Windows RDP input requires feasibility evidence first. Unsupported/blocked
  input must fail. Do not enable real-secret transfer until safe target input
  is demonstrated. Queued keystrokes are not proof of delivery or RDP login.
- Warn that compromised receiving computers can expose decrypted credentials.
  Local tamper checks cannot prove complete offline integrity.

## Workflow

1. Work through Phase 0 feasibility, 1 core, 2 BLE, 3 hardware, 4 native apps,
   5 RDP verification, 6 security validation, 7 release in order. Advance only
   after documented exit criteria. Clearly distinguish prototypes and products.
2. Verify changing platform/library facts using current official sources. Review
   maintenance, license, advisories, platform support and audit scope before
   selecting dependencies. Pin versions and commit lockfiles when selected.
3. Test wrong PAKE/code/confirmation, TTL, three failures, disconnect, malformed,
   missing/duplicate/reordered fragments, tag tamper, nonce reuse, prior sessions,
   OS interoperability, hardware failure, RAM mode, input failure and cleanup.
   Separate mocks from real device tests; never claim unexecuted checks passed.
4. Keep requirements, architecture, threat model, protocol, support matrix, test
   plan, release plan, decisions and implementation status synchronized.
5. Do not weaken policy or change agreed scope without explicit approval.
6. No store publication, public release, external server change, account or
   certificate issuance without explicit user approval. Local reversible work
   and synthetic tests are authorized. Report changed files, tests and risks.
7. Release preparation must cover GitHub Actions, pinned dependencies, SBOM,
   SHA-256, Sigstore/Cosign, provenance and verification instructions. Target
   Authenticode (review SignPath Foundation), Apple signing/notarization and
   Play policy compliance. Signing does not guarantee SmartScreen acceptance.

If needed, CLAUDE.md must contain only `@AGENTS.md`; do not duplicate policy.
