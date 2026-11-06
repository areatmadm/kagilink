//! Plain C ABI over `areatm-kagilink-core` (decision D016, docs/FFI_DESIGN.md).
//!
//! This is the only crate in the workspace that may contain `unsafe`. Rules:
//! * opaque handles, created and freed only here; no Rust-allocated memory is returned. A
//!   handle is NOT a pointer: it is an encoded slot+generation token resolved through a global
//!   registry (see the registry section), so stale, doubled and racing frees are errors or
//!   no-ops, never use-after-free;
//! * caller-allocated output buffers (`out`, `*out_len` = capacity in / length out);
//!   a too-small buffer yields `KL_ERR_BUFFER` with the required size and consumes no
//!   protocol state (sizes are checked before the operation, or the result is held);
//! * output buffers are written only on success;
//! * every export null-checks, bounds-checks input lengths, is wrapped in `catch_unwind`
//!   and uses `try_lock` (`KL_ERR_BUSY`);
//! * ABI v2 adds the BLE chunk codec (`kl_frame_*`, `kl_reassembler_*`, `kl_ack_*`); it is
//!   pure and carries opaque bytes, with the same buffer and handle conventions;
//! * no logging, no callbacks, no secret text in any return value; the only global state is the
//!   handle registry (tokens and `Arc`s, no secrets of its own).
//!
//! Synthetic data only until an independent review of this `unsafe` code exists.

#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types)]
#![warn(missing_docs)]
#![allow(clippy::too_many_arguments)]

use areatm_kagilink_core::ble_frame::{self, Reassembler};
use areatm_kagilink_core::transport::{
    Message, SecureChannel, MAX_FRAGMENTS, MAX_FRAGMENT_PAYLOAD, MAX_MESSAGE,
};
use areatm_kagilink_core::wire::{NONCE_LEN, SHARE_LEN, SID_LEN, SUITE, TAG_LEN};
use areatm_kagilink_core::{
    os_rng, Code, Error, ReceiverSession, SenderPending, SystemClock, MAX_FAILURES,
};
use std::any::Any;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, TryLockError};
use zeroize::Zeroize;

/// ABI version returned by [`kl_abi_version`]. Bumped on any signature change.
pub const KL_ABI_VERSION: u32 = 3;

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

/// Opaque receiver handle type. A `kl_receiver_t*` is a registry token, never dereferenced.
#[repr(C)]
pub struct kl_receiver_t {
    _private: [u8; 0],
}

/// Opaque sender handle type (token; the pending state is emptied by `kl_sender_finish`).
#[repr(C)]
pub struct kl_sender_t {
    _private: [u8; 0],
}

/// Opaque channel handle type (token).
#[repr(C)]
pub struct kl_channel_t {
    _private: [u8; 0],
}

/// Opaque BLE chunk reassembler handle type (token).
#[repr(C)]
pub struct kl_reassembler_t {
    _private: [u8; 0],
}

/// Real receiver object; `None` after a free (late in-flight calls then see `KL_ERR_NULL`).
struct ReceiverObj {
    inner: Mutex<Option<ReceiverSession>>,
    id_len: usize,
    claims_len: usize,
}

/// Real sender object; the pending state is `None` after finish or free.
struct SenderObj {
    inner: Mutex<Option<SenderPending>>,
}

struct ChannelState {
    ch: SecureChannel,
    /// A completed message that did not fit the caller buffer (collect with a 0-length call).
    held: Option<Message>,
}

/// Real channel object; closed and wiped by free.
struct ChannelObj {
    inner: Mutex<ChannelState>,
}

struct ReassemblerState {
    r: Reassembler,
    /// A completed message waiting for `kl_reassembler_take`.
    held: Option<(u8, Vec<u8>)>,
}

/// Real reassembler object; `None` after a free.
struct ReasmObj {
    inner: Mutex<Option<ReassemblerState>>,
}

