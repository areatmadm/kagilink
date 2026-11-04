# KagiLink Windows Phase 0 diagnostic GUI and CLI

C++20/C++/WinRT standalone EXE; static MSVC runtime, `asInvoker`, `uiAccess=false`.
Rejects non-medium-integrity input tests. Read-only inventory can run at other
integrities but reports diagnostic-only and exits 2. No installer, service, driver, elevation,
clipboard, vault, PAKE, arbitrary payload or arbitrary target input.

## Diagnostic window

Build with `apps/windows/build-probe.ps1`, then double-click
`target/phase0/kagilink.exe`. Keep `kagilink-phase0-probe.exe` beside it. The GUI
opens a persistent native Win32 window without a console and starts no diagnostic
automatically. It is a Phase 0 research interface, not a credential receiver product.

- `어댑터 확인` runs the read-only BLE/TPM inventory and shows its result.
- `BLE 연결 시험` explicitly starts the separate fixed-public-value GATT test.
- `중지` stops the current diagnostic. Closing the window also stops its child.

The window uses the same Glass visual language as the Android/Apple Phase 0 screens:
luminous teal backdrop, frosted dark panels with a reflected rim and pill glass
controls, drawn with GDI+ (no extra runtime dependency). It switches to flat system
colors in Windows high-contrast mode. The look is cosmetic; behaviour is unchanged.

The window presents Korean status and a read-only public diagnostic transcript.
`상세 결과 보기` toggles the diagnostic-code area. The visible GUI inventory,
start/busy/stop/restart and no-sender flows have been exercised; see implementation
status for the exact evidence and remaining DPI/accessibility/device tests.
It has no password/code field, vault, pairing, arbitrary command or input-injection
control. It can start only the fixed sibling probe with `--inventory` or `--ble-test`;
the CLI's `--self-input` experiment is deliberately not exposed in the GUI.
The original CLI and its Medium-integrity BLE/input guards are preserved.

The GUI polls its child without blocking the message loop, bounds captured output
to 16 KiB and supervises it with a 70-second outer limit. The child's BLE deadline
remains 60 seconds; the supervisor does not extend it. Cancel/close uses a
kill-on-close job. Physical radio/ATT cancellation timing still needs device tests.
No result is written to a log file. Current build, direct-window and screenshot
evidence is recorded in [implementation status](../../docs/IMPLEMENTATION_STATUS.md).

## Command-line probe

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
