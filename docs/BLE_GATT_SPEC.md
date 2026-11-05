# KagiLink BLE/GATT mapping, framing v1 (DRAFT, Phase 2)

Status: DRAFT proposed by the lead session on 2026-11-06 (D025); owner decisions of the same day folded in (D026: tag + list selection, Notify fallback, crowded places in scope). Nothing here has been tested
against a real peripheral except the Phase 0 fixed-string exchange (PHASE0_BLE_PROBE.md, which is a
separate contract and MUST NOT become this one). The framing codec in `core/kagilink-core/src/ble_frame.rs`
implements section 5 and has local synthetic tests only. MTU behaviour, indication delivery, disconnect
handling and multi-device cases on real iOS, Android and Windows hardware are NOT verified. Real-secret
transfer stays disabled (D012).

## 1. Roles and goals

* Receiver (desktop, Windows/macOS) = BLE Central. It scans, connects, creates and shows the code, and
  owns the 60 s / three-failure limits (D014, AGENTS.md).
* Sender (phone) = BLE Peripheral. It advertises only after the user has entered the code and pressed
  start. The logical receiver still creates the code; the BLE roles are not reversed.
* All security lives in the application layer (CPace, key confirmation, AES-256-GCM). BLE link-layer
  pairing, bonding and encryption are NOT relied on and NOT required, so no OS pairing prompt appears
  and PCs without pairing rights work. A passive or active BLE attacker is assumed.

## 2. GATT layout (framing v1)

The framing version is carried by the service UUID. A different framing gets a different UUID; there is
no negotiation (matches PROTOCOL.md: one version, exact match). UUIDs below are random draft values.

| Item | UUID | Properties | Direction |
|---|---|---|---|
| Service KL-V1 | `3685355e-2903-4e85-bdd6-474ada0838c1` | primary | |
| RX characteristic | `316da2a3-cfc7-4bbf-a736-58c4e7d8d5cf` | Write (with response) | central -> peripheral |
| TX characteristic | `7c69e9b3-c395-4fc0-87b2-b564d7b8b29e` | Indicate (preferred) or Notify (+ CCCD) | peripheral -> central |

* No Read characteristic and no Device Information: the service exposes nothing before the handshake.
* Permissions: no authentication, encryption or authorization required on any attribute.
* Write Without Response is not used; Write-with-response gives the central a per-chunk ATT
  acknowledgement. TX uses Indicate where the peripheral platform can offer it. Where it cannot,
  TX is Notify and every notified chunk is acknowledged by the central with an application ACK chunk
  (`flags = 0b100`, `seq`, 2 bytes, written to RX; see `ble_frame::ack_chunk`). With Notify the
  peripheral sends the next chunk only after the ACK for the previous one (stop-and-wait) and drops
  the connection if no ACK arrives within 2 s. The central enables whichever of Indicate/Notify the
  TX characteristic offers (Indicate first). ACKs are link-layer plumbing only; they say nothing about
  the application having processed anything (section 7).
* The peripheral ignores and drops writes on RX until the central has enabled indications on TX.

## 3. Advertising, selection and connection

Crowded places (internet cafes, libraries) are in scope (D026): several unrelated phones, some also
running KagiLink, may be in radio range. The design must therefore let the USER pick the right phone and
must not let strangers cheaply burn the receiver's failure budget.

* The sender advertises the 128-bit service UUID and, in the scan response, a Complete Local Name
  `KL-XXXX`, where XXXX is a short TAG: four random characters from the 31-symbol alphabet
  `23456789ABCDEFGHJKMNPQRSTUVWXYZ` (no look-alike characters), drawn fresh from the OS CSPRNG each
  time the user presses start. The tag is NOT derived from the code, the device, the account or time,
  and is never reused, so it cannot be used to guess the code and gives no tracking beyond one
  60 s window. The same tag is shown large on the phone screen. No other advertising data.
* Advertising starts on explicit user action after code entry, stops after 60 s monotonic, or at once
  when a central connects. At most one connection is accepted; further attempts are refused.
* The receiver scans (at most 10 s, extendable by the user) for the service UUID and shows a list of
  every advertiser it sees: tag plus a coarse signal bar, sorted by signal strength (strongest first).
  The USER selects the entry whose tag matches the phone in their hand. The receiver never
  auto-connects, not even when only one advertiser is visible, and never connects to two. If two
  advertisers show the same tag the receiver refuses to choose between them and asks to restart the
  phone session (new tag).
* Signal strength is only a sorting hint. It can be forged or distorted and is never trusted.
* A look-alike advertiser can copy a tag (a stranger copying the tag shown on a screen). Choosing it
  leads to a failed handshake, not to a leaked secret: the code never leaves the phone and the PAKE is
  bound to it. Such a failed handshake reveals nothing about the code and, if it fails before M3
  (malformed or wrong-format M2), costs no failure (D026). A fake peer that does complete M2 receives
  M3 and then counts as one failure, because M3 lets its receiver test one guess offline; the budget
  is three (D014).
