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
fn abi_version_is_three() {
    assert_eq!(kl_abi_version(), 3);
    assert_eq!(KL_ABI_VERSION, 3);
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
        // Failures 2 and 3: fresh attempts that reach M3 and then get a bogus M4 (D026: only an
        // attempt that already sent M3 uses the budget).
        for _ in 0..2 {
            let mut m1 = vec![0u8; 512];
            let mut n = m1.len();
            assert_eq!(
                kl_receiver_begin_attempt(p.r, m1.as_mut_ptr(), &mut n),
                KL_OK
            );
            m1.truncate(n);
            let (rc, m2, s2) = sender_start(&bad, &m1);
            assert_eq!(rc, KL_OK);
            let mut m3 = [0u8; 128];
            let mut n3 = m3.len();
            assert_eq!(
                kl_receiver_handle_m2(p.r, m2.as_ptr(), m2.len(), m3.as_mut_ptr(), &mut n3),
                KL_OK
            );
            let mut rx2 = null_mut();
            assert_ne!(
                kl_receiver_finish(p.r, bogus.as_ptr(), bogus.len(), &mut rx2),
                KL_OK
            );
            assert!(rx2.is_null());
            kl_sender_free(s2);
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

// ---------------------------------------------------------------------------------------
// BLE chunk codec through the C ABI (ABI v2).
// ---------------------------------------------------------------------------------------

const MT_HS: u8 = 1;
const MT_CH: u8 = 2;

fn frame_msg(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 31 + 7) as u8).collect()
}

/// Chunk through the ABI and split the packed output into chunks.
unsafe fn frame(mtype: u8, msg: &[u8], max_chunk: usize) -> Result<Vec<Vec<u8>>, kl_status_t> {
    let mut out = vec![0u8; 4096];
    let mut n = out.len();
    let mut count = 0usize;
    let rc = unsafe {
        kl_frame_chunk(
            mtype,
            msg.as_ptr(),
            msg.len(),
            max_chunk,
            out.as_mut_ptr(),
            &mut n,
            &mut count,
        )
    };
    if rc != KL_OK {
        return Err(rc);
    }
    let mut chunks = Vec::new();
    let mut off = 0;
    for _ in 0..count {
        let l = u32::from_be_bytes(out[off..off + 4].try_into().unwrap()) as usize;
        chunks.push(out[off + 4..off + 4 + l].to_vec());
        off += 4 + l;
    }
    assert_eq!(off, n);
    Ok(chunks)
}

unsafe fn push(h: *mut kl_reassembler_t, chunk: &[u8]) -> (kl_status_t, u8) {
    let mut complete = 7u8;
    let rc = unsafe { kl_reassembler_push(h, chunk.as_ptr(), chunk.len(), &mut complete) };
    (rc, complete)
}

unsafe fn take(h: *mut kl_reassembler_t) -> (kl_status_t, u8, Vec<u8>) {
    let mut out = vec![0u8; 600];
    let mut n = out.len();
    let mut t = 0u8;
    let rc = unsafe { kl_reassembler_take(h, &mut t, out.as_mut_ptr(), &mut n) };
    out.truncate(if rc == KL_OK { n } else { 0 });
    (rc, t, out)
}

unsafe fn new_reasm() -> *mut kl_reassembler_t {
    let mut h = null_mut();
    assert_eq!(unsafe { kl_reassembler_new(&mut h) }, KL_OK);
    assert!(!h.is_null());
    h
}

#[test]
fn frame_round_trip_across_chunk_sizes_and_lengths() {
    unsafe {
        let h = new_reasm();
        for max_chunk in (20usize..=244).chain([6, 512]) {
            for len in [1usize, 2, 5, 19, 20, 64, 65, 255, 269, 274, 300, 511, 512] {
                let m = frame_msg(len);
                let mtype = if len % 2 == 0 { MT_HS } else { MT_CH };
                let chunks = frame(mtype, &m, max_chunk).unwrap();
                assert!(chunks.iter().all(|c| c.len() <= max_chunk));
                for (i, c) in chunks.iter().enumerate() {
                    let (rc, done) = push(h, c);
                    assert_eq!(rc, KL_OK);
                    assert_eq!(done == 1, i + 1 == chunks.len());
                }
                let (rc, t, data) = take(h);
                assert_eq!((rc, t, data), (KL_OK, mtype, m));
                // Nothing left to take.
                assert_eq!(take(h).0, KL_ERR_WRONG_STATE);
            }
        }
        assert_eq!(kl_reassembler_is_dead(h), 0);
        kl_reassembler_free(h);
    }
}

