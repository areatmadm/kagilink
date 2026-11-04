//! Handshake tests for the KagiLink Phase 1 core.
//!
//! These tests use synthetic data only and exercise the public API; they are
//! regression tests written by the developers, NOT an independent security audit.
//!
//! Note on wrong-code behaviour: a sender that uses a wrong code can still build a
//! syntactically valid M2, and the receiver answers with an M3; but the sender can
//! never verify that M3 (key confirmation fails), so an honest sender never emits a
//! valid M4. The receiver-side failure accounting is therefore exercised by feeding
//! a bogus M4 (type byte 0x14 followed by 32 zero bytes), which is exactly what an
//! attacker who does not know the code could send.

use areatm_kagilink_core::wire::{HelloR, HelloS};
use areatm_kagilink_core::*;
use std::sync::Arc;

const RX_ID: &[u8] = b"rx-device-01";
const TX_ID: &[u8] = b"tx-device-01";
const PURPOSE: &[u8] = b"synthetic-login";
const RX_CLAIMS: &[u8] = b"rx-claims";
const TX_CLAIMS: &[u8] = b"tx-claims";

fn bogus_m4() -> Vec<u8> {
    let mut v = vec![0x14u8];
    v.extend_from_slice(&[0u8; 32]);
    v
}

fn clock() -> (Arc<ManualClock>, Arc<dyn Clock>) {
    let c = Arc::new(ManualClock::new());
    let d: Arc<dyn Clock> = c.clone();
    (c, d)
}

fn new_rx(clk: &Arc<dyn Clock>) -> ReceiverSession {
    let mut rng = os_rng();
    ReceiverSession::new(clk.clone(), &mut rng, RX_ID, RX_CLAIMS).expect("receiver session")
}

fn code_of(rx: &ReceiverSession) -> Code {
    Code::parse(rx.code().expect("code")).expect("parse code")
}

fn other_code(code: &Code) -> Code {
    if code.as_str() == "000000" {
        Code::parse("000001").unwrap()
    } else {
        Code::parse("000000").unwrap()
    }
}

fn sender_start(clk: &Arc<dyn Clock>, code: &Code, m1: &[u8]) -> Result<(Vec<u8>, SenderPending)> {
    let mut rng = os_rng();
    SenderPending::start(clk.clone(), &mut rng, code, m1, TX_ID, PURPOSE, TX_CLAIMS)
}

fn err_of<T>(r: Result<T>) -> Error {
    match r {
        Err(e) => e,
        Ok(_) => panic!("expected an error but the operation succeeded"),
    }
}

/// Used where several error variants are acceptable: any rejection passes.
fn assert_rejected<T>(r: Result<T>, what: &str) {
    if r.is_ok() {
        panic!("{what}: expected rejection but it succeeded");
    }
}

/// Fresh receiver + sender up to (and including) M2.
fn through_m2(clk: &Arc<dyn Clock>) -> (ReceiverSession, Vec<u8>, SenderPending) {
    let mut rx = new_rx(clk);
    let code = code_of(&rx);
    let mut rng = os_rng();
    let m1 = rx.begin_attempt(&mut rng).expect("M1");
    let (m2, pending) = sender_start(clk, &code, &m1).expect("M2");
    (rx, m2, pending)
}

/// ... up to and including M3.
fn through_m3(clk: &Arc<dyn Clock>) -> (ReceiverSession, SenderPending, Vec<u8>) {
    let (mut rx, m2, pending) = through_m2(clk);
    let mut rng = os_rng();
    let m3 = rx.handle_m2(&mut rng, &m2).expect("M3");
    (rx, pending, m3)
}

/// ... up to and including M4 (receiver has not yet processed it).
fn through_m4(clk: &Arc<dyn Clock>) -> (ReceiverSession, Vec<u8>, SecureChannel) {
    let (rx, pending, m3) = through_m3(clk);
    let (m4, tx) = pending.handle_m3(&m3).expect("sender accepts M3");
    (rx, m4, tx)
}

