//! Receiver session driver against the in-process fake phone (synthetic data, no radio).

use areatm_kagilink_core::{os_rng, Clock, Error as CoreError, ReceiverSession, SystemClock};
use areatm_kagilink_receiver::fake_sender::{Behaviour, FakeSender};
use areatm_kagilink_receiver::session::{run_attempt, SessionError};
use std::sync::Arc;

const SECRET: &[u8] = b"synthetic-secret-for-the-session-test-0123456789-abcdefghijklmnopqrstuvwxyz-0123456789-abcdefghijklmnopqrstuvwxyz-0123456789-abcdefghijklmnopqrstuvwxyz";

fn new_rx() -> (ReceiverSession, String) {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock::new());
    let mut rng = os_rng();
    let rx = ReceiverSession::new(clock, &mut rng, b"win-receiver", b"mode=test").unwrap();
    let code = rx.code().unwrap().to_string();
    (rx, code)
}

fn attempt(
    rx: &mut ReceiverSession,
    code: &str,
    b: Behaviour,
    chunk: usize,
    notify: bool,
) -> (
    Result<areatm_kagilink_receiver::session::ReceiveReport, SessionError>,
    FakeSender,
) {
    let mut phone = FakeSender::new(code, SECRET, b, chunk, notify);
    let mut rng = os_rng();
    let r = run_attempt(&mut phone, rx, &mut rng);
    (r, phone)
}

#[test]
fn honest_phone_across_chunk_sizes_and_modes() {
    for chunk in [20usize, 23, 64, 185, 244] {
        for notify in [false, true] {
            let (mut rx, code) = new_rx();
            let (r, phone) = attempt(&mut rx, &code, Behaviour::Honest, chunk, notify);
            let r = r.unwrap_or_else(|e| panic!("chunk {chunk} notify {notify}: {e:?}"));
            assert_eq!(r.received_len, SECRET.len());
            assert!(r.fragments >= 1);
            assert_eq!(phone.results_received, 1);
            assert!(!phone.ack_violation);
            assert_eq!(phone.acks_received > 0, notify);
        }
    }
}

#[test]
fn garbage_m2_is_rejected_for_free() {
    let (mut rx, code) = new_rx();
    for _ in 0..5 {
        let (r, _) = attempt(&mut rx, &code, Behaviour::GarbageM2, 64, false);
        assert!(matches!(r, Err(SessionError::Core(_))));
        assert_eq!(rx.failures(), 0);
    }
    let (r, _) = attempt(&mut rx, &code, Behaviour::Honest, 64, false);
    assert!(r.is_ok());
}

#[test]
fn wrong_code_after_m3_counts_and_budget_runs_out() {
    let (mut rx, code) = new_rx();
    for i in 1..=3u8 {
        let (r, _) = attempt(&mut rx, &code, Behaviour::WrongCode, 64, false);
        assert!(r.is_err(), "attempt {i} must fail");
        // An attempt abandoned after M3 is counted when the next attempt begins.
        // The next call below observes it.
    }
    let (r, _) = attempt(&mut rx, &code, Behaviour::Honest, 64, false);
    assert_eq!(r, Err(SessionError::Core(CoreError::TooManyFailures)));
    assert!(!rx.is_active());
}

#[test]
fn tampered_m4_counts_one_failure() {
    let (mut rx, code) = new_rx();
    let (r, _) = attempt(&mut rx, &code, Behaviour::TamperM4, 64, false);
    assert!(matches!(r, Err(SessionError::Core(_))));
    assert_eq!(rx.failures(), 1);
}

#[test]
fn tampered_secret_is_rejected_and_nothing_is_returned() {
    let (mut rx, code) = new_rx();
    let (r, phone) = attempt(&mut rx, &code, Behaviour::TamperSecret, 64, false);
    assert!(matches!(r, Err(SessionError::Core(_))));
    assert_eq!(phone.results_received, 0);
}

#[test]
fn silent_phone_times_out_and_link_faults_fail_closed() {
    let (mut rx, code) = new_rx();
    let (r, _) = attempt(&mut rx, &code, Behaviour::Silent, 64, false);
    assert_eq!(r, Err(SessionError::Timeout));
    for b in [
        Behaviour::DropAfterM3,
        Behaviour::DuplicateChunk,
        Behaviour::SkipChunk,
        Behaviour::OversizeLength,
    ] {
        let (mut rx, code) = new_rx();
        let (r, phone) = attempt(&mut rx, &code, b, 20, false);
        assert!(r.is_err(), "{b:?} must not transfer");
        assert_eq!(phone.results_received, 0, "{b:?}");
    }
}

#[test]
fn consumed_code_cannot_be_reused() {
    let (mut rx, code) = new_rx();
    let (r, _) = attempt(&mut rx, &code, Behaviour::Honest, 64, false);
    assert!(r.is_ok());
    let (r, _) = attempt(&mut rx, &code, Behaviour::Honest, 64, false);
    assert!(matches!(r, Err(SessionError::Core(_))));
}

#[test]
fn abandoned_attempt_after_m3_shows_in_the_open_count_before_it_is_counted() {
    let (mut rx, code) = new_rx();
    let (r, _) = attempt(&mut rx, &code, Behaviour::DropAfterM3, 64, false);
    assert!(r.is_err());
    assert_eq!(
        rx.failures(),
        0,
        "counted only when the next attempt begins (D026)"
    );
    assert_eq!(rx.failures_including_open(), 1);
    let (r, _) = attempt(&mut rx, &code, Behaviour::GarbageM2, 64, false);
    assert!(r.is_err());
    assert_eq!(rx.failures(), 1, "the abandoned attempt is counted now");
    assert_eq!(
        rx.failures_including_open(),
        1,
        "a garbage M2 costs nothing"
    );
}
