//! Plain C ABI over `areatm-kagilink-core` (decision D016, docs/FFI_DESIGN.md).
//!
//! This is the only crate in the workspace that may contain `unsafe`. Rules:
//! * opaque handles, created and freed only here; no Rust-allocated memory is returned;
//! * caller-allocated output buffers (`out`, `*out_len` = capacity in / length out);
//!   a too-small buffer yields `KL_ERR_BUFFER` with the required size and consumes no
//!   protocol state (sizes are checked before the operation, or the result is held);
//! * output buffers are written only on success;
//! * every export null-checks, bounds-checks input lengths, is wrapped in `catch_unwind`
//!   and uses `try_lock` (`KL_ERR_BUSY`);
//! * no logging, no callbacks, no global state, no secret text in any return value.
//!
//! Synthetic data only until an independent review of this `unsafe` code exists.

#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types)]
#![warn(missing_docs)]
#![allow(clippy::too_many_arguments)]

use areatm_kagilink_core::transport::{
    Message, SecureChannel, MAX_FRAGMENTS, MAX_FRAGMENT_PAYLOAD, MAX_MESSAGE,
};
use areatm_kagilink_core::wire::{NONCE_LEN, SHARE_LEN, SID_LEN, SUITE, TAG_LEN};
use areatm_kagilink_core::{os_rng, Code, Error, ReceiverSession, SenderPending, SystemClock};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use zeroize::Zeroize;

/// ABI version returned by [`kl_abi_version`]. Bumped on any signature change.
pub const KL_ABI_VERSION: u32 = 1;

/// Upper bound accepted for any single input length (sanity check, bytes).
const MAX_IN: usize = 1 << 20;
/// Transport framing constants mirrored for size prechecks (verified by tests).
const FRAGMENT_HEADER: usize = 13;
const AEAD_TAG: usize = 16;
const LEN_PREFIX: usize = 4;

/// Status codes. Numeric values are stable ABI. 1..=17 map one-to-one from the core `Error`.
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum kl_status_t {
    /// Success.
    KL_OK = 0,
    /// Core: the 60 s code/session deadline passed.
    KL_ERR_EXPIRED = 1,
    /// Core: session consumed, destroyed or not active.
    KL_ERR_NOT_ACTIVE = 2,
    /// Core: three failures, code destroyed.
    KL_ERR_TOO_MANY_FAILURES = 3,
    /// Core: an attempt is already in progress.
    KL_ERR_ATTEMPT_IN_PROGRESS = 4,
    /// Core: malformed, oversized or trailing-byte message.
    KL_ERR_MALFORMED = 5,
    /// Core: unsupported version or suite.
    KL_ERR_UNSUPPORTED = 6,
    /// Core: message in the wrong protocol state.
    KL_ERR_WRONG_STATE = 7,
    /// Core: PAKE rejected a share.
    KL_ERR_PAKE = 8,
    /// Core: key confirmation failed (wrong code or tampering).
    KL_ERR_CONFIRMATION_FAILED = 9,
    /// Core: code is not exactly six ASCII digits.
    KL_ERR_BAD_CODE = 10,
    /// Core: AEAD authentication failed.
    KL_ERR_AUTHENTICATION = 11,
    /// Core: fragment duplicated, missing, reordered or out of bounds.
    KL_ERR_FRAMING = 12,
    /// Core: length or kind not allowed.
    KL_ERR_LIMITS = 13,
    /// Core: counter space exhausted.
    KL_ERR_COUNTER_EXHAUSTED = 14,
    /// Core: explicit one-transfer approval missing or consumed.
    KL_ERR_NOT_APPROVED = 15,
    /// Core: channel closed.
    KL_ERR_CLOSED = 16,
    /// Core: OS randomness unavailable.
    KL_ERR_RANDOMNESS = 17,
    /// A required pointer was NULL (or a non-empty input had a NULL pointer).
    KL_ERR_NULL = 100,
    /// Output buffer too small; `*out_len` now holds the required size.
    KL_ERR_BUFFER = 101,
    /// The handle is in use by another call (no blocking).
    KL_ERR_BUSY = 102,
    /// An internal panic was caught; treat the handle as unusable.
    KL_ERR_PANIC = 103,
    /// ABI version mismatch (reserved for host bindings to report).
    KL_ERR_ABI = 104,
}

