#![no_main]
//! Arbitrary bytes into every handshake slot: never panic, and never complete a handshake.
use areatm_kagilink_core::*;
use libfuzzer_sys::fuzz_target;
use std::sync::Arc;

fn clock() -> Arc<dyn Clock> {
    Arc::new(ManualClock::new())
}

fuzz_target!(|data: &[u8]| {
    let mut rng = os_rng();
    let mut rx = ReceiverSession::new(clock(), &mut rng, b"rx", b"c").expect("receiver");
    let code = Code::parse(rx.code().expect("code")).expect("parse");
    let m1 = rx.begin_attempt(&mut rng).expect("m1");

    // Receiver slots.
    let _ = rx.handle_m2(&mut rng, data);
    assert!(rx.handle_m4(data).is_err(), "fuzz bytes released a channel");

    // Sender slots.
    let _ = SenderPending::start(clock(), &mut rng, &code, data, b"tx", b"p", b"c");
    if let Ok((_m2, pending)) = SenderPending::start(clock(), &mut rng, &code, &m1, b"tx", b"p", b"c")
    {
        assert!(pending.handle_m3(data).is_err(), "fuzz bytes accepted as M3");
    }
});