#[test]
fn frame_errors_are_fatal_and_distinct_cases_are_rejected() {
    unsafe {
        let m = frame_msg(200);
        let chunks = frame(MT_CH, &m, 20).unwrap();
        assert!(chunks.len() > 4);

        // duplicate
        let h = new_reasm();
        assert_eq!(push(h, &chunks[0]).0, KL_OK);
        assert_eq!(push(h, &chunks[0]).0, KL_ERR_FRAMING);
        assert_eq!(kl_reassembler_is_dead(h), 1);
        assert_eq!(push(h, &chunks[1]).0, KL_ERR_CLOSED);
        kl_reassembler_free(h);

        // gap
        let h = new_reasm();
        assert_eq!(push(h, &chunks[0]).0, KL_OK);
        assert_eq!(push(h, &chunks[2]).0, KL_ERR_FRAMING);
        assert_eq!(kl_reassembler_is_dead(h), 1);
        kl_reassembler_free(h);

        // reorder
        let h = new_reasm();
        assert_eq!(push(h, &chunks[0]).0, KL_OK);
        assert_eq!(push(h, &chunks[1]).0, KL_OK);
        assert_eq!(push(h, &chunks[3]).0, KL_ERR_FRAMING);
        kl_reassembler_free(h);

        // reserved flag bits (bit 3)
        let h = new_reasm();
        let mut bad = chunks[0].clone();
        bad[0] |= 0b1000;
        assert_eq!(push(h, &bad).0, KL_ERR_MALFORMED);
        assert_eq!(kl_reassembler_is_dead(h), 1);
        kl_reassembler_free(h);

        // ACK flag in a data chunk and a bare ACK chunk are not data
        let h = new_reasm();
        let mut ackflag = chunks[0].clone();
        ackflag[0] |= 0b100;
        assert_eq!(push(h, &ackflag).0, KL_ERR_MALFORMED);
        kl_reassembler_free(h);
        let h = new_reasm();
        assert_eq!(push(h, &[0b100, 0]).0, KL_ERR_MALFORMED);
        kl_reassembler_free(h);

        // oversize declared length (513) and zero length
        for total in [513u16, 0] {
            let h = new_reasm();
            let mut c = chunks[0].clone();
            c[3..5].copy_from_slice(&total.to_be_bytes());
            assert_eq!(push(h, &c).0, KL_ERR_LIMITS);
            kl_reassembler_free(h);
        }

        // unknown mtype
        let h = new_reasm();
        let mut c = chunks[0].clone();
        c[2] = 9;
        assert_eq!(push(h, &c).0, KL_ERR_UNSUPPORTED);
        kl_reassembler_free(h);

        // not starting at FIRST
        let h = new_reasm();
        assert_eq!(push(h, &chunks[1]).0, KL_ERR_FRAMING);
        kl_reassembler_free(h);

        // oversize chunk (513 bytes) and empty / header-only chunks
        let h = new_reasm();
        assert_eq!(push(h, &[1u8; 513]).0, KL_ERR_MALFORMED);
        kl_reassembler_free(h);
        let h = new_reasm();
        let mut complete = 0u8;
        assert_eq!(
            kl_reassembler_push(h, null(), 0, &mut complete),
            KL_ERR_MALFORMED
        );
        kl_reassembler_free(h);
        let h = new_reasm();
        assert_eq!(push(h, &[1, 0]).0, KL_ERR_MALFORMED);
        kl_reassembler_free(h);

        // LAST flag missing on the final chunk
        let h = new_reasm();
        let mut tail = chunks.clone();
        let n = tail.len() - 1;
        tail[n][0] &= !0b10;
        for c in &tail[..n] {
            assert_eq!(push(h, c).0, KL_OK);
        }
        assert_eq!(push(h, &tail[n]).0, KL_ERR_FRAMING);
        kl_reassembler_free(h);
    }
}

#[test]
fn frame_chunking_rejects_bad_inputs_without_side_effects() {
    unsafe {
        let m = frame_msg(10);
        assert_eq!(frame(0, &m, 20).unwrap_err(), KL_ERR_LIMITS);
        assert_eq!(frame(3, &m, 20).unwrap_err(), KL_ERR_LIMITS);
        assert_eq!(frame(MT_CH, &[], 20).unwrap_err(), KL_ERR_LIMITS);
        assert_eq!(
            frame(MT_CH, &frame_msg(513), 20).unwrap_err(),
            KL_ERR_LIMITS
        );
        assert_eq!(frame(MT_CH, &m, 5).unwrap_err(), KL_ERR_LIMITS);
        assert_eq!(frame(MT_CH, &m, 513).unwrap_err(), KL_ERR_LIMITS);
        // Lengths over the FFI sanity bound are rejected before any read.
        let mut n = 16usize;
        let mut count = 0usize;
        let mut out = [0u8; 16];
        assert_eq!(
            kl_frame_chunk(
                MT_CH,
                [0u8].as_ptr(),
                1 << 21,
                20,
                out.as_mut_ptr(),
                &mut n,
                &mut count
            ),
            KL_ERR_LIMITS
        );
    }
}

