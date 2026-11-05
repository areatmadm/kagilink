#![no_main]
//! Arbitrary fragments into an authenticated channel: never panic and never authenticate.
//! The first input byte selects the fragment kind of attack; the rest is the fragment.
use areatm_kagilink_core::*;
use libfuzzer_sys::fuzz_target;
use std::sync::Arc;

fn pair() -> (SecureChannel, SecureChannel) {
    let clock: Arc<dyn Clock> = Arc::new(ManualClock::new());
    let mut rng = os_rng();
    let mut rx = ReceiverSession::new(clock.clone(), &mut rng, b"rx", b"c").expect("receiver");
    let code = Code::parse(rx.code().expect("code")).expect("parse");
    let m1 = rx.begin_attempt(&mut rng).expect("m1");
    let (m2, pending) =
        SenderPending::start(clock, &mut rng, &code, &m1, b"tx", b"p", b"c").expect("m2");
    let m3 = rx.handle_m2(&mut rng, &m2).expect("m3");
    let (m4, tx) = pending.handle_m3(&m3).expect("m4");
    let rxc = rx.handle_m4(&m4).expect("channel");
    (tx, rxc)
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let (mut tx, mut rx) = pair();
    rx.set_ready_to_receive(true);
    if mode & 1 == 0 {
        // Raw bytes as a fragment.
        if let Ok(Some(_)) = rx.open_fragment(rest) {
            panic!("raw fuzz bytes authenticated");
        }
    } else {
        // Corrupt a genuinely sealed fragment: it must never open, unless left unchanged.
        tx.approve_one_transfer();
        let frags = tx.seal(kind::SECRET, b"synthetic-secret", 8).expect("seal");
        let idx = (mode as usize >> 1) % frags.len();
        let mut bad = frags[idx].clone();
        let changed = !rest.is_empty() && {
            let pos = rest[0] as usize % bad.len();
            let delta = rest.get(1).copied().unwrap_or(1).max(1);
            bad[pos] ^= delta;
            true
        };
        let mut delivered = false;
        for (i, f) in frags.iter().enumerate() {
            let input = if i == idx { &bad } else { f };
            match rx.open_fragment(input) {
                Ok(Some(_)) => delivered = true,
                Ok(None) => {}
                Err(_) => break,
            }
        }
        if changed {
            assert!(!delivered, "corrupted fragment produced a message");
        }
    }
});
