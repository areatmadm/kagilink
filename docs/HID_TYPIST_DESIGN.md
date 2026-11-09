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

## 3. Showing the one-time secret: QR on a small display (PROPOSED, owner steering 2026-11-09)

The code must be shown by the logical receiver (AGENTS.md: do not silently reverse roles). Direction chosen with the owner, inspired by the
proximity idea of cross-device passkeys but WITHOUT accounts, cloud or relay servers:

* The token shows a QR on a small display (a 128x64-class monochrome panel is expected to fit a small QR; this must be verified). The QR carries a
  high-entropy one-time secret, the token's tag and the session length chosen on the phone. The phone scans it (camera stays local), then connects
  only to that token over BLE. This replaces typing six digits and also replaces the human tag comparison of D026 for this path.
* The protocol stays CPace with HMAC confirmation, HKDF and AES-256-GCM; the one-time secret simply replaces the six-digit PAKE password (higher
  entropy, never weaker). The QR is valid for 60 seconds and once; three confirmation failures discard it. docs/PROTOCOL.md would need a matching,
  separately approved change; it is NOT changed here.
* Rejected for now: a long-term pairing between phone and token (a stored key on the token breaks "stores no secret", needs secure boot and flash
  encryption to resist cloning, and conflicts with the no-reuse rules). Typing the code on the PC is the fallback only if no display is possible.

### 3.1 Session window (PROPOSED)

After the QR is scanned the session stays open for a limited time so several values can be sent without scanning again (user name then password,
retry after fixing the input mode).

* Default 5 minutes. Selectable fixed steps (for example 1, 5, 10, 30 minutes); no free input. The chosen length is in the QR, so the phone cannot
  extend it later. The token enforces an absolute cap of 30 minutes in firmware and refuses anything longer.
* Idle timeout (default 2 minutes without a transfer) ends the session early, whatever time is left.
* At most 3 values per session. EVERY value needs the phone approval and the token button; one approval consumes one value. Nothing is typed
  without both.
* Lengths above 5 minutes show a warning on the phone ("a longer session means longer exposure") and need one extra confirmation on the token. A
  change of receiver resets the length to 5 minutes. Shared or public PCs: the notice recommends 1 minute.
* The session ends, session keys and any held value are zeroized, on disconnect, timeout, idle timeout, the wipe button, power loss or three
  confirmation failures. A new session needs a new QR.
* Open: BLE connection stability and phone battery over long sessions; the token display countdown; the exact numbers (all are drafts).

### 3.2 Shared / public PC warning (DRAFT text, owner liked the idea 2026-11-09)

Shown on the phone before the session length is chosen, and again whenever the receiver changes. It selects 1 minute as the default for this case. Wording follows the
D023 rules: it never calls a PC safe or verified. Both languages need the same review as docs/RISK_NOTICE.md before use.

* KO: "PC방·공용 PC에서는 세션을 1분으로 줄이세요. 이 PC에 키로거나 화면 녹화 프로그램이 있으면 입력한 값이 노출될 수 있고, KagiLink는 이를 알아내지 못합니다. 일회용 비밀번호만 보내고, 끝나면 지우기 버튼을 누른 뒤 비밀번호를 바꾸세요."
* EN: "On internet cafe (PC bang) PCs or any other shared PC, shorten the session to 1 minute. If this PC has a keylogger or screen recorder, the typed value can be exposed and KagiLink cannot tell. Send only one-time passwords, press the wipe button when finished, and change the password afterwards."

Venue wording is localized so it is recognized at once, and the generic words "shared or public PC" (internet cafe, library, hotel business centre, school lab) stay in every
language. Drafts below are NOT reviewed; each needs a native-speaker review (legal tone, no "safe"/"verified" for a PC) before use. Traditional Chinese (Taiwan, Hong Kong) is separate and not drafted.

* JA (ネットカフェ): "ネットカフェや共用PCでは、セッションを1分に短縮してください。このPCにキーロガーや画面録画ソフトがあると、入力した値が漏れる可能性があり、KagiLinkはそれを検知できません。使い捨てのパスワードだけを送り、終わったら消去ボタンを押し、あとでパスワードを変更してください。"
* ZH-CN (网吧): "在网吧或公共电脑上，请把会话缩短为1分钟。如果这台电脑上有键盘记录器或录屏软件，输入的内容可能泄露，KagiLink 无法检测。请只发送一次性密码，完成后按清除键，并在事后更换密码。"
* VI (quán net): "Trên máy ở quán net hoặc máy dùng chung, hãy rút ngắn phiên xuống 1 phút. Nếu máy này có phần mềm ghi phím hoặc quay màn hình, giá trị được gõ có thể bị lộ và KagiLink không thể phát hiện điều đó. Chỉ gửi mật khẩu dùng một lần, nhấn nút xóa khi xong và đổi mật khẩu sau đó."

