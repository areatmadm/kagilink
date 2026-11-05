//! Authenticated, fragmented transport on top of the handshake keys.
//!
//! One AES-256-GCM key per direction, deterministic counter nonces (never random),
//! strict in-order single-message reassembly and fail-closed lifecycle. Errors never
//! carry key or plaintext bytes. This is unreviewed prototype code (Phase 1).

use crate::clock::Clock;
use crate::error::{Error, Result};
use aes_gcm::{AeadInOut, Aes256Gcm, KeyInit};
use std::sync::Arc;
use zeroize::{Zeroize, Zeroizing};

/// Maximum plaintext message size in bytes.
pub const MAX_MESSAGE: usize = 4096;
/// Maximum fragment payload size in bytes.
pub const MAX_FRAGMENT_PAYLOAD: usize = 256;
/// Maximum number of fragments per message.
pub const MAX_FRAGMENTS: usize = 256;
/// Terminal per-direction message budget (counter exhaustion limit).
pub const MAX_MESSAGES_PER_DIRECTION: u64 = 1 << 20;

/// Message kinds carried (authenticated) in every envelope.
pub mod kind {
    /// Control message.
    pub const CONTROL: u8 = 1;
    /// Secret payload; requires one-time approval to send and receiver readiness to accept.
    pub const SECRET: u8 = 2;
    /// Result report.
    pub const RESULT: u8 = 3;
}

/// Envelope format version bound into the AAD.
const VERSION: u8 = 1;
/// AES-GCM tag length.
const TAG_LEN: usize = 16;
/// Fragment header: seq u64 | kind u8 | idx u16 | count u16.
const HEADER_LEN: usize = 13;
/// Largest ciphertext||tag accepted.
const MAX_CIPHERTEXT: usize = MAX_MESSAGE + TAG_LEN;
const DIR_SENDER_TO_RECEIVER: u8 = 1;
const DIR_RECEIVER_TO_SENDER: u8 = 2;

/// Handshake role.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// Mobile/sender side (CPace initiator "A").
    Sender,
    /// Desktop/receiver side (CPace responder "B").
    Receiver,
}

/// A fully reassembled, authenticated message.
pub struct Message {
    /// Message kind.
    pub kind: u8,
    /// Decrypted payload; erased on drop.
    pub plaintext: Zeroizing<Vec<u8>>,
}

struct Pending {
    seq: u64,
    kind: u8,
    count: u16,
    next_idx: u16,
    buf: Zeroizing<Vec<u8>>,
}

/// Authenticated encrypted channel produced by a completed handshake.
pub struct SecureChannel {
    role: Role,
    session_id: [u8; 16],
    key_send: Zeroizing<[u8; 32]>,
    key_recv: Zeroizing<[u8; 32]>,
    deadline_ms: u64,
    clock: Arc<dyn Clock>,
    send_seq: u64,
    recv_seq: u64,
    approved: bool,
    ready: bool,
    pending: Option<Pending>,
    closed: bool,
}

fn valid_kind(k: u8) -> bool {
    matches!(k, kind::CONTROL | kind::SECRET | kind::RESULT)
}

fn nonce_for(seq: u64) -> aes_gcm::Nonce<<Aes256Gcm as aes_gcm::AeadCore>::NonceSize> {
    let mut n = [0u8; 12];
    n[4..].copy_from_slice(&seq.to_be_bytes());
    n.into()
}

impl SecureChannel {
    /// Internal constructor used by the handshake.
    pub(crate) fn new(
        role: Role,
        session_id: [u8; 16],
        key_send: [u8; 32],
        key_recv: [u8; 32],
        deadline_ms: u64,
        clock: Arc<dyn Clock>,
    ) -> Self {
        // The caller's by-value copies are the caller's to erase; ours are Zeroizing.
        Self {
            role,
            session_id,
            key_send: Zeroizing::new(key_send),
            key_recv: Zeroizing::new(key_recv),
            deadline_ms,
            clock,
            send_seq: 0,
            recv_seq: 0,
            approved: false,
            ready: false,
            pending: None,
            closed: false,
        }
    }

    /// This side's role.
    pub fn role(&self) -> Role {
        self.role
    }

    /// Sender: grant exactly one SECRET transfer (after explicit local user approval).
    pub fn approve_one_transfer(&mut self) {
        if !self.closed {
            self.approved = true;
        }
    }