use kl_status_t::*;

impl From<Error> for kl_status_t {
    fn from(e: Error) -> Self {
        match e {
            Error::Expired => KL_ERR_EXPIRED,
            Error::NotActive => KL_ERR_NOT_ACTIVE,
            Error::TooManyFailures => KL_ERR_TOO_MANY_FAILURES,
            Error::AttemptInProgress => KL_ERR_ATTEMPT_IN_PROGRESS,
            Error::Malformed => KL_ERR_MALFORMED,
            Error::Unsupported => KL_ERR_UNSUPPORTED,
            Error::WrongState => KL_ERR_WRONG_STATE,
            Error::Pake => KL_ERR_PAKE,
            Error::ConfirmationFailed => KL_ERR_CONFIRMATION_FAILED,
            Error::BadCode => KL_ERR_BAD_CODE,
            Error::Authentication => KL_ERR_AUTHENTICATION,
            Error::Framing => KL_ERR_FRAMING,
            Error::Limits => KL_ERR_LIMITS,
            Error::CounterExhausted => KL_ERR_COUNTER_EXHAUSTED,
            Error::NotApproved => KL_ERR_NOT_APPROVED,
            Error::Closed => KL_ERR_CLOSED,
            Error::Randomness => KL_ERR_RANDOMNESS,
        }
    }
}

type St = kl_status_t;
type Res<T> = Result<T, St>;

/// Opaque receiver handle (wraps `ReceiverSession`).
pub struct kl_receiver_t {
    inner: Mutex<ReceiverSession>,
    id_len: usize,
    claims_len: usize,
}

/// Opaque sender handle (wraps `SenderPending`; emptied by `kl_sender_finish`).
pub struct kl_sender_t {
    inner: Mutex<Option<SenderPending>>,
}

struct ChannelState {
    ch: SecureChannel,
    /// A completed message that did not fit the caller buffer (collect with a 0-length call).
    held: Option<Message>,
}

/// Opaque channel handle (wraps `SecureChannel`).
pub struct kl_channel_t {
    inner: Mutex<ChannelState>,
}

// ---------------------------------------------------------------------------------------
// Internal helpers (all `unsafe` is concentrated here and in the exports' pointer reads).
// ---------------------------------------------------------------------------------------

fn run(f: impl FnOnce() -> Res<()>) -> St {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => KL_OK,
        Ok(Err(s)) => s,
        Err(_) => KL_ERR_PANIC,
    }
}

fn lock<T>(m: &Mutex<T>) -> Res<MutexGuard<'_, T>> {
    match m.try_lock() {
        Ok(g) => Ok(g),
        Err(TryLockError::WouldBlock) => Err(KL_ERR_BUSY),
        Err(TryLockError::Poisoned(_)) => Err(KL_ERR_PANIC),
    }
}

/// # Safety
/// `p` must be NULL or valid for reads of `len` bytes for the returned lifetime.
unsafe fn in_slice<'a>(p: *const u8, len: usize) -> Res<&'a [u8]> {
    if len > MAX_IN {
        return Err(KL_ERR_LIMITS);
    }
    if len == 0 {
        return Ok(&[]);
    }
    if p.is_null() {
        return Err(KL_ERR_NULL);
    }
    // SAFETY: non-null, len bounded by MAX_IN; the caller guarantees `len` readable bytes.
    Ok(unsafe { std::slice::from_raw_parts(p, len) })
}

/// # Safety
/// `p` must be NULL or point to a live handle created by this library.
unsafe fn handle<'a, T>(p: *const T) -> Res<&'a T> {
    // SAFETY: caller guarantees NULL or a live handle; NULL is turned into an error.
    unsafe { p.as_ref() }.ok_or(KL_ERR_NULL)
}