// ---------------------------------------------------------------------------------------
// Handle registry (docs/FFI_DESIGN.md "Handle registry").
//
// A handle handed to C is a token: `(generation << IDX_BITS | slot_index) ^ mask`, cast to the
// pointer type. It is never dereferenced. All lookups go through one global registry:
//  * `resolve` clones the `Arc` under the registry lock and releases the lock BEFORE the caller
//    touches the object, so a concurrent free can never free memory under a live call;
//  * `unregister` (used by every free) removes the slot, bumps its generation (so the old token
//    and every copy of it is dead) and returns the `Arc` for wiping outside the lock;
//  * generation 0 is never issued and `0` is never a valid token, so NULL, small integers and
//    most garbage fail; `mask` (per process, from `RandomState`, NOT a secret or a security
//    boundary) makes accidental hits by arbitrary integers or addresses negligible;
//  * the generation wraps after 2^GEN_BITS reuses of one slot (2^40 on 64-bit, 2^20 on 32-bit
//    targets); a token held across that many free/allocate cycles of the same slot could alias;
//  * LOCK ORDER: the registry lock is a leaf. It is never held while an object mutex is taken or
//    while core code runs, and an object mutex is never held while the registry lock is taken.
//    Dropping an `Arc` (and so running secret zeroization) always happens after the guard is
//    released.
// ---------------------------------------------------------------------------------------

const IDX_BITS: u32 = if usize::BITS >= 64 { 24 } else { 12 };
const GEN_BITS: u32 = usize::BITS - IDX_BITS;
const IDX_MASK: usize = (1usize << IDX_BITS) - 1;
const GEN_MASK: usize = (1usize << GEN_BITS) - 1;

type Obj = Arc<dyn Any + Send + Sync>;

struct Slot {
    gen: usize,
    obj: Option<Obj>,
}

struct Registry {
    slots: Vec<Slot>,
    free: Vec<usize>,
    mask: usize,
    mask_ready: bool,
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry {
    slots: Vec::new(),
    free: Vec::new(),
    mask: 0,
    mask_ready: false,
});

fn registry() -> MutexGuard<'static, Registry> {
    // Registry operations cannot panic while holding the lock; recover a poisoned one anyway.
    REGISTRY.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Registry {
    fn encode(&self, idx: usize, gen: usize) -> usize {
        ((gen << IDX_BITS) | idx) ^ self.mask
    }

    fn insert(&mut self, obj: Obj) -> Res<usize> {
        if !self.mask_ready {
            let mut h = RandomState::new().build_hasher();
            h.write_u8(1);
            self.mask = h.finish() as usize;
            self.mask_ready = true;
        }
        let idx = if let Some(i) = self.free.pop() {
            i
        } else {
            if self.slots.len() >= IDX_MASK {
                return Err(KL_ERR_LIMITS); // too many live handles
            }
            self.slots.push(Slot { gen: 1, obj: None });
            self.slots.len() - 1
        };
        let mut gen = self.slots[idx].gen;
        let mut tok = self.encode(idx, gen);
        while tok == 0 {
            gen = next_gen(gen); // never hand out the NULL value
            tok = self.encode(idx, gen);
        }
        let slot = &mut self.slots[idx];
        slot.gen = gen;
        slot.obj = Some(obj);
        Ok(tok)
    }

    /// Slot index of a live token, or `None` (stale, forged, NULL).
    fn find(&self, tok: usize) -> Option<usize> {
        if tok == 0 || !self.mask_ready {
            return None;
        }
        let v = tok ^ self.mask;
        let idx = v & IDX_MASK;
        let gen = (v >> IDX_BITS) & GEN_MASK;
        let slot = self.slots.get(idx)?;
        (gen != 0 && slot.gen == gen && slot.obj.is_some() && self.encode(idx, gen) == tok)
            .then_some(idx)
    }
}

fn next_gen(g: usize) -> usize {
    let n = g.wrapping_add(1) & GEN_MASK;
    if n == 0 {
        1
    } else {
        n
    }
}

/// Register a new object and return its token.
fn register<T: Any + Send + Sync>(obj: T) -> Res<usize> {
    let obj: Obj = Arc::new(obj);
    registry().insert(obj)
}