    /// Receiver: declare that the target is ready to receive a SECRET message.
    pub fn set_ready_to_receive(&mut self, ready: bool) {
        if !self.closed {
            self.ready = ready;
        }
    }

    fn send_direction(&self) -> u8 {
        match self.role {
            Role::Sender => DIR_SENDER_TO_RECEIVER,
            Role::Receiver => DIR_RECEIVER_TO_SENDER,
        }
    }

    fn recv_direction(&self) -> u8 {
        match self.role {
            Role::Sender => DIR_RECEIVER_TO_SENDER,
            Role::Receiver => DIR_SENDER_TO_RECEIVER,
        }
    }

    fn aad(&self, dir: u8, kind: u8, seq: u64, ct_len: usize) -> Result<[u8; 31]> {
        let ct_len = u32::try_from(ct_len).map_err(|_| Error::Limits)?;
        let mut a = [0u8; 31];
        a[0] = VERSION;
        a[1] = dir;
        a[2] = kind;
        a[3..19].copy_from_slice(&self.session_id);
        a[19..27].copy_from_slice(&seq.to_be_bytes());
        a[27..31].copy_from_slice(&ct_len.to_be_bytes());
        Ok(a)
    }

    fn check_deadline(&mut self) -> Result<()> {
        if self.clock.now_ms() >= self.deadline_ms {
            self.close();
            return Err(Error::Expired);
        }
        Ok(())
    }

    /// Encrypt and fragment one message.
    ///
    /// `Limits` and `NotApproved` are returned without closing the channel or consuming
    /// a counter; every other error closes it.
    pub fn seal(
        &mut self,
        kind: u8,
        plaintext: &[u8],
        max_fragment_payload: usize,
    ) -> Result<Vec<Vec<u8>>> {
        if self.closed {
            return Err(Error::Closed);
        }
        self.check_deadline()?;
        if self.send_seq >= MAX_MESSAGES_PER_DIRECTION {
            self.close();
            return Err(Error::CounterExhausted);
        }
        if !valid_kind(kind) || plaintext.is_empty() || plaintext.len() > MAX_MESSAGE {
            return Err(Error::Limits);
        }
        let payload_max = max_fragment_payload.clamp(1, MAX_FRAGMENT_PAYLOAD);
        let ct_len = plaintext.len() + TAG_LEN;
        let count = ct_len.div_ceil(payload_max);
        if count > MAX_FRAGMENTS {
            return Err(Error::Limits);
        }
        if kind == kind::SECRET && !self.approved {
            return Err(Error::NotApproved);
        }

        let seq = self.send_seq;
        let aad = self.aad(self.send_direction(), kind, seq, ct_len)?;
        let mut buf = Zeroizing::new(Vec::with_capacity(ct_len));
        buf.extend_from_slice(plaintext);
        let cipher = Aes256Gcm::new(&(*self.key_send).into());
        if cipher
            .encrypt_in_place(&nonce_for(seq), &aad, &mut *buf)
            .is_err()
        {
            self.close();
            return Err(Error::Authentication);
        }
        debug_assert_eq!(buf.len(), ct_len);
        self.send_seq += 1;
        if kind == kind::SECRET {
            self.approved = false;
        }

        let mut out = Vec::with_capacity(count);
        for (i, chunk) in buf.chunks(payload_max).enumerate() {
            let mut f = Vec::with_capacity(HEADER_LEN + chunk.len());
            f.extend_from_slice(&seq.to_be_bytes());
            f.push(kind);
            f.extend_from_slice(&(i as u16).to_be_bytes());
            f.extend_from_slice(&(count as u16).to_be_bytes());
            f.extend_from_slice(chunk);
            out.push(f);
        }
        Ok(out)
    }

    /// Feed one received fragment; returns the message once complete and authenticated.
    ///
    /// Any error closes the channel and erases keys and partial data.
    pub fn open_fragment(&mut self, fragment: &[u8]) -> Result<Option<Message>> {
        if self.closed {
            return Err(Error::Closed);
        }
        self.check_deadline()?;
        match self.open_inner(fragment) {
            Ok(v) => Ok(v),
            Err(e) => {
                self.close();
                Err(e)
            }
        }
    }

