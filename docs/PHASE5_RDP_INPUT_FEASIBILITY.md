# Phase 5 start: RDP and target-input feasibility (evidence first)

Status: research and test plan only, 2026-11-06. No new code, no RDP session and no credential UI was touched.
Policy is unchanged (AGENTS.md, D003, D012): RDP, CredUI, the secure desktop and every non-owned target stay
UNSUPPORTED and disabled; real-secret transfer stays disabled. Synthetic data only.

## 1. What the official Microsoft sources say

Fetched 2026-11-06 from Microsoft Learn (dates are the pages' own `updated_at`):

- SendInput, https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput (updated 2025-07-01):
  - Returns the number of events inserted. Zero means input was already blocked by another thread.
  - The function fails when blocked by UIPI, and neither GetLastError nor the return value says UIPI was the cause.
  - Subject to UIPI: input may be injected only into applications at an equal or lesser integrity level.
  - Events are inserted serially and not interleaved with other input, but already-pressed keys can interfere
    (the function does not reset keyboard state).
  - Nothing here speaks of delivery: a return value means "inserted into the input stream", not "received by a control".
- KEYBDINPUT, https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput (updated 2024-02-22):
  KEYEVENTF_UNICODE sends VK_PACKET to the FOREGROUND thread's queue; the receiving app must run TranslateMessage to get
  WM_CHAR. Scan-code mode (KEYEVENTF_SCANCODE) simulates a physical key and depends on the target's layout.
- SetForegroundWindow, https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow
  (updated 2025-10-07): a process may take the foreground only under listed conditions (it is the foreground process, was
  started by it, received the last input event, timeout expired, and so on); it can be denied even then; an app cannot force
  a window to the foreground while the user works in another window (the taskbar button flashes instead).
- UI Automation Value pattern, https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingvalue
  (updated 2025-07-16): SetValue should be called only when IsEnabled is true and IsReadOnly is false; the pattern is an
  optional capability of the target control, not a guarantee. The page does not discuss password fields or cross-integrity access.
- UAC how it works, https://learn.microsoft.com/en-us/windows/security/application-security/application-control/user-account-control/how-it-works
  (updated 2026-10-08): elevation prompts go to the secure desktop by default and only Windows processes can access it;
  lower-integrity applications cannot modify higher-integrity applications; clipboard paste is not possible on the secure desktop.
- RDP input modes, https://learn.microsoft.com/en-us/azure/virtual-desktop/redirection-remote-desktop-protocol (seen as a search
  snippet 2026-11-06; body not read in full): Unicode mode forwards characters, scancode mode forwards physical key events;
  Microsoft lists targets that reject Unicode input. This concerns the RDP client's redirection of a user's real keyboard,
  not injected input.
- Not found: no Microsoft document describes SendInput or UI Automation reaching mstsc.exe's connection window or its
  credential prompt (CredUI). That absence is recorded as "unproven", not as "unsupported by Microsoft".

## 2. Findings table

| # | Target (normal user, medium integrity, no elevation, Rust receiver) | Status | Basis |
|---|---|---|---|
| a1 | Own window, ordinary edit control, same desktop, foreground, SendInput Unicode | SUPPORTED (narrow) | Rust `input-probe`: 56/56 events and exact 28-character readback with the owner's click, 2026-11-06 (IMPLEMENTATION_STATUS). Proves only this path |
| a2 | Own window, UIA ValuePattern.SetValue | UNPROVEN | Documented API, never run here; only a harness candidate |
| a3 | Another ordinary app's edit at the same integrity | UNPROVEN | UIPI allows it in principle; delivery depends on the target (VK_PACKET handling, custom-drawn controls); never tested; outside D003 scope |
| b1 | mstsc.exe connection window / CredUI credential prompt via SendInput | UNPROVEN, treated as UNSUPPORTED | No Microsoft statement; community reports of lost Unicode packets are not official; no test RDP server evidence |
| b2 | Reading back what reached the remote session or a password field | IMPOSSIBLE by design | Password edit controls do not expose their text, and the remote desktop is an opaque stream (section 3.2) |
| b3 | Proof of RDP login success | UNOBSERVABLE from the receiver | Only a separate test RDP server could log the outcome, for a throwaway synthetic account |
| c1 | UAC prompt, lock screen, Winlogon, secure desktop | UNSUPPORTED | Only Windows processes reach the secure desktop; bypasses (UIAccess signing, services, drivers, SYSTEM) are forbidden by AGENTS.md |
| c2 | Elevated (high integrity) windows | UNSUPPORTED | UIPI drops input silently and the return value does not reveal it; elevation is forbidden |
| d1 | Focus changes between check and send | UNSOLVABLE as a guarantee | SendInput goes to whatever is foreground at delivery; checks narrow the race but cannot remove it (THREAT_MODEL); fail closed on any observed change |
| d2 | Receiver bringing a target to the foreground itself | UNRELIABLE | SetForegroundWindow is restricted and can be denied; the 2026-11-06 C++ run in section 4 shows this |
| d3 | Queued keystrokes as evidence of delivery or login | NEVER valid | The return value means "inserted", not "received"; UIPI failures look like success |

