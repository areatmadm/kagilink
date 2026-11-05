# KL-V1 peripheral spike: test-only fault injection

Synthetic data only. The fault code is compiled ONLY into the debug build type (source set
`src/debug`); `src/release` has a stub that returns the no-op `NoFaults`, so other build types
contain no injection code or strings (the main source set keeps only the no-op seam
`FaultPlan`/`NoFaults`). `sh check-no-faults-in-release.sh <debug.apk> <release.apk>` checks this
(local check; CI does not build this Gradle module). Default is `none`; the spinner selection
resets to `none` after every run unless "keep fault selected" is ticked. Each injection is logged
under logcat tag `KLV1` (`fault injected: ...`) and the final summary line contains `fault=<NAME>`.

## Matching phone log and PC output

Every run logs `session start utc=<UTC, seconds> tag=KL-XXXX mode=... fault=...`. The PC prints
the same `KL-XXXX` tag in its list, so use the tag (and the time) to pair a PC attempt with a phone
session: `adb logcat -d -s KLV1 | grep "session start"`. The code (or any hash of it) is never
logged: a 4-hex-digit hash of a 6-digit code would narrow it to a handful of candidates.

| Mode | What the phone does | Expected receiver result (Windows side) |
|---|---|---|
| a `GARBAGE_M2` | sends random bytes (M2 length) instead of M2 | handshake fails, failure count stays 0 |
| b `TAMPER_M4` | flips one byte of M4 | 1 failure counted (tamper after M3) |
| c `DROP_AFTER_M3` | drops the connection as soon as M3 arrives | 1 failure counted (D026) |
| d1 `CHUNK_DUP` | M2 sent in 32-byte chunks, chunk 1 sent twice | connection closed |
| d2 `CHUNK_DROP` | chunk 1 of M2 never sent | connection closed |
| d3 `CHUNK_SWAP` | chunks 1 and 2 of M2 swapped | connection closed |
| e1 `NO_RESPONSE` | never answers M1 | timeout, failure count stays 0 |
| e2 `DELAY_M2` | answers M1 after 6 s | must SUCCEED: the receiver waits up to 10 s for the first chunk of a handshake message (FIRST_CHUNK_WAIT) |
| e3 `DELAY_M2_LONG` | answers M1 after 11 s | receiver times out and fails, failure count stays 0 |
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

## Repeating one fault with the same code (failure budget 1 -> 2 -> 3, code discarded)

After the receiver change that prints `Failure budget used: n/3` and asks whether to retry with the
same code (answer `y`), repeat b (`TAMPER_M4`) or c (`DROP_AFTER_M3`) with the SAME code. The code
lives 60 s, so do not switch modes between attempts:

1. Tick "keep fault selected after a run", pick b or c.
2. Type the code, Start, choose the KL-XXXX tag on the PC. The attempt fails; the phone prints its
   summary and Start is enabled again. The PC prints `Failure budget used: 1/3`; answer `y`.
3. On the phone type the same code again (the field is cleared on every Start; the phone wipes the
   code after each attempt), Start, choose the new KL-XXXX tag (each Start uses a fresh tag).
4. Repeat until the PC reports 3/3 and discards the code. A fourth attempt with the old code must
   not be possible; start a fresh `ble-receive` for a new code.
Each attempt is a fresh BLE session; the phone log shows `fault=TAMPER_M4` or `fault=DROP_AFTER_M3`
for every attempt. Attempts in modes a, e1, e2 and e3 must not consume the budget.

## Checklist for the next run (in this order; tick each box)

Before every attempt: type the code carefully (a typo makes the phone fail at M3 with
`sender_finish failed 9` and counts as a failure on the PC), and start the phone within the 60 s
of the code. Pick the mode in the spinner BEFORE typing the code.

- [ ] 1. d1 `CHUNK_DUP` in Notify mode (tick Notify). Expect PC `Core(Framing)`, `Failure budget used: 0/3`.
- [ ] 2. f `TAMPER_SECRET` in Indicate mode (Notify unticked). Press Approve on the phone. Expect PC
      `Core(Authentication)`, budget unchanged, no data delivered.
- [ ] 3. b `TAMPER_M4`, Indicate. Expect a failed attempt with the budget at 1/3 (tamper after M3).
- [ ] 4. b `TAMPER_M4`, Notify. Same expectation, fresh `ble-receive`.
- [ ] 5. Same code three times with b (budget 1 -> 2 -> 3 and discard):
      a. Fresh `ble-receive`, note the code. Tick "keep fault selected", pick b.
      b. Attempt 1: type the code, Start, pick the new tag. Expect `Failure budget used: 1/3`, answer `y`.
      c. Attempt 2: type the SAME code again, Start, pick the new tag. Expect 2/3, answer `y`.
      d. Attempt 3: same again. Expect 3/3 and then the code reported inactive/discarded.
      e. A fourth attempt with that code must not be possible. Do all of this within 60 s, so keep the
         phone ready; if the code expires first, repeat with a fresh code (that is not a failure of the receiver).
- [ ] 6. Optional, c `DROP_AFTER_M3` as in step 5 (budget 1/3 -> 2/3 -> 3/3).
- [ ] 7. Untick "keep fault selected" and run one normal transfer (mode none) to confirm the app is healthy.
- [ ] 8. Give the PC output of every step to the Android session; it pairs it with the phone log by tag/time.
