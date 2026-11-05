//! Deterministic mutation tests (NOT coverage-guided fuzzing, NOT an audit).
//!
//! A fixed-seed generator mutates valid handshake messages and transport fragments
//! (bit flips, byte replacement, truncation, extension, splicing) and random bytes. For
//! every mutation the code must not panic and must never complete a handshake or release
//! an authenticated message from altered input. Synthetic data only.

use areatm_kagilink_core::wire::{HelloR, HelloS};
use areatm_kagilink_core::*;
use std::sync::Arc;

const ROUNDS: usize = 300;

/// Small xorshift64* generator; fixed seeds keep every run reproducible.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn nonzero_byte(&mut self) -> u8 {
        (self.below(255) + 1) as u8
    }
}

/// Always returns bytes different from the input.
fn mutate(rng: &mut Rng, input: &[u8]) -> Vec<u8> {
    let mut v = input.to_vec();
    match rng.below(5) {
        0 if !v.is_empty() => {
            let i = rng.below(v.len());
            v[i] ^= 1 << rng.below(8);
        }
        1 if !v.is_empty() => {
            let i = rng.below(v.len());
            v[i] ^= rng.nonzero_byte();
        }
        2 if !v.is_empty() => {
            let keep = rng.below(v.len());
            v.truncate(keep);
        }
        3 => {
            for _ in 0..=rng.below(8) {
                v.push(rng.next() as u8);
            }
        }
        _ => {
            if v.len() >= 2 {
                // Swap two different bytes' positions only if that changes the data.
                let (a, b) = (rng.below(v.len()), rng.below(v.len()));
                if v[a] != v[b] {
                    v.swap(a, b);
                    return v;
                }
            }
            v.push(rng.next() as u8);
        }
    }
    v
}

fn random_bytes(rng: &mut Rng, max: usize) -> Vec<u8> {
    let n = rng.below(max + 1);
    (0..n).map(|_| rng.next() as u8).collect()
}

fn clock() -> Arc<dyn Clock> {
    Arc::new(ManualClock::new())
}

struct Flow {
    rx: ReceiverSession,
    m1: Vec<u8>,
    code: Code,
}

fn start_flow() -> Flow {
    let mut rng = os_rng();
    let mut rx = ReceiverSession::new(clock(), &mut rng, b"rx-device-01", b"rx-claims").unwrap();
    let code = Code::parse(rx.code().unwrap()).unwrap();
    let m1 = rx.begin_attempt(&mut rng).unwrap();
    Flow { rx, m1, code }
}

fn sender(code: &Code, m1: &[u8]) -> Result<(Vec<u8>, SenderPending)> {
    let mut rng = os_rng();
    SenderPending::start(
        clock(),
        &mut rng,
        code,
        m1,
        b"tx-device-01",
        b"synthetic-login",
        b"tx-claims",
    )
}

#[test]
fn baseline_handshake_still_completes() {
    let mut f = start_flow();
    let (m2, pending) = sender(&f.code, &f.m1).unwrap();
    let mut rng = os_rng();
    let m3 = f.rx.handle_m2(&mut rng, &m2).unwrap();
    let (m4, _tx) = pending.handle_m3(&m3).unwrap();
    assert!(f.rx.handle_m4(&m4).is_ok());
}

#[test]
fn hello_decoders_never_panic_and_reject_mutations() {
    let mut rng = Rng(0x1234_5678_9ABC_DEF1);
    let hello_r = HelloR {
        sid: [7; 16],
        nonce: [9; 32],
        device_id: b"rx-device-01".to_vec(),
        claims: b"rx-claims".to_vec(),
    }
    .encode()
    .unwrap();
    let hello_s = HelloS {
        nonce: [3; 32],
        device_id: b"tx-device-01".to_vec(),
        purpose: b"synthetic-login".to_vec(),
        claims: b"tx-claims".to_vec(),
    }
    .encode()
    .unwrap();
    for _ in 0..ROUNDS {
        let m = mutate(&mut rng, &hello_r);
        // Mutations inside the free-form fields can still decode; the property here is
        // "no panic" plus "re-encoding is canonical", so a decoded value encodes back to
        // exactly the mutated bytes.
        if let Ok(d) = HelloR::decode(&m) {
            assert_eq!(d.encode().unwrap(), m, "non-canonical HelloR accepted");
        }
        let m = mutate(&mut rng, &hello_s);
        if let Ok(d) = HelloS::decode(&m) {
            assert_eq!(d.encode().unwrap(), m, "non-canonical HelloS accepted");
        }
        let junk = random_bytes(&mut rng, 300);
        if let Ok(d) = HelloR::decode(&junk) {
            assert_eq!(d.encode().unwrap(), junk);
        }
        if let Ok(d) = HelloS::decode(&junk) {
            assert_eq!(d.encode().unwrap(), junk);
        }
    }
}

#[test]
fn mutated_m1_never_completes_a_handshake() {
    let mut rng = Rng(0xA1);
    for _ in 0..ROUNDS {
        let mut f = start_flow();
        let bad_m1 = mutate(&mut rng, &f.m1);
        let Ok((m2, pending)) = sender(&f.code, &bad_m1) else {
            continue;
        };
        let mut r = os_rng();
        // The receiver may or may not accept this M2; if it answers, the sender must
        // reject (it hashed a different M1 into the transcript).
        if let Ok(m3) = f.rx.handle_m2(&mut r, &m2) {
            assert!(
                pending.handle_m3(&m3).is_err(),
                "handshake completed from a mutated M1"
            );
        }
    }
}