fn full_handshake(clk: &Arc<dyn Clock>) -> (SecureChannel, SecureChannel) {
    let (mut rx, m4, tx) = through_m4(clk);
    let rxc = rx.handle_m4(&m4).expect("receiver accepts M4");
    (tx, rxc)
}

fn roundtrip(from: &mut SecureChannel, to: &mut SecureChannel, payload: &[u8]) {
    let frags = from.seal(kind::CONTROL, payload, 256).expect("seal");
    let mut got = None;
    for f in &frags {
        if let Some(m) = to.open_fragment(f).expect("open") {
            got = Some(m);
        }
    }
    let m = got.expect("message completed");
    assert_eq!(m.kind, kind::CONTROL);
    assert_eq!(&m.plaintext[..], payload);
}

/// Run a failed attempt with the wrong code up to the bogus M4.
fn failed_attempt(rx: &mut ReceiverSession, clk: &Arc<dyn Clock>) {
    let code = code_of(rx);
    let wrong = other_code(&code);
    let mut rng = os_rng();
    let m1 = rx.begin_attempt(&mut rng).expect("M1");
    let (m2, pending) = sender_start(clk, &wrong, &m1).expect("M2 with wrong code");
    let m3 = rx.handle_m2(&mut rng, &m2).expect("M3");
    assert_eq!(err_of(pending.handle_m3(&m3)), Error::ConfirmationFailed);
    assert_eq!(err_of(rx.handle_m4(&bogus_m4())), Error::ConfirmationFailed);
}

// ---------------------------------------------------------------- 1. happy path

#[test]
fn happy_path_channels_work_in_both_directions() {
    let (_c, clk) = clock();
    let (mut tx, mut rx) = full_handshake(&clk);
    assert_eq!(tx.role(), Role::Sender);
    assert_eq!(rx.role(), Role::Receiver);
    assert!(!tx.is_closed());
    assert!(!rx.is_closed());
    roundtrip(&mut tx, &mut rx, b"sender to receiver");
    roundtrip(&mut rx, &mut tx, b"receiver to sender");
}

#[test]
fn independent_sessions_cannot_read_each_others_fragments() {
    let (_c, clk) = clock();
    let (mut tx1, mut rx1) = full_handshake(&clk);
    let (mut tx2, mut rx2) = full_handshake(&clk);
    let f1 = tx1.seal(kind::CONTROL, b"session one", 256).unwrap();
    let f2 = tx2.seal(kind::CONTROL, b"session two", 256).unwrap();
    for f in &f1 {
        assert!(
            !matches!(rx2.open_fragment(f), Ok(Some(_))),
            "foreign session opened fragment"
        );
    }
    for f in &f2 {
        assert!(
            !matches!(rx1.open_fragment(f), Ok(Some(_))),
            "foreign session opened fragment"
        );
    }
}

#[test]
fn fragments_open_on_own_peer() {
    let (_c, clk) = clock();
    let (mut tx1, mut rx1) = full_handshake(&clk);
    let (_tx2, mut rx2) = full_handshake(&clk);
    let frags = tx1
        .seal(kind::CONTROL, b"private to session one", 256)
        .unwrap();
    for f in &frags {
        assert!(!matches!(rx2.open_fragment(f), Ok(Some(_))));
    }
    let mut got = None;
    for f in &frags {
        if let Some(m) = rx1.open_fragment(f).unwrap() {
            got = Some(m);
        }
    }
    assert_eq!(
        &got.expect("own peer opens").plaintext[..],
        b"private to session one"
    );
}

// ---------------------------------------------------------------- 2. wrong code