/// Validate an out-handle slot and set it to NULL.
///
/// # Safety
/// `slot` must be NULL or valid for a pointer-sized write.
unsafe fn clear_slot<T>(slot: *mut *mut T) -> Res<()> {
    if slot.is_null() {
        return Err(KL_ERR_NULL);
    }
    // SAFETY: non-null, caller guarantees it is writable.
    unsafe { *slot = std::ptr::null_mut() };
    Ok(())
}

/// A validated caller output buffer with enough capacity.
struct OutBuf {
    ptr: *mut u8,
    len_ptr: *mut usize,
    cap: usize,
}

impl OutBuf {
    /// Check capacity for `required` bytes. Too small: write the required size, return BUFFER.
    ///
    /// # Safety
    /// `out_len` must be NULL or valid for read/write; `out` NULL or valid for `*out_len` bytes.
    unsafe fn prep(out: *mut u8, out_len: *mut usize, required: usize) -> Res<Self> {
        if out_len.is_null() {
            return Err(KL_ERR_NULL);
        }
        // SAFETY: non-null and caller-guaranteed readable.
        let cap = unsafe { *out_len };
        if cap < required {
            // SAFETY: non-null and caller-guaranteed writable.
            unsafe { *out_len = required };
            return Err(KL_ERR_BUFFER);
        }
        if out.is_null() && required > 0 {
            return Err(KL_ERR_NULL);
        }
        Ok(Self {
            ptr: out,
            len_ptr: out_len,
            cap,
        })
    }

    /// Copy `data` out and set the length. Never exceeds the validated capacity.
    ///
    /// # Safety
    /// Same contract as `prep`.
    unsafe fn put(&self, data: &[u8]) -> Res<()> {
        if data.len() > self.cap {
            return Err(KL_ERR_PANIC); // size precheck bug: fail closed
        }
        if !data.is_empty() {
            // SAFETY: ptr non-null (checked in prep), cap >= data.len(), caller-owned buffer.
            unsafe { std::ptr::copy(data.as_ptr(), self.ptr, data.len()) };
        }
        // SAFETY: len_ptr non-null (checked in prep).
        unsafe { *self.len_ptr = data.len() };
        Ok(())
    }
}

fn m1_len(id: usize, claims: usize) -> usize {
    // magic(2) version(1) kind(1) suite lv, sid, nonce, id lv, claims lv
    4 + 2 + SUITE.len() + SID_LEN + NONCE_LEN + 2 + id + 2 + claims
}

fn m2_len(id: usize, purpose: usize, claims: usize) -> usize {
    let hello_s = 4 + 2 + SUITE.len() + NONCE_LEN + 2 + id + 2 + purpose + 2 + claims;
    1 + 2 + hello_s + SHARE_LEN
}

const M3_LEN: usize = 1 + SHARE_LEN + TAG_LEN;
const M4_LEN: usize = 1 + TAG_LEN;

// ---------------------------------------------------------------------------------------
// General
// ---------------------------------------------------------------------------------------

/// Returns the ABI version (currently 1). Hosts must refuse to run on mismatch.
#[no_mangle]
pub extern "C" fn kl_abi_version() -> u32 {
    KL_ABI_VERSION
}

/// Overwrite `len` bytes at `ptr` with zeros using volatile semantics. NULL or 0 is a no-op.
///
/// # Safety
/// `ptr` must be NULL or valid for writes of `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn kl_secure_wipe(ptr: *mut u8, len: usize) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if ptr.is_null() || len == 0 {
            return;
        }
        // SAFETY: non-null; the caller guarantees `len` writable bytes.
        let s = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
        s.zeroize();
    }));
}

// ---------------------------------------------------------------------------------------
// Receiver
// ---------------------------------------------------------------------------------------

