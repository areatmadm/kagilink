//! Tests through the extern "C" surface. Synthetic data only; no real devices or BLE.
#![allow(non_camel_case_types)]

use areatm_kagilink_ffi::kl_status_t::*;
use areatm_kagilink_ffi::*;
use std::ptr::{null, null_mut};

const CONTROL: u8 = 1;
const SECRET: u8 = 2;

struct Pair {
    r: *mut kl_receiver_t,
    code: [u8; 6],
    m1: Vec<u8>,
}

fn receiver_with_m1() -> Pair {
    unsafe {
        let mut r = null_mut();
        assert_eq!(
            kl_receiver_new(b"rx-dev".as_ptr(), 6, b"claims".as_ptr(), 6, &mut r),
            KL_OK
        );
        let mut code = [0u8; 6];
        let mut n = 6usize;
        assert_eq!(kl_receiver_code(r, code.as_mut_ptr(), &mut n), KL_OK);
        assert_eq!(n, 6);
        assert!(code.iter().all(u8::is_ascii_digit));
        let mut m1 = vec![0u8; 512];
        let mut n = m1.len();
        assert_eq!(kl_receiver_begin_attempt(r, m1.as_mut_ptr(), &mut n), KL_OK);
        m1.truncate(n);
        Pair { r, code, m1 }
    }
}

unsafe fn sender_start(code: &[u8], m1: &[u8]) -> (kl_status_t, Vec<u8>, *mut kl_sender_t) {
    let mut m2 = vec![0u8; 512];
    let mut n = m2.len();
    let mut s = null_mut();
    let rc = unsafe {
        kl_sender_start(
            code.as_ptr(),
            code.len(),
            m1.as_ptr(),
            m1.len(),
            b"tx-dev".as_ptr(),
            6,
            b"login".as_ptr(),
            5,
            b"sclaims".as_ptr(),
            7,
            m2.as_mut_ptr(),
            &mut n,
            &mut s,
        )
    };
    m2.truncate(n);
    (rc, m2, s)
}

type Quad = (
    *mut kl_receiver_t,
    *mut kl_sender_t,
    *mut kl_channel_t,
    *mut kl_channel_t,
);

/// Runs the full handshake; returns (receiver, sender, rx channel, tx channel).
unsafe fn handshake() -> Quad {
    unsafe {
        let p = receiver_with_m1();
        let (rc, m2, s) = sender_start(&p.code, &p.m1);
        assert_eq!(rc, KL_OK);
        let mut m3 = [0u8; 128];
        let mut n = m3.len();
        assert_eq!(
            kl_receiver_handle_m2(p.r, m2.as_ptr(), m2.len(), m3.as_mut_ptr(), &mut n),
            KL_OK
        );
        let mut m4 = [0u8; 64];
        let mut n4 = m4.len();
        let mut tx = null_mut();
        assert_eq!(
            kl_sender_finish(s, m3.as_ptr(), n, m4.as_mut_ptr(), &mut n4, &mut tx),
            KL_OK
        );
        let mut rx = null_mut();
        assert_eq!(kl_receiver_finish(p.r, m4.as_ptr(), n4, &mut rx), KL_OK);
        assert!(!rx.is_null() && !tx.is_null());
        (p.r, s, rx, tx)
    }
}

/// Feed packed fragments into `rx`; returns (kind, plaintext) when complete.
unsafe fn deliver(rx: *mut kl_channel_t, packed: &[u8], count: usize) -> (u8, Vec<u8>) {
    let mut off = 0;
    let mut result = None;
    for i in 0..count {
        let l = u32::from_be_bytes(packed[off..off + 4].try_into().unwrap()) as usize;
        let f = &packed[off + 4..off + 4 + l];
        off += 4 + l;
        let mut out = vec![0u8; 8192];
        let mut n = out.len();
        let (mut kind, mut done) = (0u8, 0u8);
        let rc = unsafe {
            kl_channel_open_fragment(
                rx,
                f.as_ptr(),
                f.len(),
                out.as_mut_ptr(),
                &mut n,
                &mut kind,
                &mut done,
            )
        };
        assert_eq!(rc, KL_OK);
        assert_eq!(done == 1, i + 1 == count);
        if done == 1 {
            out.truncate(n);
            result = Some((kind, out));
        }
    }
    assert_eq!(off, packed.len());
    result.expect("complete")
}