## 3. Test plan (synthetic only)

Rules for every test: synthetic strings such as `KL-synth-0123456789`; never a real password; no clipboard; no elevation,
UIAccess, services, drivers or secure-desktop access; no real host or account. A test that cannot meet these is not run.

### 3.1 Owned control (own-window harness, medium integrity)
1. `whoami /groups` must show Medium Mandatory Level, otherwise stop (the C++ probe also refuses).
2. Run `kagilink-input-probe --send-to-own-window` (Rust) and click its window within 20 s; or run
   `kagilink-phase0-probe.exe --self-input` (C++; it needs the same user click to get the foreground, see section 4).
3. Repeat 30 times, including runs right after another window was active and with a modifier key held, to measure the foreground race.
4. Optional separate PR: read the same control through UIA (ValuePattern.Value) and compare with GetWindowText.

Pass: every run shows a complete event count (2 x characters) AND exact string equality of the readback AND the window
foreground and focused before the first and after the last event. Any shortfall, mismatch, focus change or timeout is FAIL and the
text is cleared. The result supports one narrow claim, "own edit control", and nothing about other targets.

### 3.2 mstsc.exe connection window and credential prompt (research only)
Readback and login proof are impossible without a separate test RDP server: the credential prompt's password field cannot be
read back and the remote session is a graphics/virtual-channel stream the receiver cannot inspect.
1. Never test against a real host or account. Use only a disposable VM or test server on an isolated network with a throwaway
   account whose password is synthetic, and only after owner approval of the setup.
2. The only observable outcomes live on the test server (a logon event or a harness that records success/failure for the throwaway
   account). Collect them out of band; the receiver must not claim them.
3. Per run: start mstsc to the test server, wait for the credential prompt, send the synthetic string (Unicode, then scan-code,
   as separate series) and compare with the server-side record.

Pass for any future "supported" claim: at least 50 consecutive runs with zero dropped or misdirected characters, server-side evidence
every time, behaviour documented for NLA, Unicode versus scan-code, keyboard layouts and DPI, and a clean negative control
(deliberately wrong synthetic input must fail). Until then: unsupported.

### 3.3 Negative controls (one PC)
- UIPI is not re-measured (that would need an elevated window, which is forbidden); it stays documented from Microsoft.
- Focus loss: start the harness and switch to a second owned harness window during the send; expect FAIL and no text in the
  other window (never a third-party app).
- Foreground denial: run without the user click; expect refusal and zero events (current behaviour).

## 4. Result of the existing C++ probe `--self-input`

Run on the lead PC at Medium integrity (`PROCESS_INTEGRITY_RID=8192`), 2026-11-06, using the prebuilt
`target/phase0/kagilink-phase0-probe.exe` (built 2026-11-04, SHA-256
B84758F5A6C8EF5CC5D97ABD6ED72D28E9A088EF5CFC254144C18E1262044445, not rebuilt, may predate later source edits), started from an
agent session with no user click:

- Output: `OWNED_EDIT_SYNTHETIC_FAILED_OR_FOCUS_CHANGED`, then the fixed `NO_CREDENTIAL_UI_OR_RDP_SUPPORT_PROVEN` and
  `FOCUS_RACE_NOT_ELIMINATED` lines.
- Interpretation: the program did not get foreground and focus (documented foreground restrictions), so by its own guard no
  keystroke was sent. This is FAIL/INCONCLUSIVE, not a delivery result; the exit code was not captured. It does not contradict the
  Rust probe's 56/56 result, which needed the owner's click. No input to any other window occurred.

## 5. Fail-closed rules for any future input implementation

1. Input is allowed only to a target class with a passing section 3 result; otherwise refuse and say so.
2. Verify foreground window, focus and target process identity immediately before each chunk; abort and wipe on any change;
   no automatic retry that re-types a secret (PROTOCOL.md: ACK loss must not retry input).