#[test]
fn frame_null_and_tiny_buffer_cases() {
    unsafe {
        let m = frame_msg(100);
        let mp = m.as_ptr();
        let ml = m.len();
        let mut count = 0usize;
        let mut out = [0u8; 8];
        let mut n = out.len();
        // NULL pointers
        assert_eq!(
            kl_frame_chunk(MT_CH, null(), 5, 20, out.as_mut_ptr(), &mut n, &mut count),
            KL_ERR_NULL
        );
        assert_eq!(
            kl_frame_chunk(MT_CH, mp, ml, 20, out.as_mut_ptr(), null_mut(), &mut count),
            KL_ERR_NULL
        );
        assert_eq!(
            kl_frame_chunk(MT_CH, mp, ml, 20, out.as_mut_ptr(), &mut n, null_mut()),
            KL_ERR_NULL
        );
        // size query: capacity 0 and NULL out reports the required size, writes nothing
        let mut n0 = 0usize;
        assert_eq!(
            kl_frame_chunk(MT_CH, mp, ml, 20, null_mut(), &mut n0, &mut count),
            KL_ERR_BUFFER
        );
        let required = n0;
        assert_eq!(count, 0);
        let expected = frame(MT_CH, &m, 20).unwrap();
        let want: usize = expected.iter().map(|c| 4 + c.len()).sum();
        assert_eq!(required, want);
        // tiny buffer
        let mut n1 = 8usize;
        assert_eq!(
            kl_frame_chunk(MT_CH, mp, ml, 20, out.as_mut_ptr(), &mut n1, &mut count),
            KL_ERR_BUFFER
        );
        assert_eq!(n1, required);
        assert_eq!(out, [0u8; 8]);
        // exactly sized buffer works
        let mut exact = vec![0u8; required];
        let mut n2 = required;
        assert_eq!(
            kl_frame_chunk(MT_CH, mp, ml, 20, exact.as_mut_ptr(), &mut n2, &mut count),
            KL_OK
        );
        assert_eq!((n2, count), (required, expected.len()));

        // reassembler: NULL handles and out pointers
        let mut complete = 0u8;
        assert_eq!(kl_reassembler_new(null_mut()), KL_ERR_NULL);
        assert_eq!(
            kl_reassembler_push(null_mut(), exact.as_ptr(), 3, &mut complete),
            KL_ERR_NULL
        );
        let h = new_reasm();
        assert_eq!(
            kl_reassembler_push(h, expected[0].as_ptr(), expected[0].len(), null_mut()),
            KL_ERR_NULL
        );
        assert_eq!(
            kl_reassembler_push(h, null(), 4, &mut complete),
            KL_ERR_NULL
        );
        // NULL errors did not kill it
        assert_eq!(kl_reassembler_is_dead(h), 0);
        let mut t = 0u8;
        let mut n3 = 0usize;
        assert_eq!(
            kl_reassembler_take(null_mut(), &mut t, null_mut(), &mut n3),
            KL_ERR_NULL
        );
        assert_eq!(
            kl_reassembler_take(h, null_mut(), null_mut(), &mut n3),
            KL_ERR_NULL
        );
        // nothing held yet
        assert_eq!(
            kl_reassembler_take(h, &mut t, null_mut(), null_mut()),
            KL_ERR_WRONG_STATE
        );
        assert_eq!(kl_reassembler_is_dead(null_mut()), 1);
        kl_reassembler_free(null_mut());

        // tiny take buffer keeps the message held; push is refused meanwhile
        for c in &expected {
            assert_eq!(push(h, c).0, KL_OK);
        }
        assert_eq!(push(h, &expected[0]).0, KL_ERR_WRONG_STATE);
        assert_eq!(kl_reassembler_is_dead(h), 0);
        let mut tiny = [0u8; 4];
        let mut n4 = tiny.len();
        assert_eq!(
            kl_reassembler_take(h, &mut t, tiny.as_mut_ptr(), &mut n4),
            KL_ERR_BUFFER
        );
        assert_eq!(n4, m.len());
        assert_eq!(tiny, [0u8; 4]);
        let mut n5 = 0usize;
        assert_eq!(
            kl_reassembler_take(h, &mut t, null_mut(), &mut n5),
            KL_ERR_BUFFER
        );
        assert_eq!(n5, m.len());
        let (rc, mt, data) = take(h);
        assert_eq!((rc, mt, data), (KL_OK, MT_CH, m));
        // reusable for the next message after take
        assert_eq!(push(h, &expected[0]).0, KL_OK);
        kl_reassembler_free(h);
    }
}