    fn open_inner(&mut self, f: &[u8]) -> Result<Option<Message>> {
        if f.len() <= HEADER_LEN {
            return Err(Error::Framing);
        }
        let seq = u64::from_be_bytes(f[0..8].try_into().map_err(|_| Error::Framing)?);
        let k = f[8];
        let idx = u16::from_be_bytes([f[9], f[10]]);
        let count = u16::from_be_bytes([f[11], f[12]]);
        let payload = &f[HEADER_LEN..];
        if payload.len() > MAX_FRAGMENT_PAYLOAD || !valid_kind(k) {
            return Err(Error::Framing);
        }

        match &self.pending {
            None => {
                if self.recv_seq >= MAX_MESSAGES_PER_DIRECTION {
                    return Err(Error::CounterExhausted);
                }
                if idx != 0 || seq != self.recv_seq || count == 0 || count as usize > MAX_FRAGMENTS
                {
                    return Err(Error::Framing);
                }
                if k == kind::SECRET && !self.ready {
                    return Err(Error::NotApproved);
                }
                self.pending = Some(Pending {
                    seq,
                    kind: k,
                    count,
                    next_idx: 0,
                    buf: Zeroizing::new(Vec::with_capacity(
                        MAX_CIPHERTEXT.min(payload.len().saturating_mul(count as usize)),
                    )),
                });
            }
            Some(p) => {
                if p.seq != seq || p.kind != k || p.count != count || p.next_idx != idx {
                    return Err(Error::Framing);
                }
            }
        }

        let p = self.pending.as_mut().ok_or(Error::Framing)?;
        if p.buf.len() + payload.len() > MAX_CIPHERTEXT {
            return Err(Error::Limits);
        }
        p.buf.extend_from_slice(payload);
        p.next_idx += 1;
        if p.next_idx < p.count {
            return Ok(None);
        }

        // Final fragment: decrypt and verify.
        let mut p = self.pending.take().ok_or(Error::Framing)?;
        if p.buf.len() <= TAG_LEN {
            return Err(Error::Framing);
        }
        let aad = self.aad(self.recv_direction(), p.kind, p.seq, p.buf.len())?;
        let cipher = Aes256Gcm::new(&(*self.key_recv).into());
        if cipher
            .decrypt_in_place(&nonce_for(p.seq), &aad, &mut *p.buf)
            .is_err()
        {
            // p.buf is zeroized on drop; no plaintext is released.
            return Err(Error::Authentication);
        }
        self.recv_seq += 1;
        Ok(Some(Message {
            kind: p.kind,
            plaintext: std::mem::take(&mut p.buf),
        }))
    }

    /// Close the channel and erase keys.
    pub fn close(&mut self) {
        self.key_send.zeroize();
        self.key_recv.zeroize();
        self.pending = None;
        self.approved = false;
        self.ready = false;
        self.closed = true;
    }

    /// True once the channel is closed (error, expiry, close()).
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    #[cfg(test)]
    fn set_counters(&mut self, send: u64, recv: u64) {
        self.send_seq = send;
        self.recv_seq = recv;
    }

    #[cfg(test)]
    fn keys_are_zero(&self) -> bool {
        self.key_send.iter().all(|b| *b == 0) && self.key_recv.iter().all(|b| *b == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::ManualClock;

    const DEADLINE: u64 = 60_000;

    fn pair() -> (SecureChannel, SecureChannel, Arc<ManualClock>) {
        let clock = Arc::new(ManualClock::new());
        let sid = [7u8; 16];
        let (k1, k2) = ([0x11u8; 32], [0x22u8; 32]);
        let a = SecureChannel::new(Role::Sender, sid, k1, k2, DEADLINE, clock.clone());
        let b = SecureChannel::new(Role::Receiver, sid, k2, k1, DEADLINE, clock.clone());
        (a, b, clock)
    }

    fn feed(rx: &mut SecureChannel, frags: &[Vec<u8>]) -> Result<Option<Message>> {
        let mut last = None;
        for f in frags {
            last = rx.open_fragment(f)?;
        }
        Ok(last)
    }

    fn data(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i * 31 + 5) as u8).collect()
    }

    #[test]
    fn roundtrip_small_and_max() {
        let (mut a, mut b, _) = pair();
        for n in [1usize, 5, 4096] {
            let pt = data(n);
            let fr = a.seal(kind::CONTROL, &pt, 256).unwrap();
            let m = feed(&mut b, &fr).unwrap().unwrap();
            assert_eq!(m.kind, kind::CONTROL);
            assert_eq!(&m.plaintext[..], &pt[..]);
        }
    }