/// Look up a token and clone the object. Wrong type, stale, forged and NULL all give NULL.
fn resolve<T: Any + Send + Sync>(tok: usize) -> Res<Arc<T>> {
    let found = {
        let g = registry();
        let idx = g.find(tok).ok_or(KL_ERR_NULL)?;
        g.slots[idx].obj.clone().ok_or(KL_ERR_NULL)?
    }; // guard released before any use of the object
    found.downcast::<T>().map_err(|_| KL_ERR_NULL)
}

/// Remove a live token of type `T` (bumping the slot generation) and return the object.
/// `None` for stale, doubled, forged, NULL or wrong-type tokens (nothing is changed then).
fn unregister<T: Any + Send + Sync>(tok: usize) -> Option<Arc<T>> {
    let taken = {
        let mut g = registry();
        let idx = g.find(tok)?;
        let slot = &mut g.slots[idx];
        let same_type = slot.obj.as_ref().is_some_and(|o| {
            let r: &(dyn Any + Send + Sync) = &**o;
            r.is::<T>()
        });
        if !same_type {
            return None;
        }
        let obj = slot.obj.take();
        slot.gen = next_gen(slot.gen);
        g.free.push(idx);
        obj
    }; // guard released
    taken?.downcast::<T>().ok()
}

/// Objects that can be erased in place while other `Arc` clones may still exist.
trait Wipe {
    /// Erase all secret state. Blocks only until an in-flight call on the same object finishes
    /// (calls never block, so this is short); a poisoned mutex is recovered and still erased.
    fn wipe(&self);
}

impl Wipe for ReceiverObj {
    fn wipe(&self) {
        drop(wipe_lock(&self.inner).take());
    }
}
impl Wipe for SenderObj {
    fn wipe(&self) {
        drop(wipe_lock(&self.inner).take());
    }
}
impl Wipe for ChannelObj {
    fn wipe(&self) {
        wipe_lock(&self.inner).close_and_wipe();
    }
}
impl Wipe for ReasmObj {
    fn wipe(&self) {
        let mut g = wipe_lock(&self.inner);
        if let Some(st) = g.as_mut() {
            if let Some((_, d)) = st.held.as_mut() {
                d.zeroize();
            }
        }
        *g = None;
    }
}

fn wipe_lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Free rule: the token dies first (new calls get `KL_ERR_NULL`), then the object is wiped
/// under its own mutex (after any in-flight call finishes), then the last `Arc` drops.
/// Stale, doubled, NULL, forged and wrong-type tokens are a no-op.
fn release<T: Wipe + Any + Send + Sync>(tok: usize) {
    if let Some(obj) = unregister::<T>(tok) {
        obj.wipe();
        drop(obj);
    }
}

fn token_of<T>(p: *mut T) -> usize {
    p as usize
}

fn token_ptr<T>(t: usize) -> *mut T {
    t as *mut T
}

/// Resolve a handle token of the expected object type.
fn handle<O: Any + Send + Sync, T>(p: *mut T) -> Res<Arc<O>> {
    resolve::<O>(token_of(p))
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

/// Like `lock` but a poisoned mutex is recovered (never blocks): used only on close/zeroize
/// paths so that secrets are still erased after a panic.
fn lock_for_wipe<T>(m: &Mutex<T>) -> Res<MutexGuard<'_, T>> {
    match m.try_lock() {
        Ok(g) => Ok(g),
        Err(TryLockError::WouldBlock) => Err(KL_ERR_BUSY),
        Err(TryLockError::Poisoned(p)) => Ok(p.into_inner()),
    }
}

impl ChannelState {
    /// Close the channel and erase any held plaintext.
    fn close_and_wipe(&mut self) {
        self.held = None; // `Message` plaintext is `Zeroizing`
        self.ch.close();
    }