/// Create a receiver session with a fresh code and a 60 s deadline.
///
/// # Safety
/// Pointers must be NULL or valid for the given lengths; `out` writable.
#[no_mangle]
pub unsafe extern "C" fn kl_receiver_new(
    device_id: *const u8,
    device_id_len: usize,
    claims: *const u8,
    claims_len: usize,
    out: *mut *mut kl_receiver_t,
) -> kl_status_t {
    run(|| {
        // SAFETY: `out` validated inside; inputs validated by in_slice.
        unsafe { clear_slot(out)? };
        let (id, cl) = unsafe {
            (
                in_slice(device_id, device_id_len)?,
                in_slice(claims, claims_len)?,
            )
        };
        let mut rng = os_rng();
        let s = ReceiverSession::new(Arc::new(SystemClock::new()), &mut rng, id, cl)?;
        let b = Box::new(kl_receiver_t {
            inner: Mutex::new(s),
            id_len: id.len(),
            claims_len: cl.len(),
        });
        // SAFETY: `out` was validated non-null and writable above.
        unsafe { *out = Box::into_raw(b) };
        Ok(())
    })
}

/// Write the six ASCII code digits (no NUL) for display while the session is active.
///
/// # Safety
/// `r` a live handle or NULL; `out`/`out_len` per the buffer convention.
#[no_mangle]
pub unsafe extern "C" fn kl_receiver_code(
    r: *mut kl_receiver_t,
    out: *mut u8,
    out_len: *mut usize,
) -> kl_status_t {
    run(|| {
        // SAFETY: pointers validated by the helpers.
        let r = unsafe { handle(r)? };
        let ob = unsafe { OutBuf::prep(out, out_len, 6)? };
        let g = lock(&r.inner)?;
        let code = g.code()?;
        // SAFETY: ob validated with capacity >= 6.
        unsafe { ob.put(code.as_bytes()) }
    })
}

/// Start an attempt and write M1.
///
/// # Safety
/// As for `kl_receiver_code`.
#[no_mangle]
pub unsafe extern "C" fn kl_receiver_begin_attempt(
    r: *mut kl_receiver_t,
    out_m1: *mut u8,
    out_len: *mut usize,
) -> kl_status_t {
    run(|| {
        // SAFETY: pointers validated by the helpers.
        let r = unsafe { handle(r)? };
        let ob = unsafe { OutBuf::prep(out_m1, out_len, m1_len(r.id_len, r.claims_len))? };
        let mut g = lock(&r.inner)?;
        let mut rng = os_rng();
        let m1 = g.begin_attempt(&mut rng)?;
        // SAFETY: ob validated.
        unsafe { ob.put(&m1) }
    })
}

/// Process M2 and write M3.
///
/// # Safety
/// As for `kl_receiver_code`; `m2` valid for `m2_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn kl_receiver_handle_m2(
    r: *mut kl_receiver_t,
    m2: *const u8,
    m2_len: usize,
    out_m3: *mut u8,
    out_len: *mut usize,
) -> kl_status_t {
    run(|| {
        // SAFETY: pointers validated by the helpers.
        let r = unsafe { handle(r)? };
        let m2 = unsafe { in_slice(m2, m2_len)? };
        let ob = unsafe { OutBuf::prep(out_m3, out_len, M3_LEN)? };
        let mut g = lock(&r.inner)?;
        let mut rng = os_rng();
        let m3 = g.handle_m2(&mut rng, m2)?;
        // SAFETY: ob validated.
        unsafe { ob.put(&m3) }
    })
}

/// Verify M4; on success consumes the code and returns a channel handle in `*out_channel`.
///
/// # Safety
/// As for `kl_receiver_code`; `out_channel` writable.
#[no_mangle]
pub unsafe extern "C" fn kl_receiver_finish(
    r: *mut kl_receiver_t,
    m4: *const u8,
    m4_len: usize,
    out_channel: *mut *mut kl_channel_t,
) -> kl_status_t {
    run(|| {
        // SAFETY: pointers validated by the helpers.
        unsafe { clear_slot(out_channel)? };
        let r = unsafe { handle(r)? };
        let m4 = unsafe { in_slice(m4, m4_len)? };
        let mut g = lock(&r.inner)?;
        let ch = g.handle_m4(m4)?;
        drop(g);
        // SAFETY: out_channel validated non-null and writable above.
        unsafe { *out_channel = new_channel(ch) };
        Ok(())
    })
}