    #[test]
    fn roundtrip_fragment_sizes() {
        for (size, len) in [
            (1usize, 200usize),
            (20, 1000),
            (256, 4096),
            (9999, 300),
            (0, 100),
        ] {
            let (mut a, mut b, _) = pair();
            let pt = data(len);
            let fr = a.seal(kind::RESULT, &pt, size).unwrap();
            let eff = size.clamp(1, 256);
            assert!(fr.iter().all(|f| f.len() - HEADER_LEN <= eff));
            let m = feed(&mut b, &fr).unwrap().unwrap();
            assert_eq!(&m.plaintext[..], &pt[..]);
        }
    }

    #[test]
    fn fragment_size_one_with_large_message_hits_limits() {
        let (mut a, _, _) = pair();
        assert_eq!(
            a.seal(kind::CONTROL, &data(4096), 1).err(),
            Some(Error::Limits)
        );
        assert!(!a.is_closed());
        assert_eq!(a.send_seq, 0);
    }

    #[test]
    fn ciphertext_tamper_any_byte_fails_and_closes() {
        let (mut a0, _, _) = pair();
        let fr0 = a0.seal(kind::CONTROL, &data(50), 20).unwrap();
        for fi in 0..fr0.len() {
            for bi in HEADER_LEN..fr0[fi].len() {
                let (mut a, mut b, _) = pair();
                let mut fr = a.seal(kind::CONTROL, &data(50), 20).unwrap();
                fr[fi][bi] ^= 0x01;
                assert_eq!(feed(&mut b, &fr).err(), Some(Error::Authentication));
                assert!(b.is_closed());
                assert!(b.keys_are_zero());
                assert_eq!(b.open_fragment(&fr[0]).err(), Some(Error::Closed));
            }
        }
    }

    #[test]
    fn header_tamper_fails_and_closes() {
        let (mut a0, _, _) = pair();
        let n = a0.seal(kind::CONTROL, &data(50), 20).unwrap().len();
        for fi in 0..n {
            for off in 0..HEADER_LEN {
                let (mut a, mut b, _) = pair();
                let mut fr = a.seal(kind::CONTROL, &data(50), 20).unwrap();
                fr[fi][off] ^= 0x01;
                let e = feed(&mut b, &fr).err();
                assert!(
                    matches!(e, Some(Error::Framing) | Some(Error::Authentication)),
                    "fi={fi} off={off} got {e:?}"
                );
                assert!(b.is_closed());
            }
        }
    }

    #[test]
    fn kind_swap_is_rejected() {
        // Rewrite all headers consistently to another valid kind: the AAD must catch it.
        let (mut a, mut b, _) = pair();
        let mut fr = a.seal(kind::CONTROL, &data(40), 20).unwrap();
        for f in &mut fr {
            f[8] = kind::RESULT;
        }
        assert_eq!(feed(&mut b, &fr).err(), Some(Error::Authentication));
    }

    #[test]
    fn truncated_and_trailing_fragments() {
        let (mut a, mut b, _) = pair();
        let fr = a.seal(kind::CONTROL, &data(10), 256).unwrap();
        assert_eq!(
            b.open_fragment(&fr[0][..HEADER_LEN]).err(),
            Some(Error::Framing)
        );
        assert!(b.is_closed());
        let (mut a, mut b, _) = pair();
        let mut fr = a.seal(kind::CONTROL, &data(10), 256).unwrap();
        fr[0].push(0); // trailing garbage changes the ciphertext length
        assert!(b.open_fragment(&fr[0]).is_err());
        assert!(b.is_closed());
        let (_, mut b, _) = pair();
        let mut big = vec![0u8; HEADER_LEN + MAX_FRAGMENT_PAYLOAD + 1];
        big[8] = kind::CONTROL;
        big[12] = 1;
        assert_eq!(b.open_fragment(&big).err(), Some(Error::Framing));
    }

    #[test]
    fn duplicate_fragment() {
        let (mut a, mut b, _) = pair();
        let fr = a.seal(kind::CONTROL, &data(100), 20).unwrap();
        assert!(b.open_fragment(&fr[0]).unwrap().is_none());
        assert_eq!(b.open_fragment(&fr[0]).err(), Some(Error::Framing));
        assert!(b.is_closed());
    }