    /// Drop held plaintext if the channel is (or just became) closed.
    fn wipe_held_if_closed(&mut self) {
        if self.ch.is_closed() {
            self.held = None;
        }
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

/// Returns the ABI version (currently 3). Hosts must refuse to run on mismatch.
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
        let tok = register(ReceiverObj {
            inner: Mutex::new(Some(s)),
            id_len: id.len(),
            claims_len: cl.len(),
        })?;
        // SAFETY: `out` was validated non-null and writable above.
        unsafe { *out = token_ptr(tok) };
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
        let r = handle::<ReceiverObj, _>(r)?;
        // SAFETY: pointers validated by the helpers.
        let ob = unsafe { OutBuf::prep(out, out_len, 6)? };
        let g = lock(&r.inner)?;
        let code = g.as_ref().ok_or(KL_ERR_NULL)?.code()?;
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
        let r = handle::<ReceiverObj, _>(r)?;
        // SAFETY: pointers validated by the helpers.
        let ob = unsafe { OutBuf::prep(out_m1, out_len, m1_len(r.id_len, r.claims_len))? };
        let mut g = lock(&r.inner)?;
        let mut rng = os_rng();
        let m1 = g.as_mut().ok_or(KL_ERR_NULL)?.begin_attempt(&mut rng)?;
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
        let r = handle::<ReceiverObj, _>(r)?;
        // SAFETY: pointers validated by the helpers.
        let m2 = unsafe { in_slice(m2, m2_len)? };
        let ob = unsafe { OutBuf::prep(out_m3, out_len, M3_LEN)? };
        let mut g = lock(&r.inner)?;
        let mut rng = os_rng();
        let m3 = g.as_mut().ok_or(KL_ERR_NULL)?.handle_m2(&mut rng, m2)?;
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
        let r = handle::<ReceiverObj, _>(r)?;
        let m4 = unsafe { in_slice(m4, m4_len)? };
        let mut g = lock(&r.inner)?;
        let ch = g.as_mut().ok_or(KL_ERR_NULL)?.handle_m4(m4)?;
        drop(g); // object mutex released before the registry is touched (lock order)
        let tok = new_channel(ch)?;
        // SAFETY: out_channel validated non-null and writable above.
        unsafe { *out_channel = token_ptr(tok) };
        Ok(())
    })
}

/// Read the receiver's failure budget use: `*out_used` = failed attempts so far plus one if an
/// attempt that already sent M3 is still open (core `failures_including_open`, D026), and
/// `*out_max` = the budget (3). Read-only: no state change, no code or key material. Still works
/// after expiry, success or budget exhaustion (final count, no error); only a NULL/stale handle,
/// a freed receiver or NULL out pointers fail.
///
/// # Safety
/// `r` a live handle or NULL; `out_used` and `out_max` writable or NULL.
#[no_mangle]
pub unsafe extern "C" fn kl_receiver_failures(
    r: *mut kl_receiver_t,
    out_used: *mut u8,
    out_max: *mut u8,
) -> kl_status_t {
    run(|| {
        let r = handle::<ReceiverObj, _>(r)?;
        if out_used.is_null() || out_max.is_null() {
            return Err(KL_ERR_NULL);
        }
        let used = lock(&r.inner)?
            .as_ref()
            .ok_or(KL_ERR_NULL)?
            .failures_including_open();
        // SAFETY: both pointers validated non-null above; the caller guarantees writability.
        unsafe {
            *out_used = used;
            *out_max = MAX_FAILURES;
        }
        Ok(())
    })
}