unsafe fn seal(c: *mut kl_channel_t, kind: u8, pt: &[u8], frag: usize) -> (Vec<u8>, usize) {
    let mut out = vec![0u8; 16384];
    let mut n = out.len();
    let mut count = 0usize;
    let rc = unsafe {
        kl_channel_seal(
            c,
            kind,
            pt.as_ptr(),
            pt.len(),
            frag,
            out.as_mut_ptr(),
            &mut n,
            &mut count,
        )
    };
    assert_eq!(rc, KL_OK);
    out.truncate(n);
    (out, count)
}

#[test]
fn abi_version_is_one() {
    assert_eq!(kl_abi_version(), 1);
}

#[test]
fn full_handshake_and_secret_round_trip() {
    unsafe {
        let (r, s, rx, tx) = handshake();
        let (packed, count) = seal(tx, CONTROL, b"synthetic control", 256);
        let (k, p) = deliver(rx, &packed, count);
        assert_eq!((k, p.as_slice()), (CONTROL, &b"synthetic control"[..]));

        // SECRET without approval is refused and consumes nothing.
        let mut out = vec![0u8; 1024];
        let mut n = out.len();
        let mut cnt = 0usize;
        let pt = b"synthetic-secret-value-0123456789";
        assert_eq!(
            kl_channel_seal(
                tx,
                SECRET,
                pt.as_ptr(),
                pt.len(),
                20,
                out.as_mut_ptr(),
                &mut n,
                &mut cnt
            ),
            KL_ERR_NOT_APPROVED
        );
        assert_eq!(kl_channel_approve_one_transfer(tx), KL_OK);
        assert_eq!(kl_channel_set_ready_to_receive(rx, 1), KL_OK);
        let (packed, count) = seal(tx, SECRET, pt, 20);
        assert!(count > 1);
        let (k, mut p) = deliver(rx, &packed, count);
        assert_eq!((k, p.as_slice()), (SECRET, &pt[..]));
        kl_secure_wipe(p.as_mut_ptr(), p.len());
        assert!(p.iter().all(|b| *b == 0));

        // Approval was consumed by the first SECRET.
        let mut n = out.len();
        assert_eq!(
            kl_channel_seal(
                tx,
                SECRET,
                pt.as_ptr(),
                pt.len(),
                20,
                out.as_mut_ptr(),
                &mut n,
                &mut cnt
            ),
            KL_ERR_NOT_APPROVED
        );
        assert_eq!(kl_channel_is_closed(tx), 0);
        assert_eq!(kl_channel_close(tx), KL_OK);
        assert_eq!(kl_channel_is_closed(tx), 1);
        kl_channel_free(tx);
        kl_channel_free(rx);
        kl_sender_free(s);
        kl_receiver_free(r);
    }
}

#[test]
fn secret_refused_when_receiver_not_ready() {
    unsafe {
        let (r, s, rx, tx) = handshake();
        assert_eq!(kl_channel_approve_one_transfer(tx), KL_OK);
        let (packed, _) = seal(tx, SECRET, b"synthetic", 256);
        let f = &packed[4..];
        let mut out = [0u8; 64];
        let mut n = out.len();
        let (mut kind, mut done) = (0u8, 0u8);
        let rc = kl_channel_open_fragment(
            rx,
            f.as_ptr(),
            f.len(),
            out.as_mut_ptr(),
            &mut n,
            &mut kind,
            &mut done,
        );
        assert_eq!(rc, KL_ERR_NOT_APPROVED);
        assert_eq!(kl_channel_is_closed(rx), 1);
        kl_channel_free(tx);
        kl_channel_free(rx);
        kl_sender_free(s);
        kl_receiver_free(r);
    }
}