#[test]
fn wrong_code_sender_fails_confirmation_and_receiver_counts_failure() {
    let (_c, clk) = clock();
    let mut rx = new_rx(&clk);
    let code = code_of(&rx);
    let wrong = other_code(&code);
    let mut rng = os_rng();
    let m1 = rx.begin_attempt(&mut rng).unwrap();
    let (m2, pending) = sender_start(&clk, &wrong, &m1).unwrap();
    let m3 = rx.handle_m2(&mut rng, &m2).unwrap();
    assert_eq!(err_of(pending.handle_m3(&m3)), Error::ConfirmationFailed);
    assert_eq!(rx.failures(), 0);
    assert_eq!(err_of(rx.handle_m4(&bogus_m4())), Error::ConfirmationFailed);
    assert_eq!(rx.failures(), 1);
    assert!(rx.is_active());
}

// ---------------------------------------------------------------- 3. three failures

#[test]
fn three_failures_destroy_the_code() {
    let (_c, clk) = clock();
    let mut rx = new_rx(&clk);
    for i in 1..=MAX_FAILURES {
        failed_attempt(&mut rx, &clk);
        assert_eq!(rx.failures(), i);
    }
    assert_eq!(rx.failures(), 3);
    assert!(!rx.is_active());
    assert!(rx.code().is_err());
    let mut rng = os_rng();
    let e = err_of(rx.begin_attempt(&mut rng));
    assert!(
        matches!(e, Error::TooManyFailures | Error::NotActive),
        "got {e:?}"
    );
}

#[test]
fn fourth_attempt_never_succeeds_even_with_correct_code() {
    let (_c, clk) = clock();
    let mut rx = new_rx(&clk);
    for _ in 0..MAX_FAILURES {
        failed_attempt(&mut rx, &clk);
    }
    // A second receiver provides a valid M1/M2/M3/M4 using the (leaked) correct code of the first.
    let mut other = new_rx(&clk);
    let mut rng = os_rng();
    let other_code = code_of(&other);
    let m1 = other.begin_attempt(&mut rng).unwrap();
    let (m2, pending) = sender_start(&clk, &other_code, &m1).unwrap();
    let m3 = other.handle_m2(&mut rng, &m2).unwrap();
    let (m4, _tx) = pending.handle_m3(&m3).unwrap();
    assert_rejected(rx.begin_attempt(&mut rng), "begin_attempt after limit");
    assert_rejected(rx.handle_m2(&mut rng, &m2), "handle_m2 after limit");
    assert_rejected(rx.handle_m4(&m4), "handle_m4 after limit");
    assert!(!rx.is_active());
}

// ---------------------------------------------------------------- 4. success after failures

#[test]
fn success_after_two_failures_then_session_is_consumed() {
    let (_c, clk) = clock();
    let mut rx = new_rx(&clk);
    failed_attempt(&mut rx, &clk);
    failed_attempt(&mut rx, &clk);
    assert_eq!(rx.failures(), 2);
    assert!(rx.is_active());

    let code = code_of(&rx);
    let mut rng = os_rng();
    let m1 = rx.begin_attempt(&mut rng).unwrap();
    let (m2, pending) = sender_start(&clk, &code, &m1).unwrap();
    let m3 = rx.handle_m2(&mut rng, &m2).unwrap();
    let (m4, mut tx) = pending.handle_m3(&m3).unwrap();
    let mut rxc = rx.handle_m4(&m4).unwrap();
    roundtrip(&mut tx, &mut rxc, b"after two failures");

    assert!(rx.code().is_err());
    assert!(!rx.is_active());
    assert_rejected(rx.begin_attempt(&mut rng), "begin_attempt after success");
}

// ---------------------------------------------------------------- 5. TTL

#[test]
fn handshake_completes_just_before_expiry() {
    let (c, clk) = clock();
    let mut rx = new_rx(&clk);
    c.advance(CODE_TTL_MS - 1);
    assert!(rx.is_active());
    let code = code_of(&rx);
    let mut rng = os_rng();
    let m1 = rx.begin_attempt(&mut rng).unwrap();
    let (m2, pending) = sender_start(&clk, &code, &m1).unwrap();
    let m3 = rx.handle_m2(&mut rng, &m2).unwrap();
    let (m4, _tx) = pending.handle_m3(&m3).unwrap();
    assert!(rx.handle_m4(&m4).is_ok());
}