#[test]
fn ack_round_trip_and_rejections() {
    unsafe {
        for seq in [0u8, 1, 127, 128, 255] {
            let mut out = [0u8; 2];
            let mut n = out.len();
            assert_eq!(kl_ack_chunk(seq, out.as_mut_ptr(), &mut n), KL_OK);
            assert_eq!(n, 2);
            let (mut is_ack, mut got) = (9u8, 9u8);
            assert_eq!(kl_parse_ack(out.as_ptr(), 2, &mut is_ack, &mut got), KL_OK);
            assert_eq!((is_ack, got), (1, seq));
        }
        // buffer / null handling for kl_ack_chunk
        let mut out = [0u8; 1];
        let mut n = 1usize;
        assert_eq!(kl_ack_chunk(3, out.as_mut_ptr(), &mut n), KL_ERR_BUFFER);
        assert_eq!((n, out), (2, [0u8]));
        assert_eq!(kl_ack_chunk(3, out.as_mut_ptr(), null_mut()), KL_ERR_NULL);
        let mut n = 2usize;
        assert_eq!(kl_ack_chunk(3, null_mut(), &mut n), KL_ERR_NULL);
        // not an ACK: still OK, outputs cleared
        let (mut is_ack, mut got) = (9u8, 9u8);
        let not_acks: [&[u8]; 4] = [&[0b100, 1, 0], &[1, 1], &[], &[0b100]];
        for c in not_acks {
            let p = if c.is_empty() { null() } else { c.as_ptr() };
            assert_eq!(kl_parse_ack(p, c.len(), &mut is_ack, &mut got), KL_OK);
            assert_eq!((is_ack, got), (0, 0));
            is_ack = 9;
            got = 9;
        }
        assert_eq!(kl_parse_ack(null(), 2, &mut is_ack, &mut got), KL_ERR_NULL);
        let a = [4u8, 0];
        assert_eq!(
            kl_parse_ack(a.as_ptr(), 2, null_mut(), &mut got),
            KL_ERR_NULL
        );
        assert_eq!(
            kl_parse_ack(a.as_ptr(), 2, &mut is_ack, null_mut()),
            KL_ERR_NULL
        );
        // an ACK chunk fed to the reassembler is an error, which is why callers filter first
        let h = new_reasm();
        assert_eq!(push(h, &[0b100, 0]).0, KL_ERR_MALFORMED);
        kl_reassembler_free(h);
    }
}

#[test]
fn frame_constants_match_core() {
    use areatm_kagilink_core::ble_frame as bf;
    assert_eq!(bf::MTYPE_HANDSHAKE, MT_HS);
    assert_eq!(bf::MTYPE_CHANNEL, MT_CH);
    assert_eq!(bf::MAX_BLE_MESSAGE, 512);
    assert_eq!(bf::MIN_CHUNK, 6);
    assert_eq!(bf::MAX_CHUNK, 512);
}

#[test]
fn closed_channel_refuses_approval_readiness_and_held_plaintext() {
    unsafe {
        let (r, s, rx, tx) = handshake();
        assert_eq!(kl_channel_set_ready_to_receive(rx, 1), KL_OK);

        // Complete a message into the held state (buffer too small).
        let pt = b"synthetic-held-plaintext";
        assert_eq!(kl_channel_approve_one_transfer(tx), KL_OK);
        let (packed, count) = seal(tx, SECRET, pt, 256);
        assert_eq!(count, 1);
        let frag = &packed[4..];
        let mut tiny = [0u8; 2];
        let mut tn = tiny.len();
        let (mut kind, mut done) = (0u8, 0u8);
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
        assert_eq!(tn, pt.len());

        // Close, then try to collect the held message: must be refused and nothing written.
        assert_eq!(kl_channel_close(rx), KL_OK);
        let mut plain = vec![0u8; 64];
        let mut pn = plain.len();
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
            KL_ERR_CLOSED
        );
        assert_eq!((done, plain.iter().all(|&b| b == 0)), (0, true));
        pn = plain.len();
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
            KL_ERR_CLOSED
        );
        assert!(plain.iter().all(|&b| b == 0));

        assert_eq!(kl_channel_is_closed(rx), 1);
        assert_eq!(kl_channel_approve_one_transfer(rx), KL_ERR_CLOSED);
        assert_eq!(kl_channel_set_ready_to_receive(rx, 1), KL_ERR_CLOSED);
        // Closing is idempotent and NULL is still an error.
        assert_eq!(kl_channel_close(rx), KL_OK);
        assert_eq!(kl_channel_approve_one_transfer(null_mut()), KL_ERR_NULL);
        assert_eq!(kl_channel_set_ready_to_receive(null_mut(), 1), KL_ERR_NULL);

        kl_channel_free(tx);
        kl_channel_free(rx);
        kl_sender_free(s);
        kl_receiver_free(r);
    }
}

#[test]
fn freeing_with_held_data_and_null_is_safe() {
    unsafe {
        kl_channel_free(null_mut());
        kl_reassembler_free(null_mut());
        // Reassembler freed while a completed message is still held.
        let h = new_reasm();
        let chunks = frame(1, &frame_msg(10), 244).unwrap();
        for c in &chunks {
            assert_eq!(push(h, c).0, KL_OK);
        }
        kl_reassembler_free(h);
        // Channel freed while a completed message is still held.
        let (r, s, rx, tx) = handshake();
        assert_eq!(kl_channel_set_ready_to_receive(rx, 1), KL_OK);
        assert_eq!(kl_channel_approve_one_transfer(tx), KL_OK);
        let (packed, _) = seal(tx, SECRET, b"synthetic", 256);
        let frag = &packed[4..];
        let mut tiny = [0u8; 1];
        let mut tn = 1usize;
        let (mut kind, mut done) = (0u8, 0u8);
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
        kl_channel_free(rx);
        kl_channel_free(tx);
        kl_sender_free(s);
        kl_receiver_free(r);
    }
}

