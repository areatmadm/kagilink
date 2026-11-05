//! KagiLink Windows receiver skeleton CLI (D022/D024). Synthetic data only.
//!
//! `check`    : read-only TPM assessment and the policy decision.
//! `loopback` : full in-process run of the shared core with a simulated phone and a synthetic
//!              secret. `--accept` answers the risk notice non-interactively (for tests/CI).
//! There is no BLE and no text entry into other windows yet.

use areatm_kagilink_core::{os_rng, Clock, ReceiverSession, SystemClock};
use areatm_kagilink_receiver::fake_sender::{Behaviour, FakeSender};
use areatm_kagilink_receiver::session::{run_attempt, SessionError};
use areatm_kagilink_receiver::{
    assess, decide, run_loopback, Decision, GateError, LoopbackError, TransferGate, NOTICE_EN,
    NOTICE_KO, RAM_ONLY_WARNING_EN, RAM_ONLY_WARNING_KO,
};
use std::io::{self, BufRead, Write};
use std::process::ExitCode;
use std::sync::Arc;

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

/// Assess, show the decision, require the risk-notice acknowledgment (D023) and return the gate.
fn acknowledged_gate(args: &[String]) -> Result<TransferGate, ExitCode> {
    let decision = print_assessment();
    let mut gate = TransferGate::new(decision);
    let accepted = args.iter().any(|a| a == "--accept") || ask_acknowledgment();
    if !accepted {
        println!("Cancelled: the risk notice was not acknowledged.");
        return Err(ExitCode::from(2));
    }
    match gate.acknowledge() {
        Err(GateError::Blocked(r)) => {
            println!("Blocked: {r}");
            Err(ExitCode::from(3))
        }
        Err(e) => {
            println!("Gate error: {e:?}");
            Err(ExitCode::from(2))
        }
        Ok(()) => Ok(gate),
    }
}

/// Current UTC time as `YYYY-MM-DD HH:MM:SS UTC` (no time zone database needed). Printed so a tester can match
/// PC output with a phone log by time; it carries no secret.
fn utc_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_utc(secs)
}