    #[test]
    fn missing_fragment_then_new_message() {
        let (mut a, mut b, _) = pair();
        let fr = a.seal(kind::CONTROL, &data(100), 20).unwrap();
        let next = a.seal(kind::CONTROL, &data(10), 20).unwrap();
        assert!(b.open_fragment(&fr[0]).unwrap().is_none());
        assert_eq!(b.open_fragment(&fr[2]).err(), Some(Error::Framing));
        assert!(b.is_closed());
        assert_eq!(b.open_fragment(&next[0]).err(), Some(Error::Closed));

        // New message arriving mid-message.
        let (mut a, mut b, _) = pair();
        let fr = a.seal(kind::CONTROL, &data(100), 20).unwrap();
        let next = a.seal(kind::CONTROL, &data(10), 20).unwrap();
        assert!(b.open_fragment(&fr[0]).unwrap().is_none());
        assert_eq!(b.open_fragment(&next[0]).err(), Some(Error::Framing));
    }

    #[test]
    fn missing_last_fragment_never_releases() {
        let (mut a, mut b, _) = pair();
        let fr = a.seal(kind::CONTROL, &data(100), 20).unwrap();
        for f in &fr[..fr.len() - 1] {
            assert!(b.open_fragment(f).unwrap().is_none());
        }
        assert!(!b.is_closed());
    }

    #[test]
    fn reordered_fragments() {
        let (mut a, mut b, _) = pair();
        let fr = a.seal(kind::CONTROL, &data(100), 20).unwrap();
        assert!(b.open_fragment(&fr[0]).unwrap().is_none());
        assert_eq!(b.open_fragment(&fr[2]).err(), Some(Error::Framing));
        let (mut a, mut b, _) = pair();
        let fr = a.seal(kind::CONTROL, &data(100), 20).unwrap();
        assert_eq!(b.open_fragment(&fr[1]).err(), Some(Error::Framing));
        assert!(b.is_closed());
    }

    #[test]
    fn replay_of_accepted_message() {
        let (mut a, mut b, _) = pair();
        let fr = a.seal(kind::CONTROL, &data(30), 20).unwrap();
        assert!(feed(&mut b, &fr).unwrap().is_some());
        assert_eq!(b.open_fragment(&fr[0]).err(), Some(Error::Framing));
        assert!(b.is_closed());
    }

    #[test]
    fn gap_in_sequence_rejected() {
        let (mut a, mut b, _) = pair();
        let _skipped = a.seal(kind::CONTROL, &data(30), 256).unwrap();
        let second = a.seal(kind::CONTROL, &data(30), 256).unwrap();
        assert_eq!(b.open_fragment(&second[0]).err(), Some(Error::Framing));
    }

    #[test]
    fn wrong_direction_replay() {
        let (mut a, mut b, _) = pair();
        let fr = a.seal(kind::CONTROL, &data(30), 256).unwrap();
        feed(&mut b, &fr).unwrap().unwrap();
        // Reflect the sender's own fragment back to the sender.
        assert!(a.open_fragment(&fr[0]).is_err());
        assert!(a.is_closed());

        // Same key in both directions: only the direction byte separates them.
        let clock = Arc::new(ManualClock::new());
        let k = [0x33u8; 32];
        let mut a = SecureChannel::new(Role::Sender, [1; 16], k, k, DEADLINE, clock.clone());
        let mut b = SecureChannel::new(Role::Receiver, [1; 16], k, k, DEADLINE, clock);
        let fr = a.seal(kind::CONTROL, &data(30), 256).unwrap();
        feed(&mut b, &fr).unwrap().unwrap();
        // Receiver-to-sender seq 0 is expected by `a`; reflect a's own message.
        assert_eq!(a.open_fragment(&fr[0]).err(), Some(Error::Authentication));
    }

    #[test]
    fn session_id_binding() {
        let clock = Arc::new(ManualClock::new());
        let (k1, k2) = ([1u8; 32], [2u8; 32]);
        let mut a = SecureChannel::new(Role::Sender, [1; 16], k1, k2, DEADLINE, clock.clone());
        let mut b = SecureChannel::new(Role::Receiver, [2; 16], k2, k1, DEADLINE, clock);
        let fr = a.seal(kind::CONTROL, &data(30), 256).unwrap();
        assert_eq!(feed(&mut b, &fr).err(), Some(Error::Authentication));
    }

    #[test]
    fn counter_exhaustion_send() {
        let (mut a, _, _) = pair();
        a.set_counters(MAX_MESSAGES_PER_DIRECTION - 1, 0);
        assert!(a.seal(kind::CONTROL, b"x", 256).is_ok());
        assert_eq!(
            a.seal(kind::CONTROL, b"x", 256).err(),
            Some(Error::CounterExhausted)
        );
        assert!(a.is_closed());
        assert_eq!(a.seal(kind::CONTROL, b"x", 256).err(), Some(Error::Closed));
    }

