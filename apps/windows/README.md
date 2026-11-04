# Windows Phase 0 feasibility probe

C++20/C++/WinRT standalone EXE; static MSVC runtime, `asInvoker`, `uiAccess=false`.
Rejects non-medium-integrity input tests. Read-only inventory can run at other
integrities but reports diagnostic-only and exits 2. No installer, service, driver, elevation,
clipboard, vault, PAKE, arbitrary payload or arbitrary target input.

From repository root:

```powershell
.\apps\windows\build-probe.ps1
.\target\phase0\windows-probe.exe --policy
.\target\phase0\windows-probe.exe --inventory
```

Inventory queries default adapter/LE/central capability and opens/closes the TPM
provider. It does not scan devices, pair, generate keys, prove TPM 2.0, test GATT
or claim a working transfer. Exit 2 means unavailable/blocked/incomplete.

Optional interactive synthetic test, after saving your work:

```powershell
.\target\phase0\windows-probe.exe --self-input
```

This briefly opens its own edit control and attempts only a fixed public literal
using Unicode SendInput. It checks foreground/focus and readback, clears/closes
its window, and reports failure if delivery cannot be verified. It never targets
mstsc or Credential UI. A focus check cannot eliminate the check/use race; a
synthetic character could reach a different window if focus changes concurrently.
Do not run alongside sensitive input. Success applies only to the owned test
control and cannot enable real-secret transfer. Manual execution is separate
from noninteractive CI; current run evidence is in docs/IMPLEMENTATION_STATUS.md.