/// Free a receiver handle. NULL, stale and already-freed handles are a no-op. Secrets are erased
/// before this returns (after an in-flight call on the same handle finishes).
///
/// # Safety
/// Memory-safe for any `r` value; no pointer is ever dereferenced.
#[no_mangle]
pub unsafe extern "C" fn kl_receiver_free(r: *mut kl_receiver_t) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        release::<ReceiverObj>(token_of(r));
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
        let tok = register(SenderObj {
            inner: Mutex::new(Some(pending)),
        })?;
        // SAFETY: ob validated.
        if let Err(e) = unsafe { ob.put(&m2) } {
            release::<SenderObj>(tok);
            return Err(e);
        }
        // SAFETY: out_sender validated non-null and writable above.
        unsafe { *out_sender = token_ptr(tok) };
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
        let s = handle::<SenderObj, _>(s)?;
        let m3 = unsafe { in_slice(m3, m3_len)? };
        let ob = unsafe { OutBuf::prep(out_m4, out_len, M4_LEN)? };
        let mut g = lock(&s.inner)?;
        let pending = g.take().ok_or(KL_ERR_WRONG_STATE)?;
        drop(g);
        let (m4, ch) = pending.handle_m3(m3)?;
        let tok = new_channel(ch)?;
        // SAFETY: ob validated.
        if let Err(e) = unsafe { ob.put(&m4) } {
            release::<ChannelObj>(tok);
            return Err(e);
        }
        // SAFETY: out_channel validated non-null and writable above.
        unsafe { *out_channel = token_ptr(tok) };
        Ok(())
    })
}

/// Free a sender handle. NULL, stale and already-freed handles are a no-op; also valid after
/// `kl_sender_finish`.
///
/// # Safety
/// Memory-safe for any `s` value; no pointer is ever dereferenced.
#[no_mangle]
pub unsafe extern "C" fn kl_sender_free(s: *mut kl_sender_t) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        release::<SenderObj>(token_of(s));
    }));
}

// ---------------------------------------------------------------------------------------
// Channel
// ---------------------------------------------------------------------------------------

fn new_channel(ch: SecureChannel) -> Res<usize> {
    register(ChannelObj {
        inner: Mutex::new(ChannelState { ch, held: None }),
    })
}

/// Sender: grant exactly one SECRET transfer. Call only after local user approval.
///
/// # Safety
/// `c` live handle or NULL.
#[no_mangle]
pub unsafe extern "C" fn kl_channel_approve_one_transfer(c: *mut kl_channel_t) -> kl_status_t {
    run(|| {
        let c = handle::<ChannelObj, _>(c)?;
        let mut g = lock(&c.inner)?;
        if g.ch.poll_closed() {
            g.held = None;
            return Err(KL_ERR_CLOSED);
        }
        g.ch.approve_one_transfer();
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
        let c = handle::<ChannelObj, _>(c)?;
        let mut g = lock(&c.inner)?;
        if g.ch.poll_closed() {
            g.held = None;
            return Err(KL_ERR_CLOSED);
        }
        g.ch.set_ready_to_receive(ready != 0);
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
        let c = handle::<ChannelObj, _>(c)?;
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
        let sealed = g.ch.seal(kind, pt, max_fragment_payload);
        g.wipe_held_if_closed(); // expiry / counter exhaustion close the channel
        let frags = sealed?;
        let mut packed = Vec::with_capacity(frags.len() * 8);
        for f in &frags {
            let l = u32::try_from(f.len()).map_err(|_| KL_ERR_LIMITS)?;
            packed.extend_from_slice(&l.to_be_bytes());
            packed.extend_from_slice(f);
        }
        let Some(ob) = ob else {
            g.close_and_wipe();
            return Err(KL_ERR_PANIC);
        };
        // SAFETY: ob validated.
        if let Err(e) = unsafe { ob.put(&packed) } {
            g.close_and_wipe(); // precheck mismatch: counter already used, fail closed
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
        let c = handle::<ChannelObj, _>(c)?;
        let frag = unsafe { in_slice(fragment, fragment_len)? };
        if out_len.is_null() || out_kind.is_null() || out_complete.is_null() {
            return Err(KL_ERR_NULL);
        }
        // SAFETY: out_complete validated non-null.
        unsafe { *out_complete = 0 };
        let mut g = lock(&c.inner)?;

        let msg = if let Some(held) = g.held.take() {
            // A closed or expired channel never releases a completed plaintext.
            if g.ch.poll_closed() {
                drop(held); // zeroized on drop
                return Err(KL_ERR_CLOSED);
            }
            if !frag.is_empty() {
                drop(held);
                g.close_and_wipe();
                return Err(KL_ERR_FRAMING);
            }
            held
        } else {
            let opened = g.ch.open_fragment(frag);
            g.wipe_held_if_closed();
            match opened {
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
        let c = handle::<ChannelObj, _>(c)?;
        // Poisoned mutexes are recovered so keys and held data are still erased.
        let mut g = lock_for_wipe(&c.inner)?;
        g.close_and_wipe();
        Ok(())
    })
}

/// 1 if the channel is closed, expired (the 60 s deadline is evaluated; an expired channel is
/// closed on the spot), NULL, busy or otherwise unusable; 0 if open.
///
/// # Safety
/// `c` live handle or NULL.
#[no_mangle]
pub unsafe extern "C" fn kl_channel_is_closed(c: *mut kl_channel_t) -> u8 {
    catch_unwind(AssertUnwindSafe(|| {
        let Ok(c) = handle::<ChannelObj, _>(c) else {
            return 1;
        };
        let r = match lock(&c.inner) {
            Ok(mut g) => {
                let closed = g.ch.poll_closed();
                g.wipe_held_if_closed();
                u8::from(closed)
            }
            Err(_) => 1,
        };
        r
    }))
    .unwrap_or(1)
}

/// Free a channel handle: keys and held plaintext are erased before this returns (after an
/// in-flight call on the same handle finishes). NULL, stale and doubled frees are a no-op.
///
/// # Safety
/// Memory-safe for any `c` value; no pointer is ever dereferenced.
#[no_mangle]
pub unsafe extern "C" fn kl_channel_free(c: *mut kl_channel_t) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        release::<ChannelObj>(token_of(c));
    }));
}