#[test]
fn wrong_code_fails_and_counts_failure() {
    unsafe {
        let p = receiver_with_m1();
        let mut bad = p.code;
        bad[0] = if bad[0] == b'0' { b'1' } else { b'0' };
        let (rc, m2, s) = sender_start(&bad, &p.m1);
        assert_eq!(rc, KL_OK);
        let mut m3 = [0u8; 128];
        let mut n = m3.len();
        assert_eq!(
            kl_receiver_handle_m2(p.r, m2.as_ptr(), m2.len(), m3.as_mut_ptr(), &mut n),
            KL_OK
        );
        let mut m4 = [0u8; 64];
        let mut n4 = m4.len();
        let mut tx = null_mut();
        let rc = kl_sender_finish(s, m3.as_ptr(), n, m4.as_mut_ptr(), &mut n4, &mut tx);
        assert_eq!(rc, KL_ERR_CONFIRMATION_FAILED);
        assert!(tx.is_null());
        // The pending state is consumed even on failure; the shell can only be freed.
        let rc = kl_sender_finish(s, m3.as_ptr(), n, m4.as_mut_ptr(), &mut n4, &mut tx);
        assert_eq!(rc, KL_ERR_WRONG_STATE);
        kl_sender_free(s);
        kl_sender_free(null_mut());

        // Failure 1: a bogus M4 on the open attempt.
        let mut rx = null_mut();
        let bogus = [0x14u8; 33];
        assert_eq!(
            kl_receiver_finish(p.r, bogus.as_ptr(), bogus.len(), &mut rx),
            KL_ERR_CONFIRMATION_FAILED
        );
        assert!(rx.is_null());
        // Failures 2 and 3: junk M2 after fresh attempts.
        for _ in 0..2 {
            let mut m1 = vec![0u8; 512];
            let mut n = m1.len();
            assert_eq!(
                kl_receiver_begin_attempt(p.r, m1.as_mut_ptr(), &mut n),
                KL_OK
            );
            let junk = [0x12u8; 10];
            let mut m3 = [0u8; 128];
            let mut n = m3.len();
            assert_ne!(
                kl_receiver_handle_m2(p.r, junk.as_ptr(), junk.len(), m3.as_mut_ptr(), &mut n),
                KL_OK
            );
        }
        // Three failures: the code is destroyed.
        let mut code = [0u8; 6];
        let mut n = 6usize;
        assert_eq!(
            kl_receiver_code(p.r, code.as_mut_ptr(), &mut n),
            KL_ERR_TOO_MANY_FAILURES
        );
        assert_eq!(code, [0u8; 6]);
        kl_receiver_free(p.r);
    }
}

#[test]
fn null_pointers_return_null_error() {
    unsafe {
        let mut n = 6usize;
        let mut buf = [0u8; 8];
        assert_eq!(
            kl_receiver_new(null(), 0, null(), 0, null_mut()),
            KL_ERR_NULL
        );
        assert_eq!(
            kl_receiver_code(null_mut(), buf.as_mut_ptr(), &mut n),
            KL_ERR_NULL
        );
        assert_eq!(
            kl_receiver_begin_attempt(null_mut(), buf.as_mut_ptr(), &mut n),
            KL_ERR_NULL
        );
        assert_eq!(
            kl_receiver_handle_m2(null_mut(), buf.as_ptr(), 1, buf.as_mut_ptr(), &mut n),
            KL_ERR_NULL
        );
        assert_eq!(
            kl_receiver_finish(null_mut(), buf.as_ptr(), 1, &mut null_mut()),
            KL_ERR_NULL
        );
        assert_eq!(
            kl_sender_finish(
                null_mut(),
                buf.as_ptr(),
                1,
                buf.as_mut_ptr(),
                &mut n,
                &mut null_mut()
            ),
            KL_ERR_NULL
        );
        assert_eq!(kl_channel_approve_one_transfer(null_mut()), KL_ERR_NULL);
        assert_eq!(kl_channel_set_ready_to_receive(null_mut(), 1), KL_ERR_NULL);
        assert_eq!(kl_channel_close(null_mut()), KL_ERR_NULL);
        assert_eq!(kl_channel_is_closed(null_mut()), 1);
        let (mut k, mut d, mut c) = (0u8, 0u8, 0usize);
        assert_eq!(
            kl_channel_seal(
                null_mut(),
                1,
                buf.as_ptr(),
                1,
                8,
                buf.as_mut_ptr(),
                &mut n,
                &mut c
            ),
            KL_ERR_NULL
        );
        assert_eq!(
            kl_channel_open_fragment(
                null_mut(),
                buf.as_ptr(),
                1,
                buf.as_mut_ptr(),
                &mut n,
                &mut k,
                &mut d
            ),
            KL_ERR_NULL
        );
        // Non-empty length with a NULL data pointer.
        let mut r = null_mut();
        assert_eq!(kl_receiver_new(null(), 3, null(), 0, &mut r), KL_ERR_NULL);
        assert!(r.is_null());
        // NULL out_len / out buffer.
        assert_eq!(kl_receiver_new(null(), 0, null(), 0, &mut r), KL_OK);
        assert_eq!(
            kl_receiver_code(r, buf.as_mut_ptr(), null_mut()),
            KL_ERR_NULL
        );
        let mut n = 6usize;
        assert_eq!(kl_receiver_code(r, null_mut(), &mut n), KL_ERR_NULL);
        kl_receiver_free(r);
        // Frees and wipe of NULL are no-ops (double free of NULL is OK).
        kl_receiver_free(null_mut());
        kl_receiver_free(null_mut());
        kl_sender_free(null_mut());
        kl_channel_free(null_mut());
        kl_secure_wipe(null_mut(), 10);
    }
}