    #[test]
    fn counter_exhaustion_receive() {
        let (mut a, mut b, _) = pair();
        a.set_counters(MAX_MESSAGES_PER_DIRECTION - 1, 0);
        b.set_counters(0, MAX_MESSAGES_PER_DIRECTION - 1);
        let fr = a.seal(kind::CONTROL, b"x", 256).unwrap();
        assert!(feed(&mut b, &fr).unwrap().is_some());
        assert_eq!(b.recv_seq, MAX_MESSAGES_PER_DIRECTION);
        assert_eq!(b.open_fragment(&fr[0]).err(), Some(Error::CounterExhausted));
        assert!(b.is_closed());
    }

    #[test]
    fn expiry_before_seal_and_mid_message() {
        let (mut a, mut b, clock) = pair();
        let fr = a.seal(kind::CONTROL, &data(100), 20).unwrap();
        assert!(b.open_fragment(&fr[0]).unwrap().is_none());
        clock.advance(DEADLINE - 1);
        assert!(b.open_fragment(&fr[1]).unwrap().is_none());
        clock.advance(1);
        assert_eq!(b.open_fragment(&fr[2]).err(), Some(Error::Expired));
        assert!(b.is_closed());
        assert!(b.keys_are_zero());
        assert_eq!(b.open_fragment(&fr[3]).err(), Some(Error::Closed));
        assert_eq!(a.seal(kind::CONTROL, b"x", 20).err(), Some(Error::Expired));
        assert!(a.is_closed());
        assert_eq!(a.seal(kind::CONTROL, b"x", 20).err(), Some(Error::Closed));
    }

    #[test]
    fn secret_requires_approval_and_is_consumed() {
        let (mut a, mut b, _) = pair();
        b.set_ready_to_receive(true);
        assert_eq!(
            a.seal(kind::SECRET, b"synthetic", 256).err(),
            Some(Error::NotApproved)
        );
        assert!(!a.is_closed());
        assert_eq!(a.send_seq, 0);
        a.approve_one_transfer();
        let fr = a.seal(kind::SECRET, b"synthetic", 256).unwrap();
        assert_eq!(
            a.seal(kind::SECRET, b"synthetic", 256).err(),
            Some(Error::NotApproved)
        );
        // Other kinds need no approval.
        assert!(a.seal(kind::CONTROL, b"c", 256).is_ok());
        let m = feed(&mut b, &fr).unwrap().unwrap();
        assert_eq!(m.kind, kind::SECRET);
        assert_eq!(&m.plaintext[..], b"synthetic");
    }

    #[test]
    fn failed_seal_does_not_consume_approval() {
        let (mut a, _, _) = pair();
        a.approve_one_transfer();
        assert_eq!(a.seal(kind::SECRET, b"", 256).err(), Some(Error::Limits));
        assert!(a.seal(kind::SECRET, b"ok", 256).is_ok());
    }

    #[test]
    fn approval_cleared_on_close() {
        let (mut a, _, _) = pair();
        a.approve_one_transfer();
        a.close();
        assert!(!a.approved);
        assert_eq!(a.seal(kind::SECRET, b"x", 256).err(), Some(Error::Closed));
    }

    #[test]
    fn receiver_not_ready_rejects_secret() {
        let (mut a, mut b, _) = pair();
        a.approve_one_transfer();
        let fr = a.seal(kind::SECRET, b"synthetic", 256).unwrap();
        assert_eq!(b.open_fragment(&fr[0]).err(), Some(Error::NotApproved));
        assert!(b.is_closed());

        // Ready toggled off again before arrival.
        let (mut a, mut b, _) = pair();
        a.approve_one_transfer();
        b.set_ready_to_receive(true);
        b.set_ready_to_receive(false);
        let fr = a.seal(kind::SECRET, b"synthetic", 256).unwrap();
        assert_eq!(b.open_fragment(&fr[0]).err(), Some(Error::NotApproved));
    }