// ---------------------------------------------------------------------------------------
// BLE chunk codec (docs/BLE_GATT_SPEC.md section 5). Pure, no secret state of its own; the
// messages it carries are handshake messages or AEAD fragments.
// ---------------------------------------------------------------------------------------

/// Split one BLE message into chunks of at most `max_chunk` bytes (6..=512). `out` receives
/// repeated `[u32 big-endian length][chunk]`; `*out_count` receives the chunk count. Nothing is
/// consumed on any error (the codec is stateless); `KL_ERR_BUFFER` reports the required size.
///
/// # Safety
/// `msg` valid for `msg_len` bytes (or NULL with 0); `out`, `out_len`, `out_count` per convention.
#[no_mangle]
pub unsafe extern "C" fn kl_frame_chunk(
    mtype: u8,
    msg: *const u8,
    msg_len: usize,
    max_chunk: usize,
    out: *mut u8,
    out_len: *mut usize,
    out_count: *mut usize,
) -> kl_status_t {
    run(|| {
        // SAFETY: pointers validated by the helpers.
        let m = unsafe { in_slice(msg, msg_len)? };
        if out_count.is_null() || out_len.is_null() {
            return Err(KL_ERR_NULL);
        }
        let chunks = ble_frame::chunk_message(mtype, m, max_chunk)?;
        let total: usize = chunks.iter().map(|c| LEN_PREFIX + c.len()).sum();
        // SAFETY: out_len validated non-null; buffer checked against `total`.
        let ob = unsafe { OutBuf::prep(out, out_len, total)? };
        let mut packed = Vec::with_capacity(total);
        for c in &chunks {
            let l = u32::try_from(c.len()).map_err(|_| KL_ERR_LIMITS)?;
            packed.extend_from_slice(&l.to_be_bytes());
            packed.extend_from_slice(c);
        }
        // SAFETY: ob validated with capacity >= total == packed.len().
        unsafe { ob.put(&packed)? };
        // SAFETY: out_count validated non-null above, caller-writable.
        unsafe { *out_count = chunks.len() };
        Ok(())
    })
}

/// Create a reassembler waiting for a FIRST chunk. On failure `*out` is NULL.
///
/// # Safety
/// `out` must be NULL or writable.
#[no_mangle]
pub unsafe extern "C" fn kl_reassembler_new(out: *mut *mut kl_reassembler_t) -> kl_status_t {
    run(|| {
        // SAFETY: `out` validated inside.
        unsafe { clear_slot(out)? };
        let tok = register(ReasmObj {
            inner: Mutex::new(Some(ReassemblerState {
                r: Reassembler::new(),
                held: None,
            })),
        })?;
        // SAFETY: `out` was validated non-null and writable above.
        unsafe { *out = token_ptr(tok) };
        Ok(())
    })
}