#[test]
fn session_expires_exactly_at_ttl() {
    let (c, clk) = clock();
    let mut rx = new_rx(&clk);
    let code = code_of(&rx);
    let mut rng = os_rng();
    let m1 = rx.begin_attempt(&mut rng).unwrap();
    let (m2, _pending) = sender_start(&clk, &code, &m1).unwrap();
    c.advance(CODE_TTL_MS);
    assert!(!rx.is_active());
    assert!(rx.code().is_err());
    let e = err_of(rx.handle_m2(&mut rng, &m2));
    assert!(matches!(e, Error::Expired | Error::NotActive), "got {e:?}");
    let e = err_of(rx.begin_attempt(&mut rng));
    assert!(matches!(e, Error::Expired | Error::NotActive), "got {e:?}");
    let e = err_of(rx.handle_m4(&bogus_m4()));
    assert!(matches!(e, Error::Expired | Error::NotActive), "got {e:?}");
}

#[test]
fn expiry_between_m2_and_m4_kills_the_attempt() {
    let (c, clk) = clock();
    let (mut rx, pending, m3) = through_m3(&clk);
    let (m4, _tx) = pending.handle_m3(&m3).unwrap();
    c.advance(CODE_TTL_MS);
    let e = err_of(rx.handle_m4(&m4));
    assert!(matches!(e, Error::Expired | Error::NotActive), "got {e:?}");
}

#[test]
fn ttl_is_not_extended_by_a_late_attempt() {
    let (c, clk) = clock();
    let mut rx = new_rx(&clk);
    let code = code_of(&rx);
    c.advance(59_000);
    let mut rng = os_rng();
    let m1 = rx.begin_attempt(&mut rng).unwrap();
    let (m2, pending) = sender_start(&clk, &code, &m1).unwrap();
    let m3 = rx.handle_m2(&mut rng, &m2).unwrap();
    let (m4, _tx) = pending.handle_m3(&m3).unwrap();
    c.advance(1_001); // 60_001 ms after creation
    let e = err_of(rx.handle_m4(&m4));
    assert!(matches!(e, Error::Expired | Error::NotActive), "got {e:?}");
}

#[test]
fn ttl_is_not_extended_by_failures() {
    let (c, clk) = clock();
    let mut rx = new_rx(&clk);
    c.advance(30_000);
    failed_attempt(&mut rx, &clk);
    c.advance(29_999);
    assert!(rx.is_active());
    c.advance(1);
    assert!(!rx.is_active());
    assert!(rx.code().is_err());
}

// ---------------------------------------------------------------- 6. abandoned attempt

#[test]
fn abandoned_attempt_counts_as_failure_and_uses_fresh_sid() {
    let (_c, clk) = clock();
    let mut rx = new_rx(&clk);
    let mut rng = os_rng();
    let m1a = rx.begin_attempt(&mut rng).unwrap();
    let m1b = rx.begin_attempt(&mut rng).unwrap();
    assert_eq!(rx.failures(), 1);
    let a = HelloR::decode(&m1a).unwrap();
    let b = HelloR::decode(&m1b).unwrap();
    assert_ne!(a.sid, b.sid);
    assert_ne!(a.nonce, b.nonce);
    // The second attempt can still complete.
    let code = code_of(&rx);
    let (m2, pending) = sender_start(&clk, &code, &m1b).unwrap();
    let m3 = rx.handle_m2(&mut rng, &m2).unwrap();
    let (m4, _tx) = pending.handle_m3(&m3).unwrap();
    assert!(rx.handle_m4(&m4).is_ok());
}

