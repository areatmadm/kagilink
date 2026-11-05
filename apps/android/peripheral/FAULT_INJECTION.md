# KL-V1 peripheral spike: test-only fault injection

Synthetic data only. The spinner "TEST ONLY fault injection" exists only in debuggable builds
(this spike module is never a release build). Default is `none`; the selection resets to `none`
after every run (the session also ignores a fault request when the app is not debuggable). Each
injection is logged under logcat tag `KLV1` (`fault injected: ...`) and the final summary line
contains `fault=<NAME>`.

| Mode | What the phone does | Expected receiver result (Windows side) |
|---|---|---|
| a `GARBAGE_M2` | sends random bytes (M2 length) instead of M2 | handshake fails, failure count stays 0 |
| b `TAMPER_M4` | flips one byte of M4 | 1 failure counted (tamper after M3) |
| c `DROP_AFTER_M3` | drops the connection as soon as M3 arrives | 1 failure counted (D026) |
| d1 `CHUNK_DUP` | M2 sent in 32-byte chunks, chunk 1 sent twice | connection closed |
| d2 `CHUNK_DROP` | chunk 1 of M2 never sent | connection closed |
| d3 `CHUNK_SWAP` | chunks 1 and 2 of M2 swapped | connection closed |
| e1 `NO_RESPONSE` | never answers M1 | timeout, failure count stays 0 |
| e2 `DELAY_M2` | answers M1 after 6 s (> 5 s message timer) | timeout, failure count stays 0 |
| f `TAMPER_SECRET` | flips one byte of the SECRET fragment | channel closed, nothing delivered |

Note: for b and f the phone still runs the full handshake first and you must press
"Approve ONE synthetic SECRET transfer" for f. Modes d1..d3 need M2 to span at least 3 chunks;
otherwise the log says `fault ... not injected`.

## Procedure per mode (user)

1. PC: run `target\debug\areatm-kagilink-receiver.exe ble-receive` and note the 6-digit code.
2. Phone: open the app, type the code, pick the fault in the spinner, (optionally tick Notify), Start.
3. PC: choose the phone's KL-XXXX tag from the list. For mode f press "Approve ..." on the phone
   when it becomes enabled.
4. Write down the PC output (result and failure count). The phone log is read with
   `adb logcat -d -s KLV1`.
5. Repeat for each mode; each run is a fresh session with a fresh code and tag.