/// Feed one received chunk (an ACK chunk is rejected: filter with `kl_parse_ack` first).
/// `*out_complete` is 1 only when this chunk completed a message, which is then held inside the
/// handle until `kl_reassembler_take`; otherwise 0 (need more). Any codec error is final: the
/// reassembler is dead and later pushes return `KL_ERR_CLOSED`. While a completed message is
/// still held, push returns `KL_ERR_WRONG_STATE` and changes nothing.
///
/// # Safety
/// `h` live handle or NULL; `chunk` valid for `chunk_len` bytes (or NULL with 0);
/// `out_complete` writable.
#[no_mangle]
pub unsafe extern "C" fn kl_reassembler_push(
    h: *mut kl_reassembler_t,
    chunk: *const u8,
    chunk_len: usize,
    out_complete: *mut u8,
) -> kl_status_t {
    run(|| {
        let h = handle::<ReasmObj, _>(h)?;
        // SAFETY: pointers validated by the helpers.
        let chunk = unsafe { in_slice(chunk, chunk_len)? };
        if out_complete.is_null() {
            return Err(KL_ERR_NULL);
        }
        // SAFETY: out_complete validated non-null.
        unsafe { *out_complete = 0 };
        let mut guard = lock(&h.inner)?;
        let g = guard.as_mut().ok_or(KL_ERR_NULL)?;
        if g.held.is_some() {
            return Err(KL_ERR_WRONG_STATE);
        }
        if let Some(m) = g.r.push(chunk)? {
            g.held = Some(m);
            // SAFETY: out_complete validated non-null above.
            unsafe { *out_complete = 1 };
        }
        Ok(())
    })
}

/// Copy the completed message out: `*out_mtype` receives the message type and `out` the data
/// (`*out_len` = capacity in, length out). Too small returns `KL_ERR_BUFFER` with the required
/// size and keeps the message held. On success the held copy is erased. With nothing held it
/// returns `KL_ERR_WRONG_STATE`.
///
/// # Safety
/// `h` live handle or NULL; `out_mtype` writable; `out`, `out_len` per convention.
#[no_mangle]
pub unsafe extern "C" fn kl_reassembler_take(
    h: *mut kl_reassembler_t,
    out_mtype: *mut u8,
    out: *mut u8,
    out_len: *mut usize,
) -> kl_status_t {
    run(|| {
        let h = handle::<ReasmObj, _>(h)?;
        if out_mtype.is_null() {
            return Err(KL_ERR_NULL);
        }
        let mut guard = lock(&h.inner)?;
        let g = guard.as_mut().ok_or(KL_ERR_NULL)?;
        let need = g.held.as_ref().ok_or(KL_ERR_WRONG_STATE)?.1.len();
        // SAFETY: out_len validated inside; the held message is kept on BUFFER/NULL errors.
        let ob = unsafe { OutBuf::prep(out, out_len, need)? };
        let (t, mut data) = g.held.take().ok_or(KL_ERR_WRONG_STATE)?;
        // SAFETY: ob validated with capacity >= data.len().
        let r = unsafe { ob.put(&data) };
        data.zeroize();
        r?;
        // SAFETY: out_mtype validated non-null above.
        unsafe { *out_mtype = t };
        Ok(())
    })
}

/// 1 if the reassembler is dead (a codec error occurred), NULL, busy or unusable; 0 otherwise.
///
/// # Safety
/// `h` live handle or NULL.
#[no_mangle]
pub unsafe extern "C" fn kl_reassembler_is_dead(h: *mut kl_reassembler_t) -> u8 {
    catch_unwind(AssertUnwindSafe(|| {
        let Ok(h) = handle::<ReasmObj, _>(h) else {
            return 1;
        };
        let r = match lock(&h.inner) {
            Ok(g) => g.as_ref().map_or(1, |g| u8::from(g.r.is_dead())),
            Err(_) => 1,
        };
        r
    }))
    .unwrap_or(1)
}