#[test]
fn mutated_m2_never_completes_a_handshake() {
    let mut rng = Rng(0xB2);
    for _ in 0..ROUNDS {
        let mut f = start_flow();
        let (m2, pending) = sender(&f.code, &f.m1).unwrap();
        let bad = mutate(&mut rng, &m2);
        let mut r = os_rng();
        if let Ok(m3) = f.rx.handle_m2(&mut r, &bad) {
            assert!(
                pending.handle_m3(&m3).is_err(),
                "handshake completed from a mutated M2"
            );
        }
    }
}

#[test]
fn mutated_m3_is_always_rejected_by_sender() {
    let mut rng = Rng(0xC3);
    for _ in 0..ROUNDS {
        let mut f = start_flow();
        let (m2, pending) = sender(&f.code, &f.m1).unwrap();
        let mut r = os_rng();
        let m3 = f.rx.handle_m2(&mut r, &m2).unwrap();
        let bad = mutate(&mut rng, &m3);
        assert!(pending.handle_m3(&bad).is_err(), "mutated M3 accepted");
    }
}

#[test]
fn mutated_m4_never_releases_a_channel() {
    let mut rng = Rng(0xD4);
    for _ in 0..ROUNDS {
        let mut f = start_flow();
        let (m2, pending) = sender(&f.code, &f.m1).unwrap();
        let mut r = os_rng();
        let m3 = f.rx.handle_m2(&mut r, &m2).unwrap();
        let (m4, _tx) = pending.handle_m3(&m3).unwrap();
        let bad = mutate(&mut rng, &m4);
        assert!(f.rx.handle_m4(&bad).is_err(), "mutated M4 accepted");
        assert_eq!(f.rx.failures(), 1, "failure not counted");
    }
}

#[test]
fn random_bytes_never_panic_in_any_handshake_slot() {
    let mut rng = Rng(0xE5);
    for _ in 0..ROUNDS {
        let junk = random_bytes(&mut rng, 600);
        let mut f = start_flow();
        let mut r = os_rng();
        let _ = f.rx.handle_m2(&mut r, &junk);
        let mut f = start_flow();
        assert!(f.rx.handle_m4(&junk).is_err());
        let f = start_flow();
        let _ = sender(&f.code, &junk).map(|_| ());
        // Sender handle_m3 with junk.
        let f = start_flow();
        let (_m2, pending) = sender(&f.code, &f.m1).unwrap();
        assert!(pending.handle_m3(&junk).is_err());
    }
}

fn channel_pair() -> (SecureChannel, SecureChannel) {
    let mut f = start_flow();
    let (m2, pending) = sender(&f.code, &f.m1).unwrap();
    let mut r = os_rng();
    let m3 = f.rx.handle_m2(&mut r, &m2).unwrap();
    let (m4, tx) = pending.handle_m3(&m3).unwrap();
    let rx = f.rx.handle_m4(&m4).unwrap();
    (tx, rx)
}

#[test]
fn mutated_fragments_never_yield_a_message() {
    let mut rng = Rng(0xF6);
    for round in 0..ROUNDS {
        let (mut tx, mut rx) = channel_pair();
        rx.set_ready_to_receive(true);
        tx.approve_one_transfer();
        let len = 1 + rng.below(900);
        let plain: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
        // Keep the fragment count within the 256-fragment limit (16-byte GCM tag included).
        let min_payload = (len + 16).div_ceil(256);
        let payload = min_payload + rng.below(257 - min_payload);
        let frags = tx.seal(kind::SECRET, &plain, payload).unwrap();
        let which = rng.below(frags.len());
        let mut delivered = None;
        for (i, frag) in frags.iter().enumerate() {
            let bytes = if i == which {
                mutate(&mut rng, frag)
            } else {
                frag.clone()
            };
            match rx.open_fragment(&bytes) {
                Ok(Some(m)) => delivered = Some(m),
                Ok(None) => {}
                Err(_) => break,
            }
        }
        assert!(
            delivered.is_none(),
            "round {round}: mutated fragment produced an authenticated message"
        );
    }
}

#[test]
fn random_fragments_never_panic_or_authenticate() {
    let mut rng = Rng(0x17);
    let (_tx, mut rx) = channel_pair();
    rx.set_ready_to_receive(true);
    for _ in 0..ROUNDS {
        let junk = random_bytes(&mut rng, 400);
        if let Ok(Some(_)) = rx.open_fragment(&junk) {
            panic!("random bytes authenticated");
        }
        if rx.is_closed() {
            // Fail-closed: once closed, everything stays rejected.
            assert!(rx.open_fragment(&junk).is_err());
            break;
        }
    }
}

#[test]
fn replayed_and_reordered_fragments_are_rejected() {
    let (mut tx, mut rx) = channel_pair();
    rx.set_ready_to_receive(true);
    let frags = tx.seal(kind::CONTROL, &[0x5A; 700], 100).unwrap();
    assert!(frags.len() > 2);
    // Skip fragment 0, deliver 1 first: strict in-order reassembly must refuse.
    assert!(rx.open_fragment(&frags[1]).is_err());
    assert!(rx.is_closed());

    let (mut tx, mut rx) = channel_pair();
    rx.set_ready_to_receive(true);
    let frags = tx.seal(kind::CONTROL, &[0x5A; 50], 100).unwrap();
    let mut msg = None;
    for f in &frags {
        msg = rx.open_fragment(f).unwrap();
    }
    assert!(msg.is_some());
    // Replay of the same complete message.
    assert!(rx.open_fragment(&frags[0]).is_err());
}
