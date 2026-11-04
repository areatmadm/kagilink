# KagiLink requirements and acceptance mapping

Public requirements are summarized here. The original conversation prompt is in
the separate private AI memory repository and is not required for public-source
work. Agent policy is defined only in ../AGENTS.md. This table maps acceptance evidence.

| ID | Requirement | Acceptance evidence / current state |
|---|---|---|
| R01 | Offline BLE, no accounts/remote auth/telemetry | Packet/network observation on real apps; NOT RUN |
| R02 | Six digits, CSPRNG, 60s, max 3 failures | Synthetic TTL/failure model only; CSPRNG not implemented |
| R03 | SPAKE2, mutual confirmation, authenticated context | RFC vectors + wrong code/context tests; NOT IMPLEMENTED |
| R04 | HKDF + AES-256-GCM, independent keys/nonces | Vectors, tamper/replay/exhaustion tests; NOT IMPLEMENTED |
| R05 | Bounded fragments, strict order, disconnect cleanup | Model tests; separate fixed-value native diagnostic sources, actual transport NOT RUN |
| R06 | Approval every transfer, single use, no reuse | Synthetic model only; native approval NOT IMPLEMENTED |
| R07 | iOS/iPadOS hardware/user auth/tamper block | Actual Secure Enclave operations and device matrix; NOT RUN |
| R08 | Android StrongBox required, no fallback | Feature + actual key security level/use, negative devices; NOT RUN |
| R09 | Windows portable normal-user BLE/TPM or labelled RAM | Probe and fixed-value diagnostic client built; normal-user device GATT/key use NOT RUN |
| R10 | macOS sender/receiver, required hardware | Native diagnostic source added; Apple build/hardware/device tests NOT RUN |
| R11 | Transient secrets, zeroize/locking, no leaks | Full error/drop/FFI and memory review; NOT IMPLEMENTED |
| R12 | Direct entry/autofill + encrypted app vault | Registration/use/change/delete/loss tests; NOT IMPLEMENTED |
| R13 | Safe input/RDP, fail closed without bypass | Ordinary owned-window test separate from CredUI/mstsc; pending |
| R14 | Accessible native sender/receiver screens | Diagnostic mobile/Apple UI source only; product UI/accessibility tests NOT RUN |
| R15 | Apache-2.0 public source; signed/provenanced releases | Release checklist; no publication authorized |
| R16 | Common instructions + project-local AI memory | AGENTS.md + shared/ai/ references established |
| R17 | Android 14+/latest stable SDK; iOS/iPadOS 18+/SDK 27.1 for iPhone Duo | Build configuration/source checks; SDK compilation and device support NOT RUN |
| R18 | Liquid Glass mobile UI with accessible adaptive layout | Native Apple glass/material fallback and Android platform styling; visual/device QA NOT RUN |

Code, ephemeral secrets and received passwords may not persist. App vault content
is the explicitly authorized exception: ciphertext with hardware-protected keys.
No arbitrary other-app credential access is permitted.