    #[test]
    fn limits_empty_oversize_invalid_kind() {
        let (mut a, _, _) = pair();
        assert_eq!(a.seal(kind::CONTROL, b"", 256).err(), Some(Error::Limits));
        assert_eq!(
            a.seal(kind::CONTROL, &data(MAX_MESSAGE + 1), 256).err(),
            Some(Error::Limits)
        );
        assert_eq!(a.seal(0, b"x", 256).err(), Some(Error::Limits));
        assert_eq!(a.seal(4, b"x", 256).err(), Some(Error::Limits));
        assert!(!a.is_closed());
        assert_eq!(a.send_seq, 0);
        assert!(a.seal(kind::CONTROL, b"x", 256).is_ok());
    }

    #[test]
    fn invalid_fragment_header_fields() {
        let (mut a, _, _) = pair();
        let fr = a.seal(kind::CONTROL, &data(10), 256).unwrap();
        for (hi, lo) in [(0u8, 0u8), (0x01, 0x01)] {
            // count = 0 and count = 257
            let (_, mut b, _) = pair();
            let mut f = fr[0].clone();
            f[11] = hi;
            f[12] = lo;
            assert_eq!(b.open_fragment(&f).err(), Some(Error::Framing));
        }
        let (_, mut b, _) = pair();
        let mut f = fr[0].clone();
        f[8] = 9; // invalid kind
        assert_eq!(b.open_fragment(&f).err(), Some(Error::Framing));
    }

    #[test]
    fn oversized_reassembly_rejected_early() {
        let (_, mut b, _) = pair();
        let mut frags = Vec::new();
        for i in 0..=(MAX_CIPHERTEXT / MAX_FRAGMENT_PAYLOAD) + 1 {
            let mut f = vec![0u8; HEADER_LEN + MAX_FRAGMENT_PAYLOAD];
            f[8] = kind::CONTROL;
            f[9..11].copy_from_slice(&(i as u16).to_be_bytes());
            f[11..13].copy_from_slice(&(MAX_FRAGMENTS as u16).to_be_bytes());
            frags.push(f);
        }
        let r = feed(&mut b, &frags);
        assert_eq!(r.err(), Some(Error::Limits));
        assert!(b.is_closed());
    }

    #[test]
    fn too_short_ciphertext_rejected() {
        let (_, mut b, _) = pair();
        let mut f = vec![0u8; HEADER_LEN + TAG_LEN];
        f[8] = kind::CONTROL;
        f[12] = 1;
        assert_eq!(b.open_fragment(&f).err(), Some(Error::Framing));
    }

    #[test]
    fn thousand_messages_both_directions() {
        let (mut a, mut b, _) = pair();
        for i in 0..1000usize {
            let pt = data(1 + i % 300);
            let fr = a.seal(kind::CONTROL, &pt, 20 + i % 237).unwrap();
            let m = feed(&mut b, &fr).unwrap().unwrap();
            assert_eq!(&m.plaintext[..], &pt[..]);
            let pt2 = data(1 + (i * 7) % 500);
            let fr = b.seal(kind::RESULT, &pt2, 64).unwrap();
            let m = feed(&mut a, &fr).unwrap().unwrap();
            assert_eq!(&m.plaintext[..], &pt2[..]);
        }
        assert_eq!(
            (a.send_seq, a.recv_seq, b.send_seq, b.recv_seq),
            (1000, 1000, 1000, 1000)
        );
    }

    #[test]
    fn nonces_unique_per_message() {
        let (mut a, _, _) = pair();
        let f1 = a.seal(kind::CONTROL, b"same", 256).unwrap();
        let f2 = a.seal(kind::CONTROL, b"same", 256).unwrap();
        assert_ne!(f1[0][HEADER_LEN..], f2[0][HEADER_LEN..]);
    }

    #[test]
    fn close_erases_keys_and_blocks_everything() {
        let (mut a, mut b, _) = pair();
        let fr = a.seal(kind::CONTROL, &data(100), 20).unwrap();
        assert!(b.open_fragment(&fr[0]).unwrap().is_none());
        b.close();
        assert!(b.is_closed());
        assert!(b.keys_are_zero());
        assert!(b.pending.is_none());
        assert_eq!(b.open_fragment(&fr[1]).err(), Some(Error::Closed));
        assert_eq!(b.seal(kind::CONTROL, b"x", 20).err(), Some(Error::Closed));
        a.close();
        assert!(a.keys_are_zero());
        assert_eq!(a.seal(kind::CONTROL, b"x", 20).err(), Some(Error::Closed));
        assert_eq!(a.role(), Role::Sender);
        assert_eq!(b.role(), Role::Receiver);
    }
}