// ---------------------------------------------------------------------------------------
// Handle registry: stale / doubled / racing / forged handles (docs/FFI_DESIGN.md).
// Handles are tokens, never dereferenced, so none of these may crash.
// ---------------------------------------------------------------------------------------

/// Calls every handle-taking export with `bad` as each handle type; none may succeed.
unsafe fn assert_all_handles_rejected(bad: usize) {
    unsafe {
        let r = bad as *mut kl_receiver_t;
        let s = bad as *mut kl_sender_t;
        let c = bad as *mut kl_channel_t;
        let h = bad as *mut kl_reassembler_t;
        let mut buf = [0u8; 600];
        let mut n = buf.len();
        assert_eq!(kl_receiver_code(r, buf.as_mut_ptr(), &mut n), KL_ERR_NULL);
        n = buf.len();
        assert_eq!(
            kl_receiver_begin_attempt(r, buf.as_mut_ptr(), &mut n),
            KL_ERR_NULL
        );
        n = buf.len();
        assert_eq!(
            kl_receiver_handle_m2(r, buf.as_ptr(), 4, buf.as_mut_ptr(), &mut n),
            KL_ERR_NULL
        );
        let mut ch = null_mut();
        assert_eq!(kl_receiver_finish(r, buf.as_ptr(), 4, &mut ch), KL_ERR_NULL);
        assert!(ch.is_null());
        n = buf.len();
        let mut tx = null_mut();
        let m3 = [0u8; 4];
        assert_eq!(
            kl_sender_finish(s, m3.as_ptr(), 4, buf.as_mut_ptr(), &mut n, &mut tx),
            KL_ERR_NULL
        );
        assert_eq!(kl_channel_approve_one_transfer(c), KL_ERR_NULL);
        assert_eq!(kl_channel_set_ready_to_receive(c, 1), KL_ERR_NULL);
        let (mut out, mut on, mut cnt) = ([0u8; 256], 256usize, 0usize);
        assert_eq!(
            kl_channel_seal(
                c,
                SECRET,
                b"x".as_ptr(),
                1,
                64,
                out.as_mut_ptr(),
                &mut on,
                &mut cnt
            ),
            KL_ERR_NULL
        );
        on = 256;
        let (mut k, mut d) = (0u8, 0u8);
        assert_eq!(
            kl_channel_open_fragment(
                c,
                b"x".as_ptr(),
                1,
                out.as_mut_ptr(),
                &mut on,
                &mut k,
                &mut d
            ),
            KL_ERR_NULL
        );
        assert_eq!(kl_channel_close(c), KL_ERR_NULL);
        assert_eq!(kl_channel_is_closed(c), 1);
        let mut done = 0u8;
        assert_eq!(
            kl_reassembler_push(h, b"x".as_ptr(), 1, &mut done),
            KL_ERR_NULL
        );
        let mut mt = 0u8;
        on = 256;
        assert_eq!(
            kl_reassembler_take(h, &mut mt, out.as_mut_ptr(), &mut on),
            KL_ERR_NULL
        );
        assert_eq!(kl_reassembler_is_dead(h), 1);
        // Frees of garbage are harmless no-ops.
        kl_receiver_free(r);
        kl_sender_free(s);
        kl_channel_free(c);
        kl_reassembler_free(h);
    }
}

#[test]
fn double_free_and_use_after_free_are_errors_not_ub() {
    unsafe {
        let (r, s, rx, tx) = handshake();
        let h = new_reasm();
        kl_receiver_free(r);
        kl_sender_free(s);
        kl_channel_free(rx);
        kl_channel_free(tx);
        kl_reassembler_free(h);
        // Second and third frees: harmless.
        for _ in 0..2 {
            kl_receiver_free(r);
            kl_sender_free(s);
            kl_channel_free(rx);
            kl_channel_free(tx);
            kl_reassembler_free(h);
        }
        // Every use of a dead token is an error, per handle kind.
        let mut buf = [0u8; 16];
        let mut n = buf.len();
        assert_eq!(kl_receiver_code(r, buf.as_mut_ptr(), &mut n), KL_ERR_NULL);
        assert_eq!(kl_channel_approve_one_transfer(rx), KL_ERR_NULL);
        assert_eq!(kl_channel_close(tx), KL_ERR_NULL);
        assert_eq!(kl_channel_is_closed(tx), 1);
        let mut done = 0u8;
        assert_eq!(
            kl_reassembler_push(h, b"x".as_ptr(), 1, &mut done),
            KL_ERR_NULL
        );
        assert_eq!(kl_reassembler_is_dead(h), 1);
        let mut ch = null_mut();
        let mut m4 = [0u8; 64];
        let mut n4 = m4.len();
        assert_eq!(
            kl_sender_finish(s, b"abcd".as_ptr(), 4, m4.as_mut_ptr(), &mut n4, &mut ch),
            KL_ERR_NULL
        );
        assert!(ch.is_null());
    }
}