* A disconnect at any time ends the session on both sides. Reconnecting always starts a fresh session
  (fresh code, fresh keys, fresh tag); nothing resumes.
* Platform note (unverified): the local name in the scan response is expected to work on iOS (app in
  the foreground), Android and Windows; if a platform hides it, the tag must be carried another way
  and this section changes.

## 4. Application messages carried

Each BLE message has a type byte and a payload:

| mtype | Meaning | Payload |
|---|---|---|
| 0x01 | handshake | one of M1, M2, M3, M4 exactly as in PROTOCOL.md |
| 0x02 | channel | one transport fragment as produced by `SecureChannel::seal` (13-byte header + ciphertext) |

Sizes (computed from the v1 encodings; M1 and M2 vary with the unverified device-id/claims fields):
M1 about 152 (at most 223), M2 about 189 (at most 274), M3 65, M4 33, channel fragment at most 269.
The BLE message limit is 512 bytes, with room to grow; larger declared lengths are rejected.

Order of events: central enables indications -> central writes M1 -> peripheral indicates M2 -> central
writes M3 -> peripheral indicates M4 -> channel fragments flow (SECRET from sender, RESULT from receiver).
During the handshake exactly one message is in flight; any message out of that order is an error.

## 5. Chunk framing (what actually goes in one GATT value)

A BLE message is split into chunks that each fit in one ATT value (ATT MTU - 3 bytes).

```
chunk  = flags:u8 | seq:u8 | body
flags  : bit0 FIRST, bit1 LAST, bits 2..7 reserved = 0
seq    : chunk counter inside one message, starts at 0 and increases by 1
body   : FIRST chunk starts with  mtype:u8 | total_len:u16 big-endian | data...
         later chunks are pure data
```

Receiver-side rules (the codec enforces them and fails closed):
* a message starts only with seq 0 and FIRST; a FIRST chunk inside an unfinished message is an error;
* seq must be exactly previous + 1; duplicates, gaps and reordering are errors;
* reserved flag bits set, unknown mtype, total_len 0 or above 512, or data that would exceed total_len
  are errors; a message completes when exactly total_len data bytes have arrived and LAST is set;
  LAST before the length is reached, or the length reached without LAST, is an error;
* any error closes the reassembler for good; the connection is dropped and the session discarded.
The smallest usable ATT value is 20 bytes (MTU 23): header 2 + FIRST prefix 3 + at least 1 data byte.
The sender chooses chunk size = min(ATT MTU - 3, 244); both sides request the largest MTU the platform
allows but must work at 23.

## 6. Timing and lifecycle

* The 60 s code lifetime of D014 bounds the whole session; the receiver is authoritative.
* A message must complete within 5 s of its FIRST chunk, otherwise the connection is dropped.
* If the central does not enable indications within 5 s of connecting, the peripheral disconnects.
* After the SECRET is delivered and the RESULT is exchanged, or on any error, both sides disconnect and
  wipe state. Reconnect = new session.

## 7. What BLE cannot tell us (kept honest)

* An ATT write response or an indication confirmation says the peer's BLE stack took the bytes, not that
  the application processed them. Some platforms do not report indication confirmation to the app (for
  example iOS peripheral APIs). Delivery is therefore decided only by application-level messages
  (M3/M4 confirmation, the RESULT message), never by BLE acknowledgements.
* Device names, addresses and advertising contents are claims. Proximity is not proven; relay attacks are
  possible (AGENTS.md).
* Radio observers see message sizes and timing.

## 8. Test plan (nothing below has been run on real devices)

Codec (local, synthetic, run): round trip for every message length 1..512 across ATT payloads 20..244;
duplicate, gap, reorder, missing FIRST, double FIRST, reserved bits, unknown mtype, zero and oversize
length, overflow, LAST too early/late, reassembler dead after an error, random mutations.
Real devices (to do): MTU 23 and negotiated MTU on iOS, Android and Windows; indication delivery and
ordering; disconnect at each step; two advertisers and a copied tag; wrong code; 60 s expiry; battery/background limits;
Windows `GattSession` behaviour; bonded-device interference.

## 9. Resolved and open questions

Resolved by the owner (2026-11-06, D026): (1) Notify with a per-chunk application ACK is allowed where
Indicate is unavailable; (2) a random per-start display tag in the scan response, with a signal-sorted
list on the receiver and user selection, replaces "exactly one advertiser"; (3) failures are counted
only for attempts that already sent M3; (4) crowded places are in scope.

Still open: (a) whether the platforms can carry the tag in the scan response as assumed; (b) address
rotation per attempt (platform dependent); (c) platform implementations of this framing through the
shared Rust codec over the C ABI (not started); (d) whether the receiver should rate-limit new
connections while several look-alike advertisers keep appearing.
