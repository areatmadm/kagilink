# Implementation status — 2026-11-04 (Asia/Seoul)

Phase 0 is IN PROGRESS. Real-secret transfer is disabled. This is not release ready.

## Environment

Initial workspace: empty S:\project\pwreceiver; not a Git repository.
Source publication is separate from a product release. The source/memory Git split
uses a clean source history, an independent ignored private repository at
shared/ai/, and a private archive for earlier mixed history. See AI_MEMORY.md.
Original conversation prompts/transcripts are excluded from the source snapshot.
No release or binary was published.

- Source: https://github.com/areatmadm/pwreceiver — PUBLIC, fresh source-only history.
- Memory: areatmadm/pwreceiver-ai-memory — PRIVATE, only owner listed as collaborator.
- Earlier mixed history: areatmadm/pwreceiver-private-archive — PRIVATE.
- Source full local object history and remote tree contain no memory/original-prompt
  paths; no submodule/gitlink. Earlier local Git metadata preserved under ignored target/.
Rust/cargo 1.92.0, MSVC toolchain target; fmt/clippy installed. Visual Studio 2022
Community and Windows SDK 10.0.22621.0 / 10.0.26100.0 detected.

## Work

| Phase | State | Evidence / remaining work |
|---|---|---|
| 0 | In progress | Official review, policy/docs and synthetic prototypes; native/device validation pending |
| 1 | Not started | Dependency/suite gate unresolved; no production PAKE/HKDF/AES/zeroization |
| 2 | Not started | No end-to-end BLE transport |
| 3 | Not started | No verified StrongBox/SE/TPM key integration |
| 4 | Not started | No native product UI/vault |
| 5 | Unsupported/pending | RDP/CredUI disabled; no safe credential input evidence |
| 6 | Not started | No fuzzing/advisory/interoperability/security audit |
| 7 | Not started | No release/signing/store/server actions |

## Validation

| Check | Actual local outcome |
|---|---|
| cargo generate-lockfile --offline | PASS, no third-party dependencies |
| cargo test --workspace --locked --offline | PASS, 11 unit tests, 0 failures; 0 doc tests |
| cargo fmt --all -- --check | PASS |
| cargo clippy --workspace --all-targets --locked --offline -- -D warnings | PASS |
| apps/windows/build-probe.ps1 | PASS, MSVC C++20 /W4 /WX /MT; portable research EXE built |
| windows-probe.exe --policy | PASS, real-secret transfer disabled and RDP unsupported |
| windows-probe.exe --inventory | INCOMPLETE, diagnostic-only restricted process; native exit 2 |
| windows-probe.exe --self-input | BLOCKED safely by integrity guard; no input performed |
| GitHub-hosted clean-source CI | PASS, run 41760663274: fmt, clippy, 11 model tests, 9 memory-gate cases |
| Private-memory identity gate | PASS, 9 mocked cases; actual owner allow and unavailable-auth deny verified |
| Real devices / crypto / interoperability / fuzzing | NOT RUN |

Inventory observed PROCESS_INTEGRITY_RID=4096 (Low), BLE LE and Central capability
reported, and TPM provider opened. This proves only access to those queries in
this restricted execution context, not normal-user medium-integrity feasibility,
active radio, GATT exchange, TPM 2.0, key protection or safe input. The input probe
requires Medium integrity and deliberately refuses this environment. No guard
was weakened to allow synthetic input at Low integrity. A user-run normal desktop
test is still required; --self-input can briefly take focus and is opt-in only.

Tests not explicitly recorded as run are NOT RUN. Synthetic model success is not
cryptographic evidence. No production memory zeroization has been implemented.

## Next gates and limitations

1. Run the built probe on a normal medium-integrity desktop and record results;
   perform opt-in own-window synthetic input only in a suitable test environment.
2. Real foreground mobile peripheral + Windows Central GATT exchange including
   MTU/disconnect/permission/radio-off tests; identify adapter/OS configurations.
3. Verify input safety separately for ordinary windows, CredUI and mstsc. Current
   official restrictions prevent claiming supported non-elevated credential input.
4. Resolve audited/reviewed SPAKE2 suite and memory lifecycle, then freeze vectors.
5. Only after Phase 0 exit, implement security core and native hardware adapters.

No real credentials have been used. No production memory-clearing guarantees,
hardware attestation, secure target input, interoperability or full offline
integrity have been established. Private handoff access instructions are in AI_MEMORY.md.