#[test]
fn wrong_type_token_is_rejected_and_not_freed() {
    unsafe {
        let (r, s, rx, tx) = handshake();
        // Use each handle as another type: error, and the free is a no-op.
        let as_chan = r as usize as *mut kl_channel_t;
        assert_eq!(kl_channel_close(as_chan), KL_ERR_NULL);
        kl_channel_free(as_chan);
        let as_recv = rx as usize as *mut kl_receiver_t;
        let mut buf = [0u8; 16];
        let mut n = buf.len();
        assert_eq!(
            kl_receiver_code(as_recv, buf.as_mut_ptr(), &mut n),
            KL_ERR_NULL
        );
        kl_receiver_free(as_recv);
        kl_sender_free(rx as usize as *mut kl_sender_t);
        kl_reassembler_free(tx as usize as *mut kl_reassembler_t);
        // The real objects are untouched and still usable.
        assert_eq!(kl_channel_is_closed(rx), 0);
        assert_eq!(kl_channel_is_closed(tx), 0);
        n = buf.len();
        // The receiver finished its handshake, so its code is consumed (not a NULL error).
        assert_eq!(
            kl_receiver_code(r, buf.as_mut_ptr(), &mut n),
            KL_ERR_NOT_ACTIVE
        );
        kl_channel_free(rx);
        kl_channel_free(tx);
        kl_sender_free(s);
        kl_receiver_free(r);
    }
}

#[test]
fn forged_and_garbage_tokens_never_dereference() {
    unsafe {
        // Keep live objects so the registry has real slots.
        let (r, s, rx, tx) = handshake();
        let local = 0u64;
        let stack_addr = &local as *const u64 as usize;
        let boxed = Box::new([0u8; 64]);
        let heap_addr = boxed.as_ptr() as usize;
        let live = [r as usize, s as usize, rx as usize, tx as usize];
        for bad in [
            1usize,
            2,
            0x10,
            0x1000,
            0xdead_beef,
            usize::MAX,
            usize::MAX - 1,
            usize::MAX / 2,
            1usize << (usize::BITS - 1),
            stack_addr,
            heap_addr,
            // live tokens with a high (generation) bit flipped. Index-bit flips are NOT used:
            // they can legitimately alias another live object (tests share the registry).
            (rx as usize) ^ (1usize << (usize::BITS - 5)),
            (tx as usize) ^ (1usize << (usize::BITS - 3)),
        ] {
            if live.contains(&bad) {
                continue;
            }
            assert_all_handles_rejected(bad);
        }
        assert_all_handles_rejected(0);
        // None of that disturbed the live objects.
        assert_eq!(kl_channel_is_closed(rx), 0);
        assert_eq!(kl_channel_is_closed(tx), 0);
        drop(boxed);
        kl_channel_free(rx);
        kl_channel_free(tx);
        kl_sender_free(s);
        kl_receiver_free(r);
    }
}

#[test]
fn token_reuse_after_free_does_not_resolve_to_new_object() {
    unsafe {
        let mut old: Vec<usize> = Vec::new();
        for _ in 0..64 {
            let h = new_reasm();
            assert!(!old.contains(&(h as usize)), "token repeated while stale");
            // Every earlier (freed) token must stay dead while the slot is reused.
            for &t in &old {
                let mut done = 0u8;
                assert_eq!(
                    kl_reassembler_push(t as *mut kl_reassembler_t, b"x".as_ptr(), 1, &mut done),
                    KL_ERR_NULL
                );
            }
            // The new object is alive.
            assert_eq!(kl_reassembler_is_dead(h), 0);
            kl_reassembler_free(h);
            // A stale free of an old token must not free a newer object in the reused slot.
            let h2 = new_reasm();
            for &t in &old {
                kl_reassembler_free(t as *mut kl_reassembler_t);
            }
            assert_eq!(kl_reassembler_is_dead(h2), 0);
            kl_reassembler_free(h2);
            old.push(h as usize);
            old.push(h2 as usize);
        }
    }
}

#[derive(Clone, Copy)]
struct Tok(usize);

