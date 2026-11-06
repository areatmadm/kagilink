# Windows test station (second Windows machine for BLE tests)

Status: process document, 2026-11-06. It lets a second Windows computer (for example a school or academy PC)
run the receiver-side BLE tests when the home PC is not available. It changes no protocol, policy or decision.

## Role and limits

* The station is a **test station only**. The lead stays the home Windows session (D015). The station does not merge
  PRs, change decisions, or edit shared policy files; it builds, runs tests and reports exact outputs to the lead.
* Synthetic data only. No real password, token or key is ever typed or sent (D012 keeps real-secret transfer off).
* Owner-seat variant (owner decision, 2026-11-06): if the owner states that the machine is a seat only they use and that
  they must stay signed in there for other work, the station MAY use the owner's GitHub sign-in and the private memory
  gate (`tools/Test-AiMemoryAccess.ps1` must still exit 0). The rules below that are not about sign-in still apply:
  never print or store tokens in files, no real secrets, no pushes to `main` (PRs only, and only when the lead asks),
  conversation records stay out of public files and logs, and the owner signs out or the lead is told if the seat stops
  being exclusive. Otherwise treat the computer as shared and untrusted:
* The default assumption is a **shared and untrusted** computer: it may have keyloggers or other users. Therefore:
  * Do NOT sign in to the owner's GitHub account, and do not store any token or password on it. Clone the public
    repository over HTTPS read-only. The station cannot push; results go to the lead as text.
  * Do NOT clone or read the private memory repository (`shared/ai/`). The AGENTS.md memory gate needs the owner's
    authenticated identity, so on this machine it must fail and the station continues with public files only.
  * Do not install drivers, services or security changes (AGENTS.md). Use a normal user account.
  * Delete the working copy and any build output when the session ends if the machine is shared.
  * The receiver prints a 6-digit code on the screen for 60 s. Anyone looking at the screen sees it. Use test codes only.
* The receiver's trust check (TPM) is a claim, not a safety verdict (D024). On a PC without a usable TPM it runs in the
  labelled RAM-only mode with a warning; that is expected and fine for synthetic tests.

## What the machine needs

* Windows 10 or 11 with a Bluetooth 4.0+ (LE) adapter that supports the central role. `ble-receive` reports
  "central role unsupported" or "no adapter" when it does not.
* Rust 1.92.0 (rustup) and the MSVC build tools (Visual Studio Build Tools, "Desktop development with C++"). Installing
  them may need administrator rights; if they are not available, ask the owner to download the unsigned test build from the
  `release-dryrun` workflow artifact (manual workflow; verify the SHA-256 printed in its log) instead of building from source.
* Git, and network access to github.com.

## Steps

```
git clone https://github.com/areatmadm/kagilink.git
cd kagilink
git log -1 --oneline            # note the commit; report it
rustup toolchain install 1.92.0
cargo +1.92.0 build -p areatm-kagilink-receiver --locked
target\debug\areatm-kagilink-receiver.exe check
target\debug\areatm-kagilink-receiver.exe fake-session --accept
```

`check` prints the TPM claim and the policy decision. `fake-session --accept` runs the full receiver against the in-process
fake phone with no radio: it must print `Transfer OK`. If either fails, stop and report the output; nothing about the
radio has been tested yet.

For a real phone test (phone app installed by the phone sessions; the owner operates the phone):

```
target\debug\areatm-kagilink-receiver.exe ble-receive
```

Type `YES` at the risk notice, read the 6-digit code and the `Code created` UTC time, enter the code on the phone within
60 s and press start, then pick the row whose `KL-XXXX` tag is shown on the phone. After a failed attempt the receiver
prints `Failure budget used: n/3` and offers to retry with the same code (type `y`); a code that expires or reaches 3/3 is
destroyed. The `Attempt started` UTC time lets the phone log be matched by time.

## What to report (paste exact text, no screenshots of codes needed)

1. Commit id, `rustc --version`, Windows version, adapter name only if the owner agrees.
2. Output of `check` and `fake-session --accept`.
3. For each `ble-receive` run: the lines from `Code created` to the final result, the tag list (tags are public, not secret),
   and which phone mode or fault mode was used.
4. Anything unexpected: errors, hangs, adapter messages.

## Not covered here

* Real secrets, text entry into other windows, RDP input (Phase 5), release signing. None is enabled.
* Different adapters behave differently (MTU, scan response merging). Record the adapter vendor/model with the results because
  it is a variable, not a defect.