/// Free a receiver handle. NULL is a no-op. The handle must not be used afterwards.
///
/// # Safety
/// `r` NULL or a handle from `kl_receiver_new` not yet freed.
#[no_mangle]
pub unsafe extern "C" fn kl_receiver_free(r: *mut kl_receiver_t) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !r.is_null() {
            // SAFETY: created by Box::into_raw in kl_receiver_new; freed once (caller contract).
            drop(unsafe { Box::from_raw(r) });
        }
    }));
}

// ---------------------------------------------------------------------------------------
// Sender
// ---------------------------------------------------------------------------------------

/// Process M1 and write M2; returns a pending sender handle.
///
/// # Safety
/// Pointers NULL or valid for their lengths; `out_sender` writable.
#[no_mangle]
pub unsafe extern "C" fn kl_sender_start(
    code: *const u8,
    code_len: usize,
    m1: *const u8,
    m1_len: usize,
    device_id: *const u8,
    device_id_len: usize,
    purpose: *const u8,
    purpose_len: usize,
    claims: *const u8,
    claims_len: usize,
    out_m2: *mut u8,
    out_len: *mut usize,
    out_sender: *mut *mut kl_sender_t,
) -> kl_status_t {
    run(|| {
        // SAFETY: all pointers validated by the helpers.
        unsafe { clear_slot(out_sender)? };
        let code_b = unsafe { in_slice(code, code_len)? };
        let m1 = unsafe { in_slice(m1, m1_len)? };
        let id = unsafe { in_slice(device_id, device_id_len)? };
        let pu = unsafe { in_slice(purpose, purpose_len)? };
        let cl = unsafe { in_slice(claims, claims_len)? };
        let ob = unsafe { OutBuf::prep(out_m2, out_len, m2_len(id.len(), pu.len(), cl.len()))? };
        let code = std::str::from_utf8(code_b)
            .map_err(|_| KL_ERR_BAD_CODE)
            .and_then(|t| Code::parse(t).map_err(St::from))?;
        let mut rng = os_rng();
        let (m2, pending) = SenderPending::start(
            Arc::new(SystemClock::new()),
            &mut rng,
            &code,
            m1,
            id,
            pu,
            cl,
        )?;
        // SAFETY: ob validated.
        unsafe { ob.put(&m2)? };
        let b = Box::new(kl_sender_t {
            inner: Mutex::new(Some(pending)),
        });
        // SAFETY: out_sender validated non-null and writable above.
        unsafe { *out_sender = Box::into_raw(b) };
        Ok(())
    })
}

/// Verify M3, write M4 and return the channel. The pending protocol state is CONSUMED on
/// every outcome except NULL/BUFFER/BUSY (checked before any state is touched). The handle
/// shell must still be released with `kl_sender_free`; reuse returns `KL_ERR_WRONG_STATE`.
///
/// # Safety
/// `s` live handle or NULL; other pointers per convention.
#[no_mangle]
pub unsafe extern "C" fn kl_sender_finish(
    s: *mut kl_sender_t,
    m3: *const u8,
    m3_len: usize,
    out_m4: *mut u8,
    out_len: *mut usize,
    out_channel: *mut *mut kl_channel_t,
) -> kl_status_t {
    run(|| {
        // SAFETY: pointers validated by the helpers.
        unsafe { clear_slot(out_channel)? };
        let s = unsafe { handle(s)? };
        let m3 = unsafe { in_slice(m3, m3_len)? };
        let ob = unsafe { OutBuf::prep(out_m4, out_len, M4_LEN)? };
        let mut g = lock(&s.inner)?;
        let pending = g.take().ok_or(KL_ERR_WRONG_STATE)?;
        drop(g);
        let (m4, ch) = pending.handle_m3(m3)?;
        // SAFETY: ob validated.
        unsafe { ob.put(&m4)? };
        // SAFETY: out_channel validated non-null and writable above.
        unsafe { *out_channel = new_channel(ch) };
        Ok(())
    })
}

/// Free a sender handle. NULL is a no-op; also valid after `kl_sender_finish`.
///
/// # Safety
/// `s` NULL or a handle from `kl_sender_start` not yet freed.
#[no_mangle]
pub unsafe extern "C" fn kl_sender_free(s: *mut kl_sender_t) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !s.is_null() {
            // SAFETY: created by Box::into_raw in kl_sender_start; freed once (caller contract).
            drop(unsafe { Box::from_raw(s) });
        }
    }));
}

