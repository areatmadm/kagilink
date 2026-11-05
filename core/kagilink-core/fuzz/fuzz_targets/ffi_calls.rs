#![no_main]
//! Drives the C ABI exports with arbitrary bytes, lengths and output capacities.
//! Property: no call may report KL_ERR_PANIC (a caught panic would otherwise hide a bug),
//! and fuzz bytes must never produce an authenticated message or a completed handshake.
use areatm_kagilink_ffi::kl_status_t::*;
use areatm_kagilink_ffi::*;
use libfuzzer_sys::fuzz_target;
use std::ptr::{null, null_mut};

fn ok(st: kl_status_t) {
    assert!(st != KL_ERR_PANIC, "an export reported KL_ERR_PANIC");
}

fn cap_for(sel: u8) -> usize {
    match sel % 4 {
        0 => 0,
        1 => 1,
        2 => 64,
        _ => 4096,
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&sel, rest)) = data.split_first() else {
        return;
    };
    let cap = cap_for(sel);
    // SAFETY: every pointer passed below is either null with a matching zero/ignored length
    // or points to a live buffer of at least the stated length; handles come from the library
    // and are freed exactly once at the end.
    unsafe {
        let mut rx: *mut kl_receiver_t = null_mut();
        ok(kl_receiver_new(b"rx".as_ptr(), 2, null(), 0, &mut rx));
        assert!(!rx.is_null());
        let mut code = [0u8; 8];
        let mut code_len = code.len();
        ok(kl_receiver_code(rx, code.as_mut_ptr(), &mut code_len));
        let mut m1 = vec![0u8; 4096];
        let mut m1_len = m1.len();
        ok(kl_receiver_begin_attempt(rx, m1.as_mut_ptr(), &mut m1_len));

        // Receiver slots with fuzz bytes and a fuzzed output capacity.
        let mut out = vec![0u8; 4096];
        let mut out_len = cap;
        ok(kl_receiver_handle_m2(rx, rest.as_ptr(), rest.len(), out.as_mut_ptr(), &mut out_len));
        let mut ch: *mut kl_channel_t = null_mut();
        let st = kl_receiver_finish(rx, rest.as_ptr(), rest.len(), &mut ch);
        ok(st);
        assert!(ch.is_null() && st != KL_OK, "fuzz bytes completed a handshake");

        // Sender slots: fuzz bytes as the code and as M1, as M3.
        let mut sender: *mut kl_sender_t = null_mut();
        let mut m2 = vec![0u8; 4096];
        let mut m2_len = cap;
        let code_ptr = if rest.len() >= 6 { rest.as_ptr() } else { code.as_ptr() };
        ok(kl_sender_start(
            code_ptr, 6, rest.as_ptr(), rest.len(), b"tx".as_ptr(), 2, b"p".as_ptr(), 1,
            null(), 0, m2.as_mut_ptr(), &mut m2_len, &mut sender,
        ));
        kl_sender_free(sender);

        let mut sender: *mut kl_sender_t = null_mut();
        let mut m2_len = m2.len();
        ok(kl_sender_start(
            code.as_ptr(), 6, m1.as_ptr(), m1_len, b"tx".as_ptr(), 2, b"p".as_ptr(), 1,
            null(), 0, m2.as_mut_ptr(), &mut m2_len, &mut sender,
        ));
        if !sender.is_null() {
            let mut m4 = vec![0u8; 4096];
            let mut m4_len = cap;
            let mut tx_ch: *mut kl_channel_t = null_mut();
            let st = kl_sender_finish(sender, rest.as_ptr(), rest.len(), m4.as_mut_ptr(), &mut m4_len, &mut tx_ch);
            ok(st);
            assert!(tx_ch.is_null() && st != KL_OK, "fuzz bytes accepted as M3");
            kl_channel_free(tx_ch);
        }
        kl_sender_free(sender);
        kl_receiver_free(rx);

        // Channel slots on a genuinely established pair.
        let mut rx: *mut kl_receiver_t = null_mut();
        ok(kl_receiver_new(b"rx".as_ptr(), 2, null(), 0, &mut rx));
        let mut code = [0u8; 8];
        let mut code_len = code.len();
        ok(kl_receiver_code(rx, code.as_mut_ptr(), &mut code_len));
        let mut m1 = vec![0u8; 4096];
        let mut m1_len = m1.len();
        ok(kl_receiver_begin_attempt(rx, m1.as_mut_ptr(), &mut m1_len));
        let mut sender: *mut kl_sender_t = null_mut();
        let mut m2 = vec![0u8; 4096];
        let mut m2_len = m2.len();
        ok(kl_sender_start(code.as_ptr(), 6, m1.as_ptr(), m1_len, b"tx".as_ptr(), 2, b"p".as_ptr(), 1, null(), 0, m2.as_mut_ptr(), &mut m2_len, &mut sender));
        let mut m3 = vec![0u8; 4096];
        let mut m3_len = m3.len();
        ok(kl_receiver_handle_m2(rx, m2.as_ptr(), m2_len, m3.as_mut_ptr(), &mut m3_len));
        let mut m4 = vec![0u8; 4096];
        let mut m4_len = m4.len();
        let mut tx_ch: *mut kl_channel_t = null_mut();
        ok(kl_sender_finish(sender, m3.as_ptr(), m3_len, m4.as_mut_ptr(), &mut m4_len, &mut tx_ch));
        let mut rx_ch: *mut kl_channel_t = null_mut();
        ok(kl_receiver_finish(rx, m4.as_ptr(), m4_len, &mut rx_ch));
        assert!(!tx_ch.is_null() && !rx_ch.is_null(), "valid handshake failed");

        ok(kl_channel_set_ready_to_receive(rx_ch, 1));
        let mut plain = vec![0u8; 4096];
        let mut plain_len = cap;
        let (mut kind, mut complete) = (0u8, 0u8);
        let st = kl_channel_open_fragment(rx_ch, rest.as_ptr(), rest.len(), plain.as_mut_ptr(), &mut plain_len, &mut kind, &mut complete);
        ok(st);
        assert!(complete == 0, "fuzz bytes authenticated as a message");

        // Seal with fuzzed kind, fragment size, plaintext and output capacity.
        if let Some((&k, tail)) = rest.split_first() {
            let (frag, pt) = tail.split_first().map_or((8usize, &[][..]), |(f, p)| (*f as usize, p));
            ok(kl_channel_approve_one_transfer(tx_ch));
            let mut sealed = vec![0u8; 8192];
            let mut sealed_len = cap.max(1).min(sealed.len());
            let mut count = 0usize;
            ok(kl_channel_seal(tx_ch, k, pt.as_ptr(), pt.len(), frag, sealed.as_mut_ptr(), &mut sealed_len, &mut count));
        }
        ok(kl_channel_close(tx_ch));
        kl_channel_free(tx_ch);
        kl_channel_free(rx_ch);
    }
});
