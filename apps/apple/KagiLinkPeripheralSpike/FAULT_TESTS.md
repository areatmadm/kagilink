# KL-V1 peripheral spike — fault-injection checklist (synthetic data only)

Test-only modes for checking how the Windows receiver reacts to a misbehaving phone. They are compiled only
when the app is built with `FAULT_INJECTION=1`; the normal build contains none of this code. Follow this file
top to bottom. Tick each box when done and write down what the PC printed and the phone's `FAULT` log lines.

Legend for "PC expected": **seen** = this exact result was already observed on 2026-11-06; **predicted** =
derived from the receiver design, wording not yet observed (record what is really printed).

## 0. Before you start

- [ ] Install the fault build (needs the user's OK; existing development certificate and existing wildcard
      profile only, the three variables are required and no personal identifiers go into the repository, D028):

      ```sh
      DEVICE_UDID=<udid> TEAM_ID=<team id> IDENTITY="<codesign identity>" FAULT_INJECTION=1 sh apps/apple/KagiLinkPeripheralSpike/build-and-install.sh
      ```
- [ ] iPhone unlocked, screen on, the app in the foreground for the whole test (it advertises only in the foreground).
- [ ] The app shows the red "FAULT TEST BUILD" menu. PC: latest `main` built (`areatm-kagilink-receiver.exe`).

## Steps used in every run

A. PC: `target\debug\areatm-kagilink-receiver.exe ble-receive`, type `YES`, read the 6-digit code.
B. Phone: pick the mode in the red menu, type the code, press **Start**. Within 60 s of the code appearing.
C. PC: choose the list row whose `KL-XXXX` tag matches the phone.
D. Note the PC result line, the line `Failure budget used: n/3`, and the phone log lines `FAULT SESSION start=<UTC>
   codeHash4=<4 hex>` and `FAULT RESULT mode=...`. The hash is the first 4 hex characters of SHA-256 of the
   six code digits (e.g. `python -c "import hashlib;print(hashlib.sha256(b'123456').hexdigest()[:4])"`), so a
   run can be matched with the PC's code without writing the code down; the phone UTC time is for matching the
   PC log time. (4 hex characters narrow a six-digit code to about 15 candidates: fine for these synthetic,
   60-second codes, never for a real flow.)

## 1. `tamperM4` three times with the SAME code (failure budget 1/3 -> 2/3 -> 3/3, code destroyed)

Everything below must finish inside the 60 s code lifetime: rehearse once. The phone UI clears the field after
Start, so type the same code again each time (a new tag appears each time).

- [ ] Run A-C with mode `tamperM4`. PC expected: `Attempt failed: Core(ConfirmationFailed)` (**seen**), then
      `Failure budget used: 1/3` (**predicted**), then a prompt to retry with the same code.
- [ ] Answer `y`. Phone: same code, Start (mode still `tamperM4`), pick the new tag. PC expected:
      `Core(ConfirmationFailed)` and `Failure budget used: 2/3` (**predicted**).
- [ ] Answer `y` again, repeat. PC expected: `Failure budget used: 3/3` and the code destroyed, no retry offered
      (**predicted**, exact wording not yet observed). A new `ble-receive` run gives a new code.
- [ ] If the 60 s code lifetime ends first, the PC reports expiry instead of a count: start over.

## 2. `delayM2Long` (M2 held 11 s, longer than the receiver's 10 s first-chunk wait)

- [ ] Run A-C with mode `delayM2Long`. PC expected: the attempt fails by timeout and `Failure budget used: 0/3`
      (**predicted**; error wording not yet observed). The phone logs `FAULT delayM2Long: holding M2 for 11 s`.
      (Mode `delayM2`, 6 s, is accepted by design and ends in Transfer OK: **seen**.)

## 3. `garbageM2` (40 garbage bytes instead of M2)

- [ ] Run A-C with mode `garbageM2`. PC expected: `Attempt failed: Core(WrongState)` (**seen**),
      `Failure budget used: 0/3` (**predicted**, fails before M3).

## 4. `dropAfterM3` (no M4; error response, service removed, advertising stopped)

- [ ] Run A-C with mode `dropAfterM3`. PC expected: `Attempt failed: Link(Disconnected)` (**seen**),
      `Failure budget used: 1/3` (**predicted**, M3 was sent and the attempt abandoned).

## 5. Chunk faults (M2 sent as four 60-byte chunks)

- [ ] `dupChunk` (first chunk twice). PC expected: `Attempt failed: Core(Framing)` (**seen**).
- [ ] `skipChunk` (second chunk dropped). PC expected: `Core(Framing)` (**seen**).
- [ ] `reorderChunk` (chunks 2 and 3 swapped). PC expected: `Core(Framing)` (**seen**).
      For all three the budget line is **predicted** `0/3` (error before M3): record what is printed.

## 6. `tamperSecret` (one bit of the first SECRET fragment flipped)

- [ ] Run A-C with mode `tamperSecret`. PC expected: `Attempt failed: Core(Authentication)` (**seen**), no
      `Transfer OK`, nothing delivered. Record the `Failure budget used` line as printed (not predicted).

## 7. Sanity run and restore

- [ ] Optional: mode `none`. PC expected: `Transfer OK: 33 bytes received in 1 fragments` (**seen**).
- [ ] Restore the normal build (no fault modes on the phone) with the same install command **without**
      `FAULT_INJECTION=1`; confirm the red "FAULT TEST BUILD" menu is gone.

## Reference: modes

| Mode | What the phone does |
|---|---|
| `none` | normal exchange |
| `garbageM2` | sends 40 garbage bytes instead of M2 |
| `delayM2` / `delayM2Long` | holds M2 for 6 s / 11 s (receiver waits up to 10 s for a first chunk) |
| `tamperM4` | flips one bit of M4 |
| `dropAfterM3` | after M3 sends no M4, answers the write with an error, removes the service and stops advertising |
| `dupChunk` / `skipChunk` / `reorderChunk` | M2 in 60-byte chunks: first chunk twice / second dropped / chunks 2 and 3 swapped |
| `tamperSecret` | flips one bit of the first SECRET fragment |

Limits: an iOS peripheral cannot drop the central's link, so `dropAfterM3` answers with an error and stops
advertising; the PC decides whether it disconnects. When the central disconnects, the app sees it as
`central unsubscribed` and ends the run.