#[test]
fn concurrent_calls_and_free_on_one_token_do_not_crash_or_deadlock() {
    for _ in 0..200 {
        unsafe {
            let (r, s, rx, tx) = handshake();
            assert_eq!(kl_channel_set_ready_to_receive(rx, 1), KL_OK);
            let t = Tok(tx as usize);
            let rxt = Tok(rx as usize);
            let mut workers = Vec::new();
            for i in 0..3u8 {
                workers.push(std::thread::spawn(move || {
                    for _ in 0..20 {
                        let c = t.0 as *mut kl_channel_t;
                        let mut out = vec![0u8; 4096];
                        let mut n = out.len();
                        let mut cnt = 0usize;
                        let rc = kl_channel_approve_one_transfer(c);
                        assert!(
                            matches!(rc, KL_OK | KL_ERR_NULL | KL_ERR_BUSY | KL_ERR_CLOSED),
                            "approve {rc:?}"
                        );
                        let rc = kl_channel_seal(
                            c,
                            SECRET,
                            b"synthetic".as_ptr(),
                            9,
                            64,
                            out.as_mut_ptr(),
                            &mut n,
                            &mut cnt,
                        );
                        assert!(
                            matches!(
                                rc,
                                KL_OK
                                    | KL_ERR_NULL
                                    | KL_ERR_BUSY
                                    | KL_ERR_CLOSED
                                    | KL_ERR_NOT_APPROVED
                            ),
                            "seal {rc:?}"
                        );
                        let _ = kl_channel_is_closed(c);
                        if i == 2 {
                            let _ = kl_channel_close(c);
                        }
                    }
                }));
            }
            // Two racing frees of the same token plus a racing use/free of the other channel.
            for _ in 0..2 {
                workers.push(std::thread::spawn(move || {
                    kl_channel_free(t.0 as *mut kl_channel_t);
                }));
            }
            workers.push(std::thread::spawn(move || {
                let c = rxt.0 as *mut kl_channel_t;
                let _ = kl_channel_is_closed(c);
                kl_channel_free(c);
            }));
            for w in workers {
                w.join().expect("worker panicked");
            }
            // After the dust settles both tokens are dead.
            assert_eq!(kl_channel_is_closed(tx), 1);
            assert_eq!(kl_channel_close(tx), KL_ERR_NULL);
            assert_eq!(kl_channel_close(rx), KL_ERR_NULL);
            kl_sender_free(s);
            kl_receiver_free(r);
        }
    }
}

#[test]
fn concurrent_reassembler_hammer_with_free() {
    for _ in 0..200 {
        unsafe {
            let h = new_reasm();
            let t = Tok(h as usize);
            let mut ws = Vec::new();
            for _ in 0..3 {
                ws.push(std::thread::spawn(move || {
                    for _ in 0..50 {
                        let h = t.0 as *mut kl_reassembler_t;
                        let mut done = 0u8;
                        let rc = kl_reassembler_push(h, [0u8].as_ptr(), 1, &mut done);
                        assert_ne!(rc, KL_OK);
                        let _ = kl_reassembler_is_dead(h);
                        let mut mt = 0u8;
                        let mut buf = [0u8; 8];
                        let mut n = buf.len();
                        let _ = kl_reassembler_take(h, &mut mt, buf.as_mut_ptr(), &mut n);
                    }
                }));
            }
            ws.push(std::thread::spawn(move || {
                kl_reassembler_free(t.0 as *mut kl_reassembler_t);
            }));
            for w in ws {
                w.join().expect("worker panicked");
            }
            assert_eq!(kl_reassembler_is_dead(h), 1);
            kl_reassembler_free(h);
        }
    }
}

// ---------------------------------------------------------------------------------------
// kl_receiver_failures (D030): read-only failure budget getter.
// ---------------------------------------------------------------------------------------

unsafe fn failures(r: *mut kl_receiver_t) -> (kl_status_t, u8, u8) {
    let (mut used, mut max) = (0xAAu8, 0xAAu8);
    let rc = unsafe { kl_receiver_failures(r, &mut used, &mut max) };
    (rc, used, max)
}

/// Starts an attempt with a wrong code and drives it to M3 sent (M4 not yet delivered).
unsafe fn attempt_to_m3_with_wrong_code(r: *mut kl_receiver_t, code: &[u8; 6]) {
    let mut bad = *code;
    bad[0] = if bad[0] == b'0' { b'1' } else { b'0' };
    let mut m1 = vec![0u8; 512];
    let mut n = m1.len();
    assert_eq!(
        unsafe { kl_receiver_begin_attempt(r, m1.as_mut_ptr(), &mut n) },
        KL_OK
    );
    m1.truncate(n);
    let (rc, m2, s) = unsafe { sender_start(&bad, &m1) };
    assert_eq!(rc, KL_OK);
    let mut m3 = [0u8; 128];
    let mut n3 = m3.len();
    assert_eq!(
        unsafe { kl_receiver_handle_m2(r, m2.as_ptr(), m2.len(), m3.as_mut_ptr(), &mut n3) },
        KL_OK
    );
    unsafe { kl_sender_free(s) };
}

#[test]
fn receiver_failures_starts_at_zero_of_three_and_is_read_only() {
    unsafe {
        let p = receiver_with_m1();
        for _ in 0..3 {
            assert_eq!(failures(p.r), (KL_OK, 0, 3));
        }
        // Reading did not disturb the code or the open attempt.
        let mut code = [0u8; 6];
        let mut n = 6usize;
        assert_eq!(kl_receiver_code(p.r, code.as_mut_ptr(), &mut n), KL_OK);
        assert_eq!(code, p.code);
        kl_receiver_free(p.r);
    }
}

