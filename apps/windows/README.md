# KagiLink Windows Phase 0 feasibility probe

C++20/C++/WinRT standalone EXE; static MSVC runtime, `asInvoker`, `uiAccess=false`.
Rejects non-medium-integrity input tests. Read-only inventory can run at other
integrities but reports diagnostic-only and exits 2. No installer, service, driver, elevation,
clipboard, vault, PAKE, arbitrary payload or arbitrary target input.

`--ble-test` is a separate opt-in, unauthenticated public diagnostic GATT exchange,
not production transport; see [cross-OS instructions](../../docs/PHASE0_BLE_PROBE.md).
With exactly one foreground Android/Apple probe advertising, run from a normal
Medium-integrity desktop:

```powershell
.\target\phase0\kagilink-phase0-probe.exe --ble-test
```

This scans a diagnostic service for 10 seconds, refuses ambiguous observed senders,
then writes and reads one fixed 16-byte public value within a 60-second deadline.
It prints no peer names/addresses or received bytes. Exit 0 means only exact
synthetic GATT response verification; exit 2 means blocked/failed/incomplete.
No pairing, retry, arbitrary payload, credential target or hardware trust exists.
GATT access from this unpackaged console may fail on consent/packaging requirements;
report that result instead of changing integrity or system protections.

From repository root:

```powershell
.\apps\windows\build-probe.ps1
.\target\phase0\kagilink-phase0-probe.exe --policy
.\target\phase0\kagilink-phase0-probe.exe --inventory
.\target\phase0\kagilink-phase0-probe.exe --version
.\target\phase0\kagilink-phase0-probe.exe --help
```

Inventory queries default adapter/LE/central capability and opens/closes the TPM
provider. It does not scan devices, pair, generate keys, prove TPM 2.0, test GATT
or claim a working transfer. Exit 2 means unavailable/blocked/incomplete.

Optional interactive synthetic test, after saving your work:

```powershell
.\target\phase0\kagilink-phase0-probe.exe --self-input
```

This briefly opens its own edit control and attempts only a fixed public literal
using Unicode SendInput. It checks foreground/focus and readback, clears/closes
its window, and reports failure if delivery cannot be verified. It never targets
mstsc or Credential UI. A focus check cannot eliminate the check/use race; a
synthetic character could reach a different window if focus changes concurrently.
Do not run alongside sensitive input. Success applies only to the owned test
control and cannot enable real-secret transfer. Manual execution is separate
from noninteractive CI; current run evidence is in docs/IMPLEMENTATION_STATUS.md.