#[test]
fn small_buffer_reports_required_size_and_retry_succeeds() {
    unsafe {
        let mut r = null_mut();
        assert_eq!(
            kl_receiver_new(b"rx-dev".as_ptr(), 6, b"claims".as_ptr(), 6, &mut r),
            KL_OK
        );
        let mut tiny = [0u8; 3];
        let mut n = 3usize;
        assert_eq!(
            kl_receiver_code(r, tiny.as_mut_ptr(), &mut n),
            KL_ERR_BUFFER
        );
        assert_eq!((n, tiny), (6, [0u8; 3]));

        // M1: size query, too small, exact. Failed calls must not have started an attempt.
        let mut need = 0usize;
        assert_eq!(
            kl_receiver_begin_attempt(r, null_mut(), &mut need),
            KL_ERR_BUFFER
        );
        assert!(need > 0);
        let mut small = vec![0u8; need - 1];
        let mut n = small.len();
        assert_eq!(
            kl_receiver_begin_attempt(r, small.as_mut_ptr(), &mut n),
            KL_ERR_BUFFER
        );
        assert_eq!(n, need);
        let mut m1 = vec![0u8; need];
        let mut n = need;
        assert_eq!(kl_receiver_begin_attempt(r, m1.as_mut_ptr(), &mut n), KL_OK);
        assert_eq!(n, need);
        let mut code = [0u8; 6];
        let mut cn = 6usize;
        assert_eq!(kl_receiver_code(r, code.as_mut_ptr(), &mut cn), KL_OK);

        // Sender M2 sizing: query yields no handle; real call yields exactly that size.
        let mut need2 = 0usize;
        let mut s0 = null_mut();
        let rc = kl_sender_start(
            code.as_ptr(),
            6,
            m1.as_ptr(),
            m1.len(),
            b"tx-dev".as_ptr(),
            6,
            b"login".as_ptr(),
            5,
            b"sclaims".as_ptr(),
            7,
            null_mut(),
            &mut need2,
            &mut s0,
        );
        assert_eq!(rc, KL_ERR_BUFFER);
        assert!(s0.is_null());
        let (rc, m2, s) = sender_start(&code, &m1);
        assert_eq!(rc, KL_OK);
        assert_eq!(m2.len(), need2);

        // M3: too small first (receiver state untouched), then OK.
        let mut m3 = [0u8; 128];
        let mut n = 10usize;
        assert_eq!(
            kl_receiver_handle_m2(r, m2.as_ptr(), m2.len(), m3.as_mut_ptr(), &mut n),
            KL_ERR_BUFFER
        );
        let need3 = n;
        assert_eq!(
            kl_receiver_handle_m2(r, m2.as_ptr(), m2.len(), m3.as_mut_ptr(), &mut n),
            KL_OK
        );

        // M4 too small: sender not consumed.
        let mut m4 = [0u8; 64];
        let mut tx = null_mut();
        let mut n4 = 5usize;
        assert_eq!(
            kl_sender_finish(s, m3.as_ptr(), need3, m4.as_mut_ptr(), &mut n4, &mut tx),
            KL_ERR_BUFFER
        );
        assert!(tx.is_null());
        assert_eq!(
            kl_sender_finish(s, m3.as_ptr(), need3, m4.as_mut_ptr(), &mut n4, &mut tx),
            KL_OK
        );
        let mut rx = null_mut();
        assert_eq!(kl_receiver_finish(r, m4.as_ptr(), n4, &mut rx), KL_OK);

        // seal: too small leaves counter and approval intact.
        assert_eq!(kl_channel_approve_one_transfer(tx), KL_OK);
        assert_eq!(kl_channel_set_ready_to_receive(rx, 1), KL_OK);
        let pt = b"synthetic secret";
        let mut cnt = 0usize;
        let mut sz = 0usize;
        assert_eq!(
            kl_channel_seal(
                tx,
                SECRET,
                pt.as_ptr(),
                pt.len(),
                256,
                null_mut(),
                &mut sz,
                &mut cnt
            ),
            KL_ERR_BUFFER
        );
        let mut buf = vec![0u8; sz];
        let mut n = sz;
        assert_eq!(
            kl_channel_seal(
                tx,
                SECRET,
                pt.as_ptr(),
                pt.len(),
                256,
                buf.as_mut_ptr(),
                &mut n,
                &mut cnt
            ),
            KL_OK
        );
        assert_eq!((n, cnt), (sz, 1));

        // open: completed message that does not fit is held, then collected.
        let frag = &buf[4..];
        let mut tiny = [0u8; 4];
        let mut tn = 4usize;
        let (mut kind, mut done) = (0u8, 9u8);
        assert_eq!(
            kl_channel_open_fragment(
                rx,
                frag.as_ptr(),
                frag.len(),
                tiny.as_mut_ptr(),
                &mut tn,
                &mut kind,
                &mut done
            ),
            KL_ERR_BUFFER
        );
        assert_eq!((tn, done, tiny), (pt.len(), 0, [0u8; 4]));
        assert_eq!(kl_channel_is_closed(rx), 0);
        let mut plain = vec![0u8; tn];
        let mut pn = tn;
        assert_eq!(
            kl_channel_open_fragment(
                rx,
                null(),
                0,
                plain.as_mut_ptr(),
                &mut pn,
                &mut kind,
                &mut done
            ),
            KL_OK
        );
        assert_eq!((done, kind, plain.as_slice()), (1, SECRET, &pt[..]));
        kl_secure_wipe(plain.as_mut_ptr(), plain.len());

        kl_channel_free(tx);
        kl_channel_free(rx);
        kl_sender_free(s);
        kl_receiver_free(r);
    }
}