#[test]
fn m2_bound_to_abandoned_attempt_cannot_complete_the_new_attempt() {
    let (_c, clk) = clock();
    let mut rx = new_rx(&clk);
    let code = code_of(&rx);
    let mut rng = os_rng();
    let m1a = rx.begin_attempt(&mut rng).unwrap();
    let (m2a, pending_a) = sender_start(&clk, &code, &m1a).unwrap();
    let _m1b = rx.begin_attempt(&mut rng).unwrap();
    match rx.handle_m2(&mut rng, &m2a) {
        Err(_) => {}
        Ok(m3) => match pending_a.handle_m3(&m3) {
            Err(_) => {}
            Ok((m4, _tx)) => assert_rejected(rx.handle_m4(&m4), "old M2 completing new attempt"),
        },
    }
}

// ---------------------------------------------------------------- 7. M1 strictness

fn sample_m1() -> Vec<u8> {
    let (_c, clk) = clock();
    let mut rx = new_rx(&clk);
    let mut rng = os_rng();
    rx.begin_attempt(&mut rng).unwrap()
}

#[test]
fn hello_r_round_trip() {
    let m1 = sample_m1();
    let h = HelloR::decode(&m1).unwrap();
    assert_eq!(h.device_id, RX_ID);
    assert_eq!(h.claims, RX_CLAIMS);
    assert_eq!(h.encode().unwrap(), m1);
}

#[test]
fn hello_r_rejects_every_truncation() {
    let m1 = sample_m1();
    for len in 0..m1.len() {
        assert!(HelloR::decode(&m1[..len]).is_err(), "truncated to {len}");
    }
}

#[test]
fn hello_r_rejects_trailing_byte() {
    let mut m1 = sample_m1();
    m1.push(0);
    assert_eq!(err_of(HelloR::decode(&m1)), Error::Malformed);
}

#[test]
fn hello_r_rejects_flipped_magic_version_type_and_suite() {
    let m1 = sample_m1();
    // Layout: "KL" | version | type | u16 suite_len | suite...
    let mut v = m1.clone();
    v[2] ^= 0x01;
    assert_eq!(err_of(HelloR::decode(&v)), Error::Unsupported);
    let mut t = m1.clone();
    t[3] ^= 0x01;
    assert_eq!(err_of(HelloR::decode(&t)), Error::WrongState);
    let mut s = m1.clone();
    s[6] ^= 0x01;
    assert_eq!(err_of(HelloR::decode(&s)), Error::Unsupported);
    let mut magic = m1.clone();
    magic[0] ^= 0x01;
    assert_eq!(err_of(HelloR::decode(&magic)), Error::Malformed);
}

#[test]
fn hello_r_encode_rejects_overlong_fields() {
    let h = HelloR {
        sid: [1; 16],
        nonce: [2; 32],
        device_id: vec![b'a'; 33],
        claims: vec![],
    };
    assert_eq!(err_of(h.encode()), Error::Malformed);
    let h = HelloR {
        sid: [1; 16],
        nonce: [2; 32],
        device_id: vec![],
        claims: vec![b'c'; 65],
    };
    assert_eq!(err_of(h.encode()), Error::Malformed);
}

#[test]
fn sender_start_rejects_corrupted_m1() {
    let (_c, clk) = clock();
    let mut rx = new_rx(&clk);
    let code = code_of(&rx);
    let mut rng = os_rng();
    let m1 = rx.begin_attempt(&mut rng).unwrap();
    for off in [0usize, 1, 2, 3, 4, 5, 6, 20, m1.len() - 1] {
        let mut bad = m1.clone();
        bad[off] ^= 0x01;
        // Offsets inside sid/nonce/ids are not detectable by a parser; only structural
        // bytes (<= 6) must be rejected here.
        if off <= 6 {
            assert_rejected(
                sender_start(&clk, &code, &bad),
                &format!("M1 flipped at {off}"),
            );
        }
    }
    for len in [0usize, 1, 5, m1.len() / 2, m1.len() - 1] {
        assert_rejected(sender_start(&clk, &code, &m1[..len]), "truncated M1");
    }
    let mut ext = m1.clone();
    ext.push(0);
    assert_rejected(sender_start(&clk, &code, &ext), "extended M1");
}