3. SendInput's count is recorded only as "input attempted". "Input verified" needs independent readback of a non-password owned
   control. Outcomes for password fields are UNKNOWN.
4. Refuse when the foreground is elevated, on the secure desktop, or cannot be determined.
5. No clipboard, UIAccess, elevation, helper service, driver, credential provider or secure-desktop access; secrets only in
   transient zeroized buffers; nothing logged.
6. Per-transfer approval and the D023 risk notice still apply; the UI never says input succeeded for RDP.

## 6. Proposed decision (not adopted)

See D029 (proposed) in docs/DECISIONS.md: keep RDP/CredUI/secure-desktop input unsupported; allow only an owned-control harness
for evidence; promotion of any other target needs the section 3.2 evidence and a new owner decision.

## 7. Running the study (test-only tooling)

Tools (not part of the product receiver, not shipped): `tools/rdp-input-harness.ps1` (host) and `tools/rdp-test-vm-evidence.ps1`
(inside the test VM). The harness works in Windows PowerShell 5.1 and pwsh 7, needs no administrator rights and refuses to run
elevated. Target must be an IPv4 literal in 192.168.77.0/24 that also exists on a local interface. D012 is unchanged: no real
secret, only the throwaway account's synthetic string, which the owner types into Read-Host (never echoed, logged, stored or
passed on a command line; the agent never sees it). The harness exposes the string only to SendInput.

Owner checklist before each series:
- Host unlocked at the console, a normal (non-admin) PowerShell, no Jump Desktop or other remote session connected, no other
  mstsc.exe running, Caps Lock off, no modifier key held.
- Do not touch keyboard or mouse during runs. Power, sleep and lock settings are the owner's to change; the tools change none.
- In `mstsc` once beforehand: no saved credentials for the target, the certificate warning accepted/"don't ask again", and
  the prompt must open with the throwaway user name filled in and the cursor in the password field (otherwise the run
  fails closed with `PasswordEditNotFocused` and nothing is typed).
- VM: run `tools\rdp-test-vm-evidence.ps1` once (prints the change, asks y/n); compare VM and host UTC clocks (both tools print
  them). Make sure the throwaway account is not locked out by the failed negative-control logons.

Commands (host, one series per mode; calibration first with the default 5 runs):
```
pwsh -NoProfile -File tools\rdp-input-harness.ps1 -Mode Unicode                              # calibration, 5 runs, 1 negative
pwsh -NoProfile -File tools\rdp-input-harness.ps1 -Mode Unicode  -Runs 50 -NegativeEvery 0   # 50 positive runs
pwsh -NoProfile -File tools\rdp-input-harness.ps1 -Mode Unicode  -Runs 10 -NegativeOnly      # negative controls
pwsh -NoProfile -File tools\rdp-input-harness.ps1 -Mode ScanCode -Runs 50 -NegativeEvery 0
pwsh -NoProfile -File tools\rdp-input-harness.ps1 -Mode ScanCode -Runs 10 -NegativeOnly
```
Inside the VM (elevated PowerShell; copy the harness CSV there, it contains no characters of any string):
```
.\rdp-test-vm-evidence.ps1                                   # enable logon auditing (y/n)
.\rdp-test-vm-evidence.ps1 -Report -Minutes 120 -RunsFile .\rdp-input-harness-report-<UTC>.csv
```
Comparison rule (implemented in `-Report`): each Positive run matches exactly one counted 4624 (default logon type 10) within 30 s
of its UTC time and no 4625; each NegativeControl exactly one 4625 and no counted 4624; runs that sent nothing, no event; no
event from the host outside run windows. Any mismatch is FAIL. Type 3 (NLA) 4624 events are listed as ignored, not hidden; the
calibration run must confirm which pattern the VM produces before the counted types are trusted.

Requirement: at least 50 consecutive Positive runs per mode with a matching 4624 each time, plus clean negative controls, as
in section 3.2. NO "supported" claim is made from fewer runs; the harness's `InputAttempted` is SendInput's accepted count only.

Expected risks to measure (not defects): the prompt may drop or reorder Unicode events; scan-code input depends on the
keyboard layout; DPI/NLA/credential-provider behaviour may change focus or hosting (then the prompt is not recognised and the
run fails closed); failed runs are recorded, never retried automatically. The report CSV is ignored by Git; do not commit it.