#[test]
fn junk_fragment_errors_and_closes_channel() {
    unsafe {
        let (r, s, rx, tx) = handshake();
        let junk = [0xAAu8; 40];
        let mut out = [0u8; 64];
        let mut n = out.len();
        let (mut kind, mut done) = (0u8, 0u8);
        let rc = kl_channel_open_fragment(
            rx,
            junk.as_ptr(),
            junk.len(),
            out.as_mut_ptr(),
            &mut n,
            &mut kind,
            &mut done,
        );
        assert_ne!(rc, KL_OK);
        assert_eq!(done, 0);
        assert_eq!(kl_channel_is_closed(rx), 1);
        let (packed, _) = seal(tx, CONTROL, b"x", 256);
        let f = &packed[4..];
        let mut n = out.len();
        assert_eq!(
            kl_channel_open_fragment(
                rx,
                f.as_ptr(),
                f.len(),
                out.as_mut_ptr(),
                &mut n,
                &mut kind,
                &mut done
            ),
            KL_ERR_CLOSED
        );
        kl_channel_free(tx);
        kl_channel_free(rx);
        kl_sender_free(s);
        kl_receiver_free(r);
    }
}

#[test]
fn length_sanity_and_bad_code() {
    unsafe {
        let mut r = null_mut();
        assert_eq!(kl_receiver_new(null(), 0, null(), 0, &mut r), KL_OK);
        let mut n = 6usize;
        let mut b = [0u8; 6];
        assert_eq!(
            kl_receiver_handle_m2(r, b.as_ptr(), usize::MAX, b.as_mut_ptr(), &mut n),
            KL_ERR_LIMITS
        );
        let p = receiver_with_m1();
        let (rc, _, s) = sender_start(b"12345", &p.m1);
        assert_eq!(rc, KL_ERR_BAD_CODE);
        assert!(s.is_null());
        let (rc, _, s) = sender_start(b"12a456", &p.m1);
        assert_eq!(rc, KL_ERR_BAD_CODE);
        assert!(s.is_null());
        kl_receiver_free(p.r);
        kl_receiver_free(r);
    }
}

#[test]
fn status_values_are_stable() {
    assert_eq!(KL_OK as i32, 0);
    assert_eq!(KL_ERR_EXPIRED as i32, 1);
    assert_eq!(KL_ERR_RANDOMNESS as i32, 17);
    assert_eq!(KL_ERR_NULL as i32, 100);
    assert_eq!(KL_ERR_ABI as i32, 104);
}