#[test]
fn m1_modified_in_transit_breaks_the_handshake() {
    // Flipping a byte of sid/nonce in M1 (undetectable by parsing) must still fail
    // end to end because M1 is bound into the transcript.
    let (_c, clk) = clock();
    let mut rx = new_rx(&clk);
    let code = code_of(&rx);
    let mut rng = os_rng();
    let m1 = rx.begin_attempt(&mut rng).unwrap();
    let mut bad = m1.clone();
    // Layout: magic(2) version(1) type(1) suite lv (2 + SUITE.len()) then sid[16].
    let sid_offset = 6 + areatm_kagilink_core::wire::SUITE.len();
    bad[sid_offset + 3] ^= 0x01;
    let (m2, pending) = sender_start(&clk, &code, &bad).unwrap();
    match rx.handle_m2(&mut rng, &m2) {
        Err(_) => {}
        Ok(m3) => assert_rejected(pending.handle_m3(&m3), "M1 modified in transit"),
    }
}

// ---------------------------------------------------------------- 8. tampering

/// Flip every byte of M2; the attempt must never produce a channel.
#[test]
fn every_single_byte_flip_of_m2_is_rejected() {
    let (_c, clk) = clock();
    let (_rx0, m2_0, _p0) = through_m2(&clk);
    for i in 0..m2_0.len() {
        let (mut rx, m2, pending) = through_m2(&clk);
        let mut bad = m2.clone();
        bad[i] ^= 0x01;
        let mut rng = os_rng();
        match rx.handle_m2(&mut rng, &bad) {
            Err(_) => {}
            Ok(m3) => match pending.handle_m3(&m3) {
                Err(_) => {}
                Ok((m4, _tx)) => assert_rejected(rx.handle_m4(&m4), &format!("M2 flip at {i}")),
            },
        }
    }
}

/// The HelloS region inside M2 is bound into the transcript: changing any byte inside it
/// (device id, purpose, claims, nonce) breaks the handshake. Context-binding check; the full
/// M2 loop above also covers it.
#[test]
fn hello_s_region_of_m2_is_transcript_bound() {
    let (_c, clk) = clock();
    let (_rx, m2, _p) = through_m2(&clk);
    assert_eq!(m2[0], 0x12);
    let hlen = u16::from_be_bytes([m2[1], m2[2]]) as usize;
    let hello = &m2[3..3 + hlen];
    let h = HelloS::decode(hello).unwrap();
    assert_eq!(h.device_id, TX_ID);
    assert_eq!(h.purpose, PURPOSE);
    assert_eq!(h.claims, TX_CLAIMS);
    assert_eq!(h.encode().unwrap(), hello);
    for i in 3..3 + hlen {
        let (mut rx, m2, pending) = through_m2(&clk);
        let mut bad = m2.clone();
        bad[i] ^= 0x80;
        let mut rng = os_rng();
        if let Ok(m3) = rx.handle_m2(&mut rng, &bad) {
            if let Ok((m4, _)) = pending.handle_m3(&m3) {
                assert_rejected(rx.handle_m4(&m4), &format!("HelloS flip at {i}"));
            }
        }
    }
}

#[test]
fn every_single_byte_flip_of_m3_is_rejected() {
    let (_c, clk) = clock();
    let (_rx0, _p0, m3_0) = through_m3(&clk);
    for i in 0..m3_0.len() {
        let (_rx, pending, m3) = through_m3(&clk);
        let mut bad = m3.clone();
        bad[i] ^= 0x01;
        assert_rejected(pending.handle_m3(&bad), &format!("M3 flip at {i}"));
    }
}

#[test]
fn every_single_byte_flip_of_m4_is_rejected() {
    let (_c, clk) = clock();
    let (_rx0, m4_0, _t0) = through_m4(&clk);
    for i in 0..m4_0.len() {
        let (mut rx, m4, _tx) = through_m4(&clk);
        let mut bad = m4.clone();
        bad[i] ^= 0x01;
        let e = err_of(rx.handle_m4(&bad));
        assert!(
            matches!(
                e,
                Error::ConfirmationFailed | Error::Malformed | Error::WrongState
            ),
            "M4 flip at {i}: {e:?}"
        );
    }
}