fn format_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        rem / 3_600,
        (rem % 3_600) / 60,
        rem % 60
    )
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn report_attempt(
    result: Result<areatm_kagilink_receiver::session::ReceiveReport, SessionError>,
) -> ExitCode {
    match result {
        Ok(r) => {
            println!(
                "Transfer OK: {} bytes received in {} fragments (plaintext not shown).",
                r.received_len, r.fragments
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            println!("Attempt failed: {e:?}");
            ExitCode::from(1)
        }
    }
}

/// Full receiver session against the in-process fake phone (synthetic data, no radio).
fn fake_session(args: &[String]) -> ExitCode {
    let gate = match acknowledged_gate(args) {
        Ok(g) => g,
        Err(c) => return c,
    };
    let mode = match gate.permit() {
        Ok(m) => m,
        Err(e) => {
            println!("Gate refused: {e:?}");
            return ExitCode::from(3);
        }
    };
    let behaviour = match flag_value(args, "--behaviour").as_deref() {
        None | Some("honest") => Behaviour::Honest,
        Some("wrong-code") => Behaviour::WrongCode,
        Some("garbage-m2") => Behaviour::GarbageM2,
        Some("tamper-m4") => Behaviour::TamperM4,
        Some("tamper-secret") => Behaviour::TamperSecret,
        Some("silent") => Behaviour::Silent,
        Some("drop-after-m3") => Behaviour::DropAfterM3,
        Some("duplicate-chunk") => Behaviour::DuplicateChunk,
        Some("skip-chunk") => Behaviour::SkipChunk,
        Some("oversize") => Behaviour::OversizeLength,
        Some(other) => {
            println!("Unknown behaviour {other}");
            return ExitCode::from(64);
        }
    };
    let chunk = flag_value(args, "--chunk")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(244)
        .clamp(20, 244);
    let notify = args.iter().any(|a| a == "--notify");
    let clock: Arc<dyn Clock> = Arc::new(SystemClock::new());
    let mut rng = os_rng();
    let mut rx = match ReceiverSession::new(clock, &mut rng, b"win-receiver", mode.claim_label()) {
        Ok(r) => r,
        Err(e) => {
            println!("Core error: {e:?}");
            return ExitCode::from(1);
        }
    };
    let code = rx.code().map(str::to_owned).unwrap_or_default();
    println!("Fake phone: behaviour {behaviour:?}, chunk {chunk}, notify {notify}, mode {mode:?}");
    let mut phone = FakeSender::new(
        &code,
        b"synthetic-secret-not-a-real-password",
        behaviour,
        chunk,
        notify,
    );
    report_attempt(run_attempt(&mut phone, &mut rx, &mut rng))
}

/// Real BLE session: show the code, scan, let the USER choose a row, connect, run one attempt.
#[cfg(windows)]
fn ble_receive(args: &[String]) -> ExitCode {
    use areatm_kagilink_core::MAX_FAILURES;
    use areatm_kagilink_receiver::selection::build_list;
    let gate = match acknowledged_gate(args) {
        Ok(g) => g,
        Err(c) => return c,
    };
    let mode = match gate.permit() {
        Ok(m) => m,
        Err(e) => {
            println!("Gate refused: {e:?}");
            return ExitCode::from(3);
        }
    };
    let clock: Arc<dyn Clock> = Arc::new(SystemClock::new());
    let mut rng = os_rng();
    let mut rx = match ReceiverSession::new(clock, &mut rng, b"win-receiver", mode.claim_label()) {
        Ok(r) => r,
        Err(e) => {
            println!("Core error: {e:?}");
            return ExitCode::from(1);
        }
    };
    let code = rx.code().map(str::to_owned).unwrap_or_default();
    println!("Code (valid 60 s): {code}");
    println!("Code created: {}", utc_stamp());
    println!("Enter it on the phone, press start, and note the KL-XXXX tag the phone shows.");
    let seconds = flag_value(args, "--seconds")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10)
        .clamp(1, 30);
    let scripted = flag_value(args, "--choose").is_some();
    // One code can be tried more than once: every attempt that already sent M3 and fails uses one
    // of the three failures (D026), so the budget is shown after each attempt and the user may
    // retry with the SAME code until it is spent or expires.
    loop {
        if !rx.is_active() {
            println!("The code is no longer active (expired, used up or consumed).");
            return ExitCode::from(1);
        }
        let raws = match areatm_kagilink_receiver::ble::scan_advertisements(seconds) {
            Ok(r) => r,
            Err(e) => {
                println!("Scan failed: {e:?}");
                return ExitCode::from(1);
            }
        };
        let list = build_list(&raws);
        if list.entries().is_empty() {
            println!("No KagiLink senders found");
            return ExitCode::from(1);
        }
        for (i, c) in list.entries().iter().enumerate() {
            match (&c.blocked, &c.tag) {
                (None, Some(t)) => println!("[{}] KL-{t}  signal: {:?}", i + 1, c.signal),
                (_, Some(t)) => println!("[{}] KL-{t}  cannot choose ({:?})", i + 1, c.blocked),
                _ => println!("[{}] (no usable tag)  cannot choose", i + 1),
            }
        }
        let choice = match flag_value(args, "--choose") {
            Some(v) => v.parse::<usize>().ok(),
            None => {
                print!("Pick the row whose tag matches your phone: ");
                let _ = io::stdout().flush();
                let mut line = String::new();
                let _ = io::stdin().lock().read_line(&mut line);
                line.trim().parse::<usize>().ok()
            }
        };
        let Some(index) = choice.and_then(|n| n.checked_sub(1)) else {
            println!("No valid row chosen. Nothing was connected.");
            return ExitCode::from(2);
        };
        let target = match list.choose_target(index) {
            Ok(t) => t,
            Err(e) => {
                println!("Refused: {e:?}");
                return ExitCode::from(2);
            }
        };
        let mut link = match areatm_kagilink_receiver::ble_link::connect(target.key()) {
            Ok(l) => l,
            Err(e) => {
                println!("Connect failed: {e:?}");
                return ExitCode::from(1);
            }
        };
        println!("Attempt started: {}", utc_stamp());
        let result = run_attempt(&mut link, &mut rx, &mut rng);
        println!(
            "Failure budget used: {}/{} (an attempt abandoned after M3 is included)",
            rx.failures_including_open(),
            MAX_FAILURES
        );
        if result.is_ok() || scripted {
            return report_attempt(result);
        }
        report_attempt(result);
        print!("Retry with the same code? Type y to scan again (anything else ends): ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        let _ = io::stdin().lock().read_line(&mut line);
        if line.trim() != "y" {
            return ExitCode::from(1);
        }
    }
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
        Some("fake-session") => fake_session(&args),
        Some("ble-receive") => {
            #[cfg(windows)]
            {
                ble_receive(&args)
            }
            #[cfg(not(windows))]
            {
                println!("ble-receive is Windows only.");
                ExitCode::from(64)
            }
        }
        _ => {
            println!(
                "usage: kagilink-receiver check | loopback [--accept] | ble-ping [--seconds N] | ble-list [--seconds N] [--choose N] | fake-session [--behaviour B] [--chunk N] [--notify] [--accept] | ble-receive [--seconds N] [--choose N] [--accept]"
            );
            println!("Skeleton only: no BLE, no text entry, synthetic data only.");
            ExitCode::from(64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format_utc;

    #[test]
    fn utc_formatting_matches_known_instants() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(format_utc(1_700_000_000), "2023-11-14 22:13:20 UTC");
        assert_eq!(format_utc(951_782_400), "2000-02-29 00:00:00 UTC");
        assert_eq!(format_utc(1_793_923_200), "2026-11-06 00:00:00 UTC");
    }
}