/// Free a reassembler handle (any held message is erased). NULL, stale and doubled frees are a
/// no-op.
///
/// # Safety
/// Memory-safe for any `h` value; no pointer is ever dereferenced.
#[no_mangle]
pub unsafe extern "C" fn kl_reassembler_free(h: *mut kl_reassembler_t) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        release::<ReasmObj>(token_of(h));
    }));
}

/// Write the 2-byte application ACK chunk for `seq` (`*out_len` = capacity in, 2 out).
///
/// # Safety
/// `out`, `out_len` per convention.
#[no_mangle]
pub unsafe extern "C" fn kl_ack_chunk(seq: u8, out: *mut u8, out_len: *mut usize) -> kl_status_t {
    run(|| {
        // SAFETY: pointers validated by prep.
        let ob = unsafe { OutBuf::prep(out, out_len, 2)? };
        // SAFETY: ob validated with capacity >= 2.
        unsafe { ob.put(&ble_frame::ack_chunk(seq)) }
    })
}

/// Classify a received chunk: `*out_is_ack` is 1 and `*out_seq` the acknowledged sequence number
/// only when the chunk is exactly an ACK chunk; otherwise both are 0 and the status is still
/// `KL_OK` (not an ACK is not an error: pass the chunk to the reassembler).
///
/// # Safety
/// `chunk` valid for `chunk_len` bytes (or NULL with 0); `out_is_ack`, `out_seq` writable.
#[no_mangle]
pub unsafe extern "C" fn kl_parse_ack(
    chunk: *const u8,
    chunk_len: usize,
    out_is_ack: *mut u8,
    out_seq: *mut u8,
) -> kl_status_t {
    run(|| {
        // SAFETY: pointers validated by the helpers.
        let c = unsafe { in_slice(chunk, chunk_len)? };
        if out_is_ack.is_null() || out_seq.is_null() {
            return Err(KL_ERR_NULL);
        }
        let (is_ack, seq) = match ble_frame::parse_ack(c) {
            Some(s) => (1, s),
            None => (0, 0),
        };
        // SAFETY: both pointers validated non-null above.
        unsafe {
            *out_is_ack = is_ack;
            *out_seq = seq;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use areatm_kagilink_core::{ManualClock, CODE_TTL_MS};

    /// Expiry needs a controllable clock, which the C ABI deliberately does not expose, so the
    /// receiver is built here with a `ManualClock` and registered like `kl_receiver_new` does.
    #[test]
    fn receiver_failures_reports_final_count_after_expiry() {
        let clock = Arc::new(ManualClock::new());
        let mut rng = os_rng();
        let s = ReceiverSession::new(clock.clone(), &mut rng, b"rx", b"c").unwrap();
        let tok = register(ReceiverObj {
            inner: Mutex::new(Some(s)),
            id_len: 2,
            claims_len: 1,
        })
        .unwrap();
        let r: *mut kl_receiver_t = token_ptr(tok);
        let (mut used, mut max) = (0xAAu8, 0xAAu8);
        // SAFETY: valid handle and writable outputs.
        unsafe {
            assert_eq!(kl_receiver_failures(r, &mut used, &mut max), KL_OK);
            assert_eq!((used, max), (0, 3));
            clock.advance(CODE_TTL_MS + 1);
            let mut code = [0u8; 6];
            let mut n = 6usize;
            assert_eq!(
                kl_receiver_code(r, code.as_mut_ptr(), &mut n),
                KL_ERR_EXPIRED
            );
            used = 0xAA;
            max = 0xAA;
            assert_eq!(kl_receiver_failures(r, &mut used, &mut max), KL_OK);
            assert_eq!((used, max), (0, 3));
            kl_receiver_free(r);
            assert_eq!(kl_receiver_failures(r, &mut used, &mut max), KL_ERR_NULL);
        }
    }
}
