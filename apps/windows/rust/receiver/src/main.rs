//! KagiLink Windows receiver skeleton CLI (D022/D024). Synthetic data only.
//!
//! `check`    : read-only TPM assessment and the policy decision.
//! `loopback` : full in-process run of the shared core with a simulated phone and a synthetic
//!              secret. `--accept` answers the risk notice non-interactively (for tests/CI).
//! There is no BLE and no text entry into other windows yet.

use areatm_kagilink_receiver::{
    assess, decide, run_loopback, Decision, GateError, LoopbackError, TransferGate, NOTICE_EN,
    NOTICE_KO, RAM_ONLY_WARNING_EN, RAM_ONLY_WARNING_KO,
};
use std::io::{self, BufRead, Write};
use std::process::ExitCode;

fn print_assessment() -> Decision {
    let a = assess();
    println!("TPM state (claim) : {:?}", a.tpm);
    println!(
        "Secure Boot       : {}",
        match a.secure_boot {
            Some(true) => "enabled (registry claim, shown only)",
            Some(false) => "disabled (registry claim, shown only)",
            None => "not reported",
        }
    );
    println!(
        "Elevated          : {}",
        match a.elevated {
            Some(true) => "yes",
            Some(false) => "no (normal user)",
            None => "unknown",
        }
    );
    let d = decide(&a);
    match &d {
        Decision::Proceed { .. } => {
            println!("Decision          : PROCEED (the TPM claims a hardware-bound key; not a safety verdict)")
        }
        Decision::Block { reason } => println!("Decision          : BLOCK - {reason}"),
        Decision::RamOnly { reason } => {
            println!("Decision          : RAM-ONLY - {reason}");
            println!("{RAM_ONLY_WARNING_KO}\n{RAM_ONLY_WARNING_EN}");
        }
    }
    d
}

fn ask_acknowledgment() -> bool {
    println!("\n{NOTICE_KO}\n{NOTICE_EN}\n");
    print!("Type YES to acknowledge and continue (anything else cancels): ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line).is_ok() && line.trim() == "YES"
}

#[cfg(windows)]
fn ble_list(args: &[String]) -> ExitCode {
    use areatm_kagilink_receiver::selection::{build_list, SignalBar, Unselectable};
    let value_of = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse::<u64>().ok())
    };
    let seconds = value_of("--seconds").unwrap_or(10).clamp(1, 60);
    let choose_requested = args.iter().any(|a| a == "--choose");
    println!("Scanning for KagiLink senders for {seconds} s ...");
    let raws = match areatm_kagilink_receiver::ble::scan_advertisements(seconds) {
        Ok(r) => r,
        Err(e) => {
            println!("Scan failed: {e:?}");
            return ExitCode::from(1);
        }
    };
    let list = build_list(&raws);
    let entries = list.entries();
    if entries.is_empty() {
        println!("No KagiLink senders found");
    }
    for (i, c) in entries.iter().enumerate() {
        let n = i + 1;
        let tag = c.tag.as_deref().map(|t| format!("KL-{t}"));
        match (&c.blocked, tag) {
            (None, Some(t)) => {
                let bar = match c.signal {
                    SignalBar::Weak => "Weak",
                    SignalBar::Medium => "Medium",
                    SignalBar::Strong => "Strong",
                    SignalBar::Unknown => "Unknown",
                };
                println!("[{n}] {t}  signal: {bar}");
            }
            (Some(Unselectable::DuplicateTag), Some(t)) => {
                println!("[{n}] {t}  duplicate tag - cannot choose")
            }
            (Some(reason), _) => println!("[{n}] (no usable tag)  {reason:?}"),
            (None, None) => println!("[{n}] (no usable tag)  no tag"),
        }
    }
    if !entries.is_empty() {
        println!("Pick the row whose tag matches the tag shown on your phone.");
    }
    if choose_requested {
        let Some(n) = value_of("--choose") else {
            println!("--choose needs a row number.");
            return ExitCode::from(2);
        };
        let Some(index) = usize::try_from(n).ok().and_then(|n| n.checked_sub(1)) else {
            println!("Refused: row numbers start at 1.");
            return ExitCode::from(2);
        };
        match list.choose(index) {
            Ok(c) => {
                let t = c.tag.as_deref().unwrap_or("????");
                println!(
                    "Selected KL-{t} (not connecting: the connect step is not implemented yet)"
                );
            }
            Err(e) => {
                println!("Refused: {e:?}");
                return ExitCode::from(2);
            }
        }
    }
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("check") => {
            print_assessment();
            ExitCode::SUCCESS
        }
        Some("loopback") => {
            let decision = print_assessment();
            let mut gate = TransferGate::new(decision);
            let accepted = args.iter().any(|a| a == "--accept") || ask_acknowledgment();
            if !accepted {
                println!("Cancelled: the risk notice was not acknowledged.");
                return ExitCode::from(2);
            }
            match gate.acknowledge() {
                Err(GateError::Blocked(r)) => {
                    println!("Blocked: {r}");
                    return ExitCode::from(3);
                }
                Err(e) => {
                    println!("Gate error: {e:?}");
                    return ExitCode::from(2);
                }
                Ok(()) => {}
            }
            match run_loopback(&gate, b"synthetic-secret-not-a-real-password") {
                Ok(r) => {
                    println!(
                        "Loopback OK: mode {:?}, {} bytes received in {} fragments (plaintext not shown).",
                        r.mode, r.received_len, r.fragments
                    );
                    ExitCode::SUCCESS
                }
                Err(LoopbackError::Gate(e)) => {
                    println!("Gate refused: {e:?}");
                    ExitCode::from(3)
                }
                Err(LoopbackError::Core(e)) => {
                    println!("Core error: {e:?}");
                    ExitCode::from(1)
                }
            }
        }
        Some("ble-ping") => {
            #[cfg(windows)]
            {
                let seconds = args
                    .iter()
                    .position(|a| a == "--seconds")
                    .and_then(|i| args.get(i + 1))
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(10)
                    .clamp(1, 60);
                println!("Phase 0 BLE diagnostic (fixed public strings, no secret). Scanning {seconds} s ...");
                let outcome = areatm_kagilink_receiver::ble::ping_diagnostic(seconds);
                println!("Result: {outcome:?}");
                match outcome {
                    areatm_kagilink_receiver::ble::BleOutcome::ExchangeOk => ExitCode::SUCCESS,
                    _ => ExitCode::from(1),
                }
            }
            #[cfg(not(windows))]
            {
                println!("ble-ping is Windows only.");
                ExitCode::from(64)
            }
        }
        Some("ble-list") => {
            #[cfg(windows)]
            {
                ble_list(&args)
            }
            #[cfg(not(windows))]
            {
                println!("ble-list is Windows only.");
                ExitCode::from(64)
            }
        }
        _ => {
            println!(
                "usage: kagilink-receiver check | loopback [--accept] | ble-ping [--seconds N] | ble-list [--seconds N] [--choose N]"
            );
            println!("Skeleton only: no BLE, no text entry, synthetic data only.");
            ExitCode::from(64)
        }
    }
}
