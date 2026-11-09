# Own RDP client: dependency review, DRAFT (no decision)

Status: research note for the "own RDP client" approach listed in docs/HID_TYPIST_DESIGN.md section 8. Nothing is selected,
nothing is built, D012 and D029 are unchanged. Source: one search-based pass by a small model on 2026-11-09. Every item marked
UNVERIFIED must be checked against official sources (releases, advisories, docs.rs, source) before any selection (AGENTS.md
Workflow 2).

## Why look at this

A KagiLink-owned RDP client could take the secret from the phone and give it to the remote side through the RDP protocol
(Unicode keyboard events, NLA/CredSSP credentials) instead of simulated local key presses, which Phase 5 showed cannot reach the
Windows 11 credential prompt. It would be a new component with its own attack surface, so the library choice matters first.

## Candidates

| Item | IronRDP (Devolutions, Rust) | FreeRDP (C) |
|---|---|---|
| License | MIT or Apache-2.0 | Apache-2.0 |
| Latest release / date | UNVERIFIED | Sources conflict on the 3.29.0 date (Jul or Aug 2026); 3.30 reported by one source only. UNVERIFIED |
| Commit activity, last 12 months | UNVERIFIED (about 2,260 commits in total) | UNVERIFIED (about 26,800 commits in total) |
| Backing | Devolutions (company) | FreeRDP project, community. UNVERIFIED beyond that |
| Security advisories | None found by search. UNVERIFIED | Many in 2026 (several releases described as security-focused). Check the official advisory list |
| NLA / CredSSP | Documented: NLA via CredSSP with NTLM and Kerberos; legacy standard RDP security not supported | nla.c exists in source; not confirmed from the README. UNVERIFIED |
| TLS | 1.2 and 1.3 documented | UNVERIFIED |
| Unicode keyboard events | Input operations for Unicode key press/release exist in ironrdp-input; end-to-end behaviour UNVERIFIED | `freerdp_input_send_unicode_keyboard_event` exists; code unit vs code point handling UNVERIFIED |
| Windows | Native clients on Windows; NASM needed for Windows builds | Native MSVC CMake build. UNVERIFIED on official wiki |
| Password through API, not command line | UNVERIFIED (expected as a connector config field) | UNVERIFIED (expected via the settings API) |
| Third-party audit | None found | None found (CodeQL, Coverity, OSS-Fuzz configuration present) |

## Reading of the evidence

* FreeRDP is mature but is a large C code base with frequent memory-safety advisories; any use would need a pinned, fixed
  version and a fast update path.
* IronRDP is Rust (memory-safety benefit), dual licensed, with NLA and modern TLS documented; its Unicode input on the wire and
  its credential API need a source check.
* Neither has a known third-party audit. Both would sit in a process that handles a real credential, so the same rules apply:
  zeroize, no logging of values, no command-line passwords, transient memory only.
* NLA success is a protocol result, not proof that the remote desktop is trustworthy. The warnings of docs/RISK_NOTICE.md still
  apply.

## Checks before any selection

1. IronRDP releases, last-12-month commit activity and GitHub advisories (official pages).
2. FreeRDP release dates and the official advisory list; the first fixed version for each advisory that matters.
3. IronRDP connector configuration (docs.rs / source): how credentials are passed and whether they can be zeroized.
4. A synthetic end-to-end test against a throwaway account on an authorized target only: NLA success, Unicode events including
   a password with symbols, failure paths (wrong password, disconnect, certificate change).
5. Whether the client may run as a normal user with no driver or service, as the Windows receiver does (D022).

## Not decided

Whether to build an own RDP client at all; which library; whether it needs a protocol change in docs/PROTOCOL.md (it would
need separate approval). Real-secret transfer stays disabled (D012).
