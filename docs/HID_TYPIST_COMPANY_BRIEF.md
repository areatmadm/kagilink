# KagiLink typing token — security brief for company reviewers, DRAFT

This is a plain-language summary for a company security team. It is a draft: the device does not exist yet, no hardware claim in
it has been verified, and it is not a certification or legal advice. Your organization's policy always wins. If your security
team does not allow the device, it must not be used on your computers.

## What it would be

A small user-owned USB device. The user sends a one-time password from their phone over Bluetooth Low Energy; the user presses a
button on the device; the device types that value as a USB keyboard; the value is then erased from the device.

## What it does NOT do

* It does not store passwords. The value exists only in the device's working memory and is erased after typing, after a timeout,
  or when power is removed.
* It has no account, no cloud, no network, no telemetry and no analytics. Authentication is offline, directly between the phone
  and the device.
* It does not install software or drivers on the PC and does not need administrator rights. The PC sees a standard USB keyboard.
* It does not appear as USB storage or serial devices, and it does not read the PC.

## What a reviewer can check

1. USB class: HID keyboard only (a descriptor dump is part of the verification plan). No mass storage, no network, no serial.
2. Bluetooth: Low Energy, short-lived pairing code (60 seconds, at most three wrong attempts, then discarded); authenticated key
   exchange (a password-authenticated protocol), mutual confirmation, encrypted transfer; no raw password sent in clear.
3. Data at rest: none for secrets. Verification plan includes comparing the device memory before and after a transfer.
4. Firmware: open source (Apache-2.0); signed releases with checksums; secure-boot support depends on the chip and is reported as
   verified or not verified in the release notes.
5. Logging: the device does not log secrets; test builds carry a visible warning.

## Known limits (honest)

* A compromised PC sees the typed value exactly as it would see any keyboard input (keylogger, screen capture). The device cannot
  detect or prevent this. Send only one-time passwords and change them afterwards.
* Typing depends on the PC: keyboard layout and input method editors (Korean, Japanese, Chinese) can change what is typed.
  Users are told to switch to direct English / half-width alphanumeric input before typing. The device cannot check this.
* Someone holding a powered device and its button could type a pending value before it expires; the window is short and any
  press-to-wipe or reset erases it.
* Any device that behaves like a keyboard can in principle be abused. This one is designed for the owner's own computers and has
  no feature to run commands or load payloads; the firmware has a fixed, reviewable behaviour.

## How an organization can control it

* Block or allow-list USB HID devices by vendor/product ID with the usual endpoint tools; the device will use a fixed ID that is
  published in the release notes.
* Disable Bluetooth on the phone side or the device side via policy.
* The device needs no PC-side component, so there is nothing to uninstall.

## Status

Design stage (docs/HID_TYPIST_DESIGN.md, decision D032). Hardware verification has not been done. Do not deploy.
