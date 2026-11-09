# RAM-only typing token (D032) — design note, DRAFT

Status: design only. Nothing is built. Owner approved the scope expansion on 2026-11-09 (D032). Every existing rule in
AGENTS.md still applies. This note is not a product claim; every hardware fact below is UNVERIFIED until the MCU study in
section 7 is run.

## 1. Why

Software input (`SendInput`) cannot reach the Windows 11 remote-desktop credential prompt (Phase 5: the prompt is a XAML host window
that exposes no UI Automation elements, and UIPI/secure-desktop rules block injection by design). A physical USB keyboard is an
ordinary input device that Windows accepts at the credential prompt, the lock screen and UAC. The token is a small, cheap,
user-owned device that acts as that keyboard for ONE value that the user just sent from the phone.

The phone stays the main device (the "key body"): it keeps the app-specific encrypted vault (its key protected by Secure Enclave or StrongBox, so
hardware protects the KEY, not the password string), chooses the value and approves each transfer. The token is only a typing peripheral with no
stored secret; losing the token loses nothing, losing the phone follows the device-loss guidance (delete the vault, change the passwords).

Not a YubiKey replacement: YubiKey static-password slots store the password permanently in the key (research note 2026-11-09:
write at configuration time, typed on touch; no runtime value). This token never stores a secret.

## 2. Shape

```
phone (sender) --BLE, CPace+HMAC confirm+HKDF+AES-256-GCM--> token --USB HID keyboard--> PC (any OS, any prompt)
```

* The token is the logical receiver: it runs the same session as the Windows receiver (60 s code, 3 failures, fresh keys,
  explicit mutual confirmation, transcript binding). The PC needs NO KagiLink software.
* The secret lives only in RAM, is typed once on an explicit button press, then zeroized. No flash write of any secret, ever
  (verified by a flash read-back diff, section 7). Power loss erases it. A lost token holds nothing.
* Approval: the token buttons (a) accept the transfer and (b) later "type now". The phone also needs the existing per-transfer
  approval. No transfer without both. A typed value consumes the permission.
* The token types over HID only. No USB storage, no serial, no network, no drivers on the PC, no admin rights.

## 3. Open design question: showing the one-time code

The code must be shown to the user by the logical receiver (AGENTS.md: do not silently reverse roles). A bare token has no screen.
Options, owner decision needed before any build:

1. A tiny OLED/e-paper display on the token (recommended; few components, nothing on the PC).
2. The token types the 6-digit code into a focused field on request. Rejected as default: puts the code on the PC screen and in
   any logger (it is short-lived, but the PC is the untrusted side).
3. The phone shows the code and the user enters it on the token (needs a keypad). More hardware, more error.

## 4. Typing: input methods and layouts (user must be told in advance)

HID sends key positions (scan codes), not characters. Results depend on the PC, so the token cannot guarantee text:

* Korean (Hangul/English), Japanese (hiragana/katakana/kanji, half/full width) and Chinese IMEs convert Latin key presses into
  other characters when the IME is not in direct (English/half-width alphanumeric) mode. A wrong mode corrupts the password.
* Keyboard layout (QWERTY, Dvorak, JIS vs ANSI) changes which key a position produces.
* Caps Lock / Num Lock state can change letters.
* Mitigations: a short pre-transfer notice on the phone (draft text must pass the same review as docs/RISK_NOTICE.md: switch the PC
  to English / half-width alphanumeric, check the layout, Caps Lock off, click the target field first); optional on-token
  prompt; limit the charset the phone will send over this path to what the chosen layout can type; refuse values the token
  cannot type; no silent fallback. Whether a Unicode input route is safe and works is NOT known and must not be assumed.
* No claim is made that the typed value matches the intended value on the PC. The user checks the login result.

## 5. Threats (summary, full list goes to docs/THREAT_MODEL.md when adopted)

* Compromised PC: a keylogger sees the typed value exactly as with any keyboard. Same D023 notice applies.
* Physical theft of a powered token: RAM may hold a value until typed or timed out. Mitigation: short expiry (the existing 60 s
  window plus a short "type now" timeout), button wipes, reset wipes.
* Rogue token or BadUSB suspicion: the device class is "keyboard". Companies may block new HID devices. See the company brief.
* Debug ports and flash dump: secrets are never in flash, but the firmware could be tampered. Secure boot / flash encryption
  where the chip supports it, and signed firmware updates, are studied in section 7, not assumed.
* Phone/BLE attacks: the existing protocol applies (code, PAKE, confirmation, replay defense). Which BLE security features the
  chosen MCU stack supports is unverified.

## 6. What stays out of scope

No persistent vault on the token, no PIN-protected stored passwords, no FIDO/PIV/OATH features, no OS drivers or services, no
input into a PC without a physical USB connection, no use as an attack tool. Real-secret transfer stays disabled until the
owner lifts D012 for this path with evidence.

## 7. MCU verification plan (required before any design is adopted)

Candidate first: an ESP32-S3 development board (BLE and USB device on one chip; flash encryption and secure boot are documented by
the vendor). nRF52840 and RP2350 + wireless module are alternates. Use a sacrificial board; burning eFuses or OTP is irreversible
and needs explicit owner approval.

Checks, each with recorded evidence (no check is assumed to pass):

1. BLE peripheral and USB HID keyboard run at the same time without drops; typing rate and error rate over 1,000 test strings.
2. The shared core (Rust) builds for the chip, or a minimal port is feasible; RAM and flash use; CPace time on the chip.
3. Secret handling: `zeroize` effective on the target; no secret in flash (flash dump diff before/after a transfer); no secret in
   logs or the debug UART; reset wipes.
4. Windows 10/11 acceptance of the token at the login screen, UAC prompt and RDP credential prompt (synthetic account only).
5. Layout/IME behaviour matrix (ko, ja, en; IME on/off; Caps Lock) with the recorded outcomes.
6. Secure boot and flash encryption: which can be enabled, what is irreversible, how firmware updates are signed.
7. Cost of a complete unit (board, display, button, case) from real purchase quotes.

Exit criterion for the next phase: all seven recorded, results reviewed by the owner, then a new decision.