// ---------------------------------------------------------------------------------------
// Channel
// ---------------------------------------------------------------------------------------

fn new_channel(ch: SecureChannel) -> *mut kl_channel_t {
    Box::into_raw(Box::new(kl_channel_t {
        inner: Mutex::new(ChannelState { ch, held: None }),
    }))
}

/// Sender: grant exactly one SECRET transfer. Call only after local user approval.
///
/// # Safety
/// `c` live handle or NULL.
#[no_mangle]
pub unsafe extern "C" fn kl_channel_approve_one_transfer(c: *mut kl_channel_t) -> kl_status_t {
    run(|| {
        // SAFETY: validated by `handle`.
        let c = unsafe { handle(c)? };
        lock(&c.inner)?.ch.approve_one_transfer();
        Ok(())
    })
}

/// Receiver: declare (nonzero) or withdraw (0) readiness to accept a SECRET message.
///
/// # Safety
/// `c` live handle or NULL.
#[no_mangle]
pub unsafe extern "C" fn kl_channel_set_ready_to_receive(
    c: *mut kl_channel_t,
    ready: u8,
) -> kl_status_t {
    run(|| {
        // SAFETY: validated by `handle`.
        let c = unsafe { handle(c)? };
        lock(&c.inner)?.ch.set_ready_to_receive(ready != 0);
        Ok(())
    })
}

fn sealed_len(pt_len: usize, max_frag: usize) -> Option<(usize, usize)> {
    if pt_len == 0 || pt_len > MAX_MESSAGE {
        return None;
    }
    let pm = max_frag.clamp(1, MAX_FRAGMENT_PAYLOAD);
    let ct = pt_len + AEAD_TAG;
    let count = ct.div_ceil(pm);
    if count > MAX_FRAGMENTS {
        return None;
    }
    Some((count, ct + count * (FRAGMENT_HEADER + LEN_PREFIX)))
}

/// Encrypt and fragment one message. Output is repeated `[u32 big-endian length][fragment]`;
/// `*out_count` receives the fragment count. Nothing is consumed on `KL_ERR_BUFFER`.
///
/// # Safety
/// `c` live handle or NULL; `plaintext` valid for `plaintext_len`; `out`, `out_len`,
/// `out_count` per convention.
#[no_mangle]
pub unsafe extern "C" fn kl_channel_seal(
    c: *mut kl_channel_t,
    kind: u8,
    plaintext: *const u8,
    plaintext_len: usize,
    max_fragment_payload: usize,
    out: *mut u8,
    out_len: *mut usize,
    out_count: *mut usize,
) -> kl_status_t {
    run(|| {
        // SAFETY: pointers validated by the helpers.
        let c = unsafe { handle(c)? };
        let pt = unsafe { in_slice(plaintext, plaintext_len)? };
        if out_count.is_null() {
            return Err(KL_ERR_NULL);
        }
        let required = sealed_len(pt.len(), max_fragment_payload);
        // Invalid sizes skip the precheck: core then reports Limits without consuming.
        let ob = match required {
            Some((_, total)) => Some(unsafe { OutBuf::prep(out, out_len, total)? }),
            None => {
                if out_len.is_null() {
                    return Err(KL_ERR_NULL);
                }
                None
            }
        };
        let mut g = lock(&c.inner)?;
        let frags = g.ch.seal(kind, pt, max_fragment_payload)?;
        let mut packed = Vec::with_capacity(frags.len() * 8);
        for f in &frags {
            let l = u32::try_from(f.len()).map_err(|_| KL_ERR_LIMITS)?;
            packed.extend_from_slice(&l.to_be_bytes());
            packed.extend_from_slice(f);
        }
        let Some(ob) = ob else {
            g.ch.close();
            return Err(KL_ERR_PANIC);
        };
        // SAFETY: ob validated.
        if let Err(e) = unsafe { ob.put(&packed) } {
            g.ch.close(); // precheck mismatch: counter already used, fail closed
            return Err(e);
        }
        // SAFETY: out_count validated non-null above, caller-writable.
        unsafe { *out_count = frags.len() };
        Ok(())
    })
}