#[test]
fn receiver_failures_counts_open_attempt_only_after_m3() {
    unsafe {
        let p = receiver_with_m1();
        // M1 sent only: an attempt that never sent M3 costs nothing.
        assert_eq!(failures(p.r), (KL_OK, 0, 3));
        // M3 sent, M4 outstanding: counted as open.
        attempt_to_m3_with_wrong_code(p.r, &p.code);
        assert_eq!(failures(p.r), (KL_OK, 1, 3));
        assert_eq!(failures(p.r), (KL_OK, 1, 3));
        // Next attempt begins: the open one is booked, the new one (no M3 yet) adds nothing.
        let mut m1 = vec![0u8; 512];
        let mut n = m1.len();
        assert_eq!(
            kl_receiver_begin_attempt(p.r, m1.as_mut_ptr(), &mut n),
            KL_OK
        );
        assert_eq!(failures(p.r), (KL_OK, 1, 3));
        // A bad M4 before M3 is refused and costs nothing.
        let bogus = [0x14u8; 33];
        let mut rx = null_mut();
        assert_ne!(
            kl_receiver_finish(p.r, bogus.as_ptr(), bogus.len(), &mut rx),
            KL_OK
        );
        assert_eq!(failures(p.r), (KL_OK, 1, 3));
        kl_receiver_free(p.r);
    }
}

#[test]
fn receiver_failures_is_three_of_three_after_budget_spent() {
    unsafe {
        let p = receiver_with_m1();
        let bogus = [0x14u8; 33];
        // Three attempts that each reach M3 and then get a bad M4.
        for i in 0..3u8 {
            attempt_to_m3_with_wrong_code(p.r, &p.code);
            let mut rx = null_mut();
            assert_ne!(
                kl_receiver_finish(p.r, bogus.as_ptr(), bogus.len(), &mut rx),
                KL_OK
            );
            assert!(rx.is_null());
            assert_eq!(failures(p.r), (KL_OK, i + 1, 3));
        }
        // Receiver inactive: code destroyed, the count stays final and is not an error.
        let mut code = [0u8; 6];
        let mut n = 6usize;
        assert_eq!(
            kl_receiver_code(p.r, code.as_mut_ptr(), &mut n),
            KL_ERR_TOO_MANY_FAILURES
        );
        assert_eq!(failures(p.r), (KL_OK, 3, 3));
        assert_eq!(failures(p.r), (KL_OK, 3, 3));
        kl_receiver_free(p.r);
    }
}

#[test]
fn receiver_failures_keeps_working_after_success() {
    unsafe {
        let (r, s, rx, tx) = handshake();
        assert_eq!(failures(r), (KL_OK, 0, 3));
        kl_channel_free(rx);
        kl_channel_free(tx);
        kl_sender_free(s);
        kl_receiver_free(r);
    }
}

#[test]
fn receiver_failures_rejects_null_stale_and_wrong_type_handles() {
    unsafe {
        let (mut used, mut max) = (0xAAu8, 0xAAu8);
        assert_eq!(
            kl_receiver_failures(null_mut(), &mut used, &mut max),
            KL_ERR_NULL
        );
        let mut r = null_mut();
        assert_eq!(kl_receiver_new(null(), 0, null(), 0, &mut r), KL_OK);
        // NULL out pointers: rejected, nothing written.
        assert_eq!(kl_receiver_failures(r, null_mut(), &mut max), KL_ERR_NULL);
        assert_eq!(kl_receiver_failures(r, &mut used, null_mut()), KL_ERR_NULL);
        assert_eq!(kl_receiver_failures(r, null_mut(), null_mut()), KL_ERR_NULL);
        assert_eq!((used, max), (0xAA, 0xAA));
        assert_eq!(failures(r), (KL_OK, 0, 3));
        // Wrong object type with a live token.
        let (r2, s, rx, tx) = handshake();
        assert_eq!(
            kl_receiver_failures(rx as *mut kl_receiver_t, &mut used, &mut max),
            KL_ERR_NULL
        );
        // Stale handle after free, also after the slot is reused by a new receiver.
        kl_receiver_free(r);
        assert_eq!(kl_receiver_failures(r, &mut used, &mut max), KL_ERR_NULL);
        let mut r3 = null_mut();
        assert_eq!(kl_receiver_new(null(), 0, null(), 0, &mut r3), KL_OK);
        assert_eq!(kl_receiver_failures(r, &mut used, &mut max), KL_ERR_NULL);
        assert_eq!(failures(r3), (KL_OK, 0, 3));
        // Forged garbage token.
        used = 0xAA;
        max = 0xAA;
        assert_eq!(
            kl_receiver_failures(0x7fff_1234usize as *mut kl_receiver_t, &mut used, &mut max),
            KL_ERR_NULL
        );
        assert_eq!((used, max), (0xAA, 0xAA));
        kl_receiver_free(r3);
        kl_channel_free(rx);
        kl_channel_free(tx);
        kl_sender_free(s);
        kl_receiver_free(r2);
    }
}
