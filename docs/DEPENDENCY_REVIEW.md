# KagiLink cryptographic dependency review — preliminary, 2026-11-04

No cryptographic runtime dependencies are selected/installed. The synthetic model
uses std only. No claim of a clean vulnerability scan is made.

| Candidate | Evidence | Decision / remaining gate |
|---|---|---|
| RustCrypto spake2 | [Upstream](https://github.com/RustCrypto/PAKEs), [crate](https://docs.rs/spake2/latest/spake2/) | Apache-2.0/MIT; upstream audit/erasure warnings; not approved for real secrets |
| RustCrypto aes-gcm | [Documentation](https://docs.rs/aes-gcm/latest/aes_gcm/) | Apache-2.0/MIT, historical audit noted; verify selected release, CPU constant-time behavior and zeroization |
| RustCrypto hkdf/sha2/hmac | [HKDF](https://docs.rs/hkdf/latest/hkdf/) | Candidate; exact versions, advisories, target and API review pending |
| zeroize | [Documentation](https://docs.rs/zeroize/latest/zeroize/) | Apache-2.0/MIT; optimizer-resistant erasure, not copy/swap immunity; candidate |
| OS entropy / getrandom | OS CSPRNG or reviewed Rust adapter | Exact dependency and failure behavior pending; no weak fallback |

RustCrypto PAKEs explicitly warns about lack of formal review and secret erasure.
Repository activity and a recognized author do not equal cryptographic audit.
Compare selected SPAKE2 group/encoding/confirmation behavior against RFC 9382
instead of assuming the crate name proves RFC compatibility.

Before adoption: inspect exact release source and recent maintenance, compare
audit scope to that version, query RustSec advisories for the complete locked
dependency graph, check Apache compatibility/NOTICE obligations, test Windows,
Android and Apple targets, inspect secret clones/drop/error paths and freeze
reference vectors. Record version, checksums, evidence and review date here.
Pin direct versions and Cargo.lock; use cargo audit/deny plus fuzzing in Phase 1.
A dependency-free Phase 0 test is not a PAKE or AES security validation.