The phone asks "Is this a shared or public PC?" (no automatic detection is claimed). If yes: default 1 minute, the longer steps need the extra confirmation, and the notice is repeated at every new session.

### 3.3 Measured: an iPhone app cannot publish the HID service (2026-11-09)

Throwaway test app on an iPhone 15 Pro (synthetic, nothing stored, removed after the test; existing certificate and profile). `CBPeripheralManager.add` with the HID
service 0x1812 (five characteristics) returned no exception, but `didAdd` reported `CBErrorDomain` code 8 "The specified UUID is not allowed for this operation." A control service with
a custom UUID was accepted and advertised. Advertising with 0x1812 started without error, but the service is not in the GATT table, and whether 0x1812 was in the advertising packet was NOT checked. Result: an iPhone
app cannot act as a Bluetooth keyboard through Core Bluetooth on this OS version (one measurement; other iOS versions and any MFi route unverified). So "the phone is the keyboard" is not available on iPhone; the USB
token stays the only route that covers both phones. The Android API route (BluetoothHidDevice) is still an unmeasured optional experiment.

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

## 8. Reference: approaches considered (2026-11-09, owner will keep weighing several)

Evidence quality: web surveys by a low-effort model (search based, official pages not always readable) plus one device measurement. Nothing here is a decision.

| Approach | Covers | Result so far |
|---|---|---|
| USB token, RAM-only typist (this note) | any PC and prompt, public PCs, login/UAC, RDP prompt, web | design only; hardware verification (section 7) not done |
| YubiKey 5 static password slot | typing on touch | not usable as designed: the value is written into the key at configuration time and stays there; no value chosen at touch time (vendor SDK docs) |
| Phone as Bluetooth HID keyboard, Android (`BluetoothHidDevice`, Android 9+) | any PC that accepts the pairing | API exists; one app at a time; needs a prior OS pairing (a bonded keyboard can send ANY key, a larger risk than this protocol); Windows sign-in screen behaviour unverified; not measured on a device |
| Phone as Bluetooth HID keyboard, iPhone | none | measured: Core Bluetooth rejects service 0x1812 (`uuidNotAllowed`, one iPhone 15 Pro, one OS version) |
| Cross-device passkey (hybrid/PXP) | passkey sites only | needs internet and Bluetooth on both devices (vendor docs); conflicts with the offline, no-account rule; not a password-prompt replacement |
| Own RDP client with the credentials passed in memory (for example a Rust library such as IronRDP, MIT or Apache-2.0, claims NLA support) | RDP only | not reviewed or tried; maintenance, audit and password-handling questions open; would also need AGENTS.md dependency review; login success is a protocol result, not a guess |
| Windows credential provider (system component) | sign-in and UAC | conflicts with the no drivers, services or secure-desktop rule; not pursued |
| Web page using Web Bluetooth | our own page only | cannot fill other sites' fields (same-origin rule of browsers); a browser extension could, on personal PCs only; not studied |
| Car-style digital key ideas (SOCAR, Apple CarKey, CCC Digital Key) | design inspiration | one-time password then PAKE channel is the same family as our protocol; long-lived keys in a secure chip and UWB distance bounding are NOT adopted; how SOCAR registers its controller in the OS list is undocumented |

Notes for the design:

* Phone side: avoid OS-level bonding with the token. Do not touch characteristics that require encryption at the OS layer; our own CPace and AES-GCM channel is the protection. Do not use accessory-registration frameworks that leave a persistent entry; a user must be able to remove anything at any time (no "locked until return" behaviour). Whether an ordinary app can prevent "Forget this device" is not documented.
* A bonded keyboard of any kind is a bigger trust grant than a one-shot token: the token firmware types printable characters only (no Enter, Tab or shortcuts) and holds no secret.
* Layered plan under discussion: the token for universal coverage, service-specific direct authentication (RDP client) where login success can be proven, and an optional Android pairing mode with explicit warnings.