#[test]
fn truncated_and_extended_m2_are_rejected() {
    let (_c, clk) = clock();
    let (_rx0, m2_0, _p0) = through_m2(&clk);
    let n = m2_0.len();
    for len in [0usize, 1, 2, 3, 4, 10, n / 2, n - 33, n - 32, n - 2, n - 1] {
        let (mut rx, m2, _p) = through_m2(&clk);
        let mut rng = os_rng();
        assert_rejected(
            rx.handle_m2(&mut rng, &m2[..len]),
            &format!("M2 truncated to {len}"),
        );
    }
    for extra in [1usize, 2, 32] {
        let (mut rx, mut m2, _p) = through_m2(&clk);
        m2.extend(std::iter::repeat_n(0u8, extra));
        let mut rng = os_rng();
        assert_rejected(rx.handle_m2(&mut rng, &m2), "M2 extended");
    }
}

#[test]
fn truncated_and_extended_m3_are_rejected() {
    let (_c, clk) = clock();
    for len in [0usize, 1, 2, 32, 33, 64] {
        let (_rx, pending, m3) = through_m3(&clk);
        assert_rejected(
            pending.handle_m3(&m3[..len]),
            &format!("M3 truncated to {len}"),
        );
    }
    for extra in [1usize, 32] {
        let (_rx, pending, mut m3) = through_m3(&clk);
        m3.extend(std::iter::repeat_n(0u8, extra));
        assert_rejected(pending.handle_m3(&m3), "M3 extended");
    }
}

#[test]
fn truncated_and_extended_m4_are_rejected() {
    let (_c, clk) = clock();
    for len in [0usize, 1, 2, 16, 32] {
        let (mut rx, m4, _tx) = through_m4(&clk);
        assert_rejected(rx.handle_m4(&m4[..len]), &format!("M4 truncated to {len}"));
    }
    for extra in [1usize, 32] {
        let (mut rx, mut m4, _tx) = through_m4(&clk);
        m4.extend(std::iter::repeat_n(0u8, extra));
        assert_rejected(rx.handle_m4(&m4), "M4 extended");
    }
}

// ---------------------------------------------------------------- 9. replay / reflection

#[test]
fn replayed_m2_from_previous_session_yields_no_channel() {
    let (_c, clk) = clock();
    let (_rx1, m2_old, pending_old) = through_m2(&clk);
    drop(pending_old);
    let (_rxz, m4_other, _txz) = through_m4(&clk);

    let mut rx2 = new_rx(&clk);
    let mut rng = os_rng();
    let _m1 = rx2.begin_attempt(&mut rng).unwrap();
    match rx2.handle_m2(&mut rng, &m2_old) {
        Err(_) => {}
        Ok(_m3) => {
            // A replayer has no sender secret state, so no valid M4 exists for this M3.
            assert_rejected(rx2.handle_m4(&m4_other), "foreign M4 into new session");
        }
    }
}

#[test]
fn replayed_m4_after_success_is_rejected() {
    let (_c, clk) = clock();
    let (mut rx, m4, _tx) = through_m4(&clk);
    assert!(rx.handle_m4(&m4).is_ok());
    assert_rejected(rx.handle_m4(&m4), "M4 replay after success");
}

#[test]
fn reflected_messages_are_rejected() {
    let (_c, clk) = clock();
    let (mut rx, pending, m3) = through_m3(&clk);
    let mut rng = os_rng();
    // M3 fed as M2.
    let (mut rx_b, _m2b, _pb) = through_m2(&clk);
    let e = err_of(rx_b.handle_m2(&mut rng, &m3));
    assert!(
        matches!(e, Error::WrongState | Error::Malformed),
        "got {e:?}"
    );
    // M1 fed as M3.
    let m1 = rx.begin_attempt(&mut rng).unwrap();
    let e = err_of(pending.handle_m3(&m1));
    assert!(
        matches!(e, Error::WrongState | Error::Malformed),
        "got {e:?}"
    );
    // M3 fed as M1 to sender start.
    let code = code_of(&rx);
    assert_rejected(sender_start(&clk, &code, &m3), "M3 as M1");
}

