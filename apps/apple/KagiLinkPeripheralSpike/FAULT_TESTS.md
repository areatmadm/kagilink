# KL-V1 peripheral spike — fault-injection test procedure (synthetic data only)

Test-only modes for checking how the Windows receiver reacts to a misbehaving phone. They are compiled
only when the app is built with `FAULT_INJECTION=1`; the normal build contains none of this code.

```sh
DEVICE_UDID=<udid> TEAM_ID=<team id> IDENTITY="<codesign identity>" FAULT_INJECTION=1 sh build-and-install.sh
```

Existing development certificate and existing wildcard profile only; the three variables are required
and no personal identifiers belong in the repository (D028). Install the normal build again afterwards
(same command without `FAULT_INJECTION`) so the phone is not left with the fault modes.

## Per run (repeat once per mode; every run needs a fresh code, it is valid for 60 s)

1. Phone: open the app (keep it in the foreground), choose the mode in the red "FAULT TEST BUILD" menu.
2. PC: run `target\debug\areatm-kagilink-receiver.exe ble-receive`, type `YES`, read the 6-digit code.
3. Phone: type the code, press Start; note the `KL-XXXX` tag on the phone.
4. PC: pick the list row whose tag matches the phone.
5. Record the PC output (result text and failure count, if printed) and the phone log lines starting with
   `FAULT` (including `FAULT RESULT mode=...`).

## Modes and the expected receiver result (from the Windows fake-phone tests)

| Mode | What the phone does | Expected on the PC |
|---|---|---|
| `garbageM2` | sends 40 garbage bytes instead of M2 | handshake fails before M3: `Failure budget used: 0/3` |
| `delayM2` | holds M2 for 6 s | accepted by design (the receiver waits up to 10 s for a first chunk): Transfer OK |
| `delayM2Long` | holds M2 for 11 s | first-chunk wait times out, attempt fails: failure count 0 |
| `tamperM4` | flips one bit of M4 | M3 was sent, then a bad M4: `Failure budget used: 1/3` |
| `dropAfterM3` | after M3 sends no M4, answers the write with an error and removes the service/advertising | M3 sent, attempt abandoned: `Failure budget used: 1/3` |
| `dupChunk` | M2 in 60-byte chunks, first chunk sent twice | chunk error: connection closed |
| `skipChunk` | M2 in 60-byte chunks, second chunk dropped | chunk error: connection closed |
| `reorderChunk` | M2 in 60-byte chunks, chunks 2 and 3 swapped | chunk error: connection closed |
| `tamperSecret` | flips one bit of the first SECRET fragment | channel closed, nothing delivered |

## Same-code repetition (failure budget 1/3 -> 2/3 -> 3/3 and code destroyed)

Uses `tamperM4`, which fails after M3, so every attempt costs one failure. The receiver prints
`Failure budget used: n/3` after each attempt and asks whether to retry with the same code; everything must
finish inside the 60 s code lifetime, so rehearse once first.

1. Phone: choose `tamperM4`. PC: start `ble-receive`, `YES`, read the code.
2. Phone: type the code, Start, pick the tag on the PC. Expect `Attempt failed: Core(ConfirmationFailed)` and `Failure budget used: 1/3`.
3. PC: answer `y` to retry with the same code. Phone: type the SAME code again and press Start (a new tag appears), pick it on the PC. Expect `2/3`.
4. Repeat once more. Expect `3/3` and the code destroyed (no further retry; a new `ble-receive` run gives a new code).
5. If the 60 s lifetime ends first, the receiver reports expiry instead of a count: start over.

Limits: an iOS peripheral cannot drop the central's link, so `dropAfterM3` answers with an error and stops
advertising; the PC decides whether it disconnects. The mode `none` is the normal exchange.