/// Feed one received fragment. `*out_complete` is 1 only when a full authenticated message
/// was written to `out_plain` (with its kind in `*out_kind`); otherwise 0 and `*out_len`=0.
/// Any error closes the channel. If a completed message did not fit (`KL_ERR_BUFFER`) it is
/// held inside the channel: call again with `fragment_len == 0` and a larger buffer.
///
/// # Safety
/// `c` live handle or NULL; other pointers per convention (`out_plain` may be NULL while
/// `*out_len` is 0 for a size query).
#[no_mangle]
pub unsafe extern "C" fn kl_channel_open_fragment(
    c: *mut kl_channel_t,
    fragment: *const u8,
    fragment_len: usize,
    out_plain: *mut u8,
    out_len: *mut usize,
    out_kind: *mut u8,
    out_complete: *mut u8,
) -> kl_status_t {
    run(|| {
        // SAFETY: pointers validated by the helpers.
        let c = unsafe { handle(c)? };
        let frag = unsafe { in_slice(fragment, fragment_len)? };
        if out_len.is_null() || out_kind.is_null() || out_complete.is_null() {
            return Err(KL_ERR_NULL);
        }
        // SAFETY: out_complete validated non-null.
        unsafe { *out_complete = 0 };
        let mut g = lock(&c.inner)?;

        let msg = if let Some(held) = g.held.take() {
            if !frag.is_empty() {
                g.ch.close();
                return Err(KL_ERR_FRAMING);
            }
            held
        } else {
            match g.ch.open_fragment(frag) {
                Ok(Some(m)) => m,
                Ok(None) => {
                    // SAFETY: out_len validated non-null.
                    unsafe { *out_len = 0 };
                    return Ok(());
                }
                Err(e) => return Err(e.into()),
            }
        };
        // SAFETY: out_len validated; buffer validated against the plaintext size.
        let ob = match unsafe { OutBuf::prep(out_plain, out_len, msg.plaintext.len()) } {
            Ok(ob) => ob,
            Err(e) => {
                g.held = Some(msg);
                return Err(e);
            }
        };
        // SAFETY: ob validated.
        unsafe { ob.put(&msg.plaintext)? };
        // SAFETY: out_kind/out_complete validated non-null above.
        unsafe {
            *out_kind = msg.kind;
            *out_complete = 1;
        }
        Ok(())
    })
}

/// Close the channel and erase keys and any partial or held data. NULL is an error.
///
/// # Safety
/// `c` live handle or NULL.
#[no_mangle]
pub unsafe extern "C" fn kl_channel_close(c: *mut kl_channel_t) -> kl_status_t {
    run(|| {
        // SAFETY: validated by `handle`.
        let c = unsafe { handle(c)? };
        let mut g = lock(&c.inner)?;
        g.held = None;
        g.ch.close();
        Ok(())
    })
}

/// 1 if the channel is closed, expired-closed, NULL, busy or otherwise unusable; 0 if open.
///
/// # Safety
/// `c` live handle or NULL.
#[no_mangle]
pub unsafe extern "C" fn kl_channel_is_closed(c: *mut kl_channel_t) -> u8 {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: validated by `handle`.
        let Ok(c) = (unsafe { handle(c) }) else {
            return 1;
        };
        match lock(&c.inner) {
            Ok(g) => u8::from(g.ch.is_closed()),
            Err(_) => 1,
        }
    }))
    .unwrap_or(1)
}

/// Free a channel handle (keys already erased on drop). NULL is a no-op.
///
/// # Safety
/// `c` NULL or a handle from a finish function not yet freed.
#[no_mangle]
pub unsafe extern "C" fn kl_channel_free(c: *mut kl_channel_t) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !c.is_null() {
            // SAFETY: created by Box::into_raw in new_channel; freed once (caller contract).
            drop(unsafe { Box::from_raw(c) });
        }
    }));
}