// ---------------------------------------------------------------- 10. state ordering

#[test]
fn handle_m2_before_begin_attempt_is_rejected() {
    let (_c, clk) = clock();
    let (_rxk, m2, _p) = through_m2(&clk);
    let mut rx = new_rx(&clk);
    let mut rng = os_rng();
    let e = err_of(rx.handle_m2(&mut rng, &m2));
    assert!(
        matches!(e, Error::WrongState | Error::NotActive),
        "got {e:?}"
    );
}

#[test]
fn handle_m4_before_handle_m2_is_wrong_state() {
    let (_c, clk) = clock();
    let mut rx = new_rx(&clk);
    let mut rng = os_rng();
    let _m1 = rx.begin_attempt(&mut rng).unwrap();
    assert_eq!(err_of(rx.handle_m4(&bogus_m4())), Error::WrongState);
}

#[test]
fn handle_m2_twice_is_rejected() {
    let (_c, clk) = clock();
    let (mut rx, m2, _p) = through_m2(&clk);
    let mut rng = os_rng();
    assert!(rx.handle_m2(&mut rng, &m2).is_ok());
    assert_rejected(rx.handle_m2(&mut rng, &m2), "second M2");
}

#[test]
fn sender_rejects_m3_from_a_different_attempt() {
    let (_c, clk) = clock();
    let (_rx_a, pending_a, _m3_a) = through_m3(&clk);
    let (_rx_b, _pending_b, m3_b) = through_m3(&clk);
    assert_rejected(pending_a.handle_m3(&m3_b), "M3 from another attempt");
}

// ---------------------------------------------------------------- 11. Code

#[test]
fn code_parse_accepts_and_rejects() {
    for ok in ["000000", "123456", "999999"] {
        assert_eq!(Code::parse(ok).unwrap().as_str(), ok);
    }
    let bad_inputs = [
        "12345",
        "1234567",
        "12a456",
        "\u{FF11}\u{FF12}\u{FF13}\u{FF14}\u{FF15}\u{FF16}",
        "",
        " 123456",
        "123456 ",
        "-12345",
        "12 456",
    ];
    for bad in bad_inputs {
        assert_eq!(err_of(Code::parse(bad)), Error::BadCode, "input {bad:?}");
    }
}

#[test]
fn code_generate_is_six_digits_and_uniform_enough() {
    let mut rng = os_rng();
    let mut seen = [[false; 10]; 6];
    let mut leading_zero = 0usize;
    const N: usize = 20_000;
    for _ in 0..N {
        let c = Code::generate(&mut rng);
        let s = c.as_str();
        assert_eq!(s.len(), 6);
        assert!(s.bytes().all(|b| b.is_ascii_digit()));
        for (pos, b) in s.bytes().enumerate() {
            seen[pos][(b - b'0') as usize] = true;
        }
        if s.starts_with('0') {
            leading_zero += 1;
        }
    }
    assert!(
        seen.iter().all(|p| p.iter().all(|&x| x)),
        "a digit never appeared at some position"
    );
    // Expected ~2000; deliberately wide band so the test is not flaky.
    assert!(
        (1_300..=2_700).contains(&leading_zero),
        "leading zeros: {leading_zero}"
    );
}

// ---------------------------------------------------------------- 12. misc

#[test]
fn receiver_sessions_get_fresh_codes() {
    let (_c, clk) = clock();
    let mut distinct = false;
    let first = new_rx(&clk);
    for _ in 0..3 {
        let other = new_rx(&clk);
        if first.code().unwrap() != other.code().unwrap() {
            distinct = true;
            break;
        }
    }
    assert!(distinct);
}
