# KagiLink threat model — draft

Assets: stored vault ciphertext/key handles, pairing code, PAKE ephemeral state,
session keys, transient plaintext, transfer approval and selected input target.
Trust boundaries: BLE peer, native/core ABI, hardware API, app storage, input API,
and receiving OS. All advertisements and incoming bytes are untrusted.

The Phase 0 native diagnostic is unauthenticated and deliberately exchanges only
fixed public values on isolated UUIDs (PHASE0_BLE_PROBE.md). It can be spoofed,
replayed or interrupted; success is not peer identity, proximity or trust evidence.
It must never accept credentials or forward data into input APIs. Transient routing
identifiers are not logged. The Apple peripheral prototype cannot immediately
observe all disconnects; its public state expires and is not production cleanup.
Liquid Glass surfaces and hardware capability labels are visual information only;
they must never imply verified peer identity, encryption or a protected session.
Reduced transparency, readable contrast and visible terminal errors are required
so styling cannot hide a denial/unsupported state on any supported display size.

| Threat | Required control | Remaining limitation |
|---|---|---|
| BLE eavesdropping/MITM/offline code guessing | Reviewed PAKE, explicit confirmation, bound context | Six digits permit online guesses; receiver-wide throttling needed |
| New-session brute force | 3 attempts/60s plus global backoff and visible restart | Session limit alone is insufficient; backoff design pending |
| Replay/reflection/unknown key share/downgrade | Fresh session/nonces, role/version/device/purpose binding | Display name is not certified identity |
| Relay/proximity deception | User checks peer and code | BLE RSSI/PAKE are not distance bounding |
| Fragment/resource attack | Bounded counts/length/time, strict order, terminal errors | Pre-auth DoS cannot be eliminated |
| Nonce reuse | Per-direction keys/counters, no resume, consume on use | Must test crash/reconnect/overflow |
| Approval bypass | Native fresh auth, narrow one-use capability | Compromised OS can subvert UI |
| Hardware failure/false feature claim | Actual key generate/use and security-level verification | Offline platform integrity cannot be fully proven |
| Focus swap/wrong target | Selected target checks, per-input validation, safe failure | SendInput has a check/use race; ordinary-window success insufficient |
| Compromised receiver | Disclosure to user, minimize lifetime | Malware can see decrypted credentials; unsupported threat guarantee |
| Logs/files/clipboard/dumps | No secret sinks, zeroizing buffers, locking where possible | Swap, copies, crash dumps cannot be ruled out absolutely |
| Dependency/build compromise | Review/pin/advisories/SBOM/provenance/signatures | Audit of an old version does not cover every new release |

No protocol log may include code, secret transcript values, keys or plaintext.
Public session metadata is privacy-sensitive too; prefer local transient diagnostics.
Root/jailbreak indicators are rejection signals, not an attestation system.
