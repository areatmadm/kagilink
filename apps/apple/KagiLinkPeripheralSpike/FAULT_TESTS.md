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
| `garbageM2` | sends 40 garbage bytes instead of M2 | handshake fails before M3: failure count 0 |
| `delayM2` | holds M2 for 6 s (limit is 5 s) | timeout, no answer: failure count 0 |
| `tamperM4` | flips one bit of M4 | M3 was sent, then a bad M4: failure count 1 |
| `dropAfterM3` | after M3 sends no M4, answers the write with an error and removes the service/advertising | M3 sent, attempt abandoned: failure count 1 |
| `dupChunk` | M2 in 60-byte chunks, first chunk sent twice | chunk error: connection closed |
| `skipChunk` | M2 in 60-byte chunks, second chunk dropped | chunk error: connection closed |
| `reorderChunk` | M2 in 60-byte chunks, chunks 2 and 3 swapped | chunk error: connection closed |
| `tamperSecret` | flips one bit of the first SECRET fragment | channel closed, nothing delivered |

Limits: an iOS peripheral cannot drop the central's link, so `dropAfterM3` answers with an error and stops
advertising; the PC decides whether it disconnects. The mode `none` is the normal exchange.
