//! Handshake and code-session lifecycle (receiver-enforced limits).
//!
//! Flow: M1 receiver->sender HelloR; M2 sender->receiver HelloS+Ya; M3 receiver->sender
//! Yb+tag_r; M4 sender->receiver tag_s. The sender verifies tag_r before sending tag_s and
//! the receiver verifies tag_s before releasing a channel. The receiver alone owns the code,
//! its 60 s monotonic deadline and the three-failure budget; the sender cannot extend either.

use crate::clock::Clock;
use crate::code::Code;
use crate::error::{Error, Result};
use crate::keys::{transcript_hash, Derived, Side};
use crate::transport::{Role, SecureChannel};
use crate::wire::{
    decode_m2, decode_m3, decode_m4, encode_m2, encode_m3, encode_m4, HelloR, HelloS, MAX_CLAIMS,
    MAX_ID, NONCE_LEN, SID_LEN,
};
use pakery_cpace::{CpaceCiphersuite, CpaceInitiator, CpaceMode, CpaceResponder, InitiatorState};
use pakery_crypto::{Ristretto255Group, Sha512Hash};
use rand_core::CryptoRng;
use std::sync::Arc;

/// Code and session lifetime: 60 seconds from first display, never extended.
pub const CODE_TTL_MS: u64 = 60_000;
/// Failed attempts allowed before the code is destroyed.
pub const MAX_FAILURES: u8 = 3;

const AD_SENDER: &[u8] = b"KagiLink/sender";
const AD_RECEIVER: &[u8] = b"KagiLink/receiver";
const MODE: CpaceMode = CpaceMode::InitiatorResponder;

struct Suite;

impl CpaceCiphersuite for Suite {
    type Group = Ristretto255Group;
    type Hash = Sha512Hash;
    const DSI: &'static [u8] = b"CPaceRistretto255";
    const HASH_BLOCK_SIZE: usize = 128;
    const FIELD_SIZE_BYTES: usize = 32;
}

/// CPace channel identifier: binds both complete canonical hellos (roles, version, suite,
/// nonces, device ids, purpose, claims) into the generator.
fn channel_info(hello_r: &[u8], hello_s: &[u8]) -> Vec<u8> {
    let mut ci = Vec::with_capacity(hello_r.len() + hello_s.len() + 32);
    ci.extend_from_slice(b"kagilink/v1/ci");
    for part in [hello_r, hello_s] {
        ci.extend_from_slice(&(part.len() as u16).to_be_bytes());
        ci.extend_from_slice(part);
    }
    ci
}

enum Stage {
    AwaitM2,
    AwaitM4(Box<Derived>),
}

struct Attempt {
    hello_r: Vec<u8>,
    sid: [u8; SID_LEN],
    stage: Stage,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Active,
    Consumed,
    Dead(Error),
}

/// Receiver-side code session (owns code, deadline and failure budget).
pub struct ReceiverSession {
    clock: Arc<dyn Clock>,
    device_id: Vec<u8>,
    claims: Vec<u8>,
    code: Option<Code>,
    deadline_ms: u64,
    failures: u8,
    state: State,
    attempt: Option<Attempt>,
}

impl ReceiverSession {
    /// Create a session with a fresh code and a 60 s deadline.
    pub fn new(
        clock: Arc<dyn Clock>,
        rng: &mut impl CryptoRng,
        device_id: &[u8],
        claims: &[u8],
    ) -> Result<Self> {
        if device_id.len() > MAX_ID || claims.len() > MAX_CLAIMS {
            return Err(Error::Malformed);
        }
        let deadline_ms = clock.now_ms().saturating_add(CODE_TTL_MS);
        Ok(Self {
            clock,
            device_id: device_id.to_vec(),
            claims: claims.to_vec(),
            code: Some(Code::generate(rng)),
            deadline_ms,
            failures: 0,
            state: State::Active,
            attempt: None,
        })
    }

    fn kill(&mut self, reason: Error) {
        // An attempt that already sent M3 and is still open counts as a failure (D026). Without this,
        // dropping it here (expiry) would silently take the open failure out of the displayed count.
        if matches!(&self.attempt, Some(a) if matches!(a.stage, Stage::AwaitM4(_))) {
            self.failures = self.failures.saturating_add(1);
        }
        self.code = None; // zeroized on drop
        self.attempt = None;
        self.state = State::Dead(reason);
    }

    fn check_live(&mut self) -> Result<()> {
        match self.state {
            State::Consumed => Err(Error::NotActive),
            State::Dead(reason) => Err(reason),
            State::Active => {
                if self.clock.now_ms() >= self.deadline_ms {
                    self.kill(Error::Expired);
                    Err(Error::Expired)
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Record a failed attempt; destroys the code at the limit. Returns `reason` unchanged.
    fn fail(&mut self, reason: Error) -> Error {
        self.attempt = None;
        self.failures = self.failures.saturating_add(1);
        if self.failures >= MAX_FAILURES {
            self.kill(Error::TooManyFailures);
        }
        reason
    }

    /// Code text for display while the session is active.
    pub fn code(&self) -> Result<&str> {
        match (self.state, &self.code) {
            (State::Active, Some(code)) if self.clock.now_ms() < self.deadline_ms => {
                Ok(code.as_str())
            }
            (State::Dead(reason), _) => Err(reason),
            (State::Active, _) => Err(Error::Expired),
            _ => Err(Error::NotActive),
        }
    }

    /// Failed attempts so far.
    pub fn failures(&self) -> u8 {
        self.failures
    }

    /// Failures counted so far plus one if an attempt is open AFTER M3 was sent: such an attempt
    /// is counted when the next attempt begins (D026), so this is what the budget will read next.
    pub fn failures_including_open(&self) -> u8 {
        let open = matches!(&self.attempt, Some(a) if matches!(a.stage, Stage::AwaitM4(_)));
        self.failures.saturating_add(u8::from(open))
    }

    /// Evaluates expiry; true only while the code can still be used.
    pub fn is_active(&mut self) -> bool {
        self.check_live().is_ok()
    }

    /// Start a fresh attempt; returns message M1. An attempt still open counts as failed.
    pub fn begin_attempt(&mut self, rng: &mut impl CryptoRng) -> Result<Vec<u8>> {
        self.check_live()?;
        // An earlier attempt that already sent M3 gave a peer one offline guess: it counts as a
        // failure. One that never got that far revealed nothing and is dropped for free (D026).
        if matches!(
            self.attempt.as_ref().map(|a| &a.stage),
            Some(Stage::AwaitM4(_))
        ) {
            self.fail(Error::AttemptInProgress);
            self.check_live()?;
        }
        self.attempt = None;
        let mut sid = [0u8; SID_LEN];
        let mut nonce = [0u8; NONCE_LEN];
        rng.fill_bytes(&mut sid);
        rng.fill_bytes(&mut nonce);
        let hello = HelloR {
            sid,
            nonce,
            device_id: self.device_id.clone(),
            claims: self.claims.clone(),
        }
        .encode()?;
        self.attempt = Some(Attempt {
            hello_r: hello.clone(),
            sid,
            stage: Stage::AwaitM2,
        });
        Ok(hello)
    }

    /// Process M2; returns M3.
    pub fn handle_m2(&mut self, rng: &mut impl CryptoRng, m2: &[u8]) -> Result<Vec<u8>> {
        self.check_live()?;
        let Some(mut attempt) = self.attempt.take() else {
            return Err(Error::WrongState);
        };
        if !matches!(attempt.stage, Stage::AwaitM2) {
            // M3 was already sent for this attempt, so a second M2 is a failed attempt.
            return Err(self.fail(Error::WrongState));
        }
        match self.respond(rng, &attempt, m2) {
            Ok((m3, derived)) => {
                attempt.stage = Stage::AwaitM4(Box::new(derived));
                self.attempt = Some(attempt);
                Ok(m3)
            }
            // No M3 left this device, so no code guess could be tested: the attempt is dropped
            // without using up the failure budget (D026). A fresh begin_attempt is needed.
            Err(e) => Err(e),
        }
    }

    fn respond(
        &self,
        rng: &mut impl CryptoRng,
        attempt: &Attempt,
        m2: &[u8],
    ) -> Result<(Vec<u8>, Derived)> {
        let (hello_s_bytes, ya) = decode_m2(m2)?;
        HelloS::decode(hello_s_bytes)?;
        let code = self.code.as_ref().ok_or(Error::NotActive)?;
        let ci = channel_info(&attempt.hello_r, hello_s_bytes);
        let (yb, out) = CpaceResponder::<Suite>::respond(
            &ya,
            code.expose(),
            &ci,
            &attempt.sid,
            AD_SENDER,
            AD_RECEIVER,
            MODE,
            rng,
        )
        .map_err(|_| Error::Pake)?;
        let th = transcript_hash(&attempt.hello_r, hello_s_bytes, &ya, &yb);
        let derived = Derived::derive(out.isk.as_bytes(), th)?;
        let tag = derived.tag(Side::Receiver)?;
        Ok((encode_m3(&yb, &tag)?, derived))
    }

    /// Verify M4; on success consumes the code and returns the channel.
    pub fn handle_m4(&mut self, m4: &[u8]) -> Result<SecureChannel> {
        self.check_live()?;
        let Some(attempt) = self.attempt.take() else {
            return Err(Error::WrongState);
        };
        let Stage::AwaitM4(derived) = attempt.stage else {
            // M4 arrived before any M3 was sent: nothing was revealed, nothing is counted.
            return Err(Error::WrongState);
        };
        let verified = decode_m4(m4).and_then(|tag| derived.verify(Side::Sender, &tag));
        if let Err(e) = verified {
            return Err(self.fail(e));
        }
        let channel = SecureChannel::new(
            Role::Receiver,
            attempt.sid,
            derived.key_r2s,
            derived.key_s2r,
            self.deadline_ms,
            self.clock.clone(),
        );
        self.code = None; // consumed: zeroized on drop
        self.state = State::Consumed;
        Ok(channel)
    }
}

/// Sender-side state between sending M2 and receiving M3.
pub struct SenderPending {
    clock: Arc<dyn Clock>,
    deadline_ms: u64,
    state: InitiatorState<Suite>,
    hello_r: Vec<u8>,
    hello_s: Vec<u8>,
    ya: Vec<u8>,
    sid: [u8; SID_LEN],
}

impl SenderPending {
    /// Process M1 and build M2. The sender applies its own 60 s guard; the receiver stays
    /// authoritative for the code.
    pub fn start(
        clock: Arc<dyn Clock>,
        rng: &mut impl CryptoRng,
        code: &Code,
        m1: &[u8],
        device_id: &[u8],
        purpose: &[u8],
        claims: &[u8],
    ) -> Result<(Vec<u8>, Self)> {
        let deadline_ms = clock.now_ms().saturating_add(CODE_TTL_MS);
        let hello_r = HelloR::decode(m1)?;
        let mut nonce = [0u8; NONCE_LEN];
        rng.fill_bytes(&mut nonce);
        let hello_s = HelloS {
            nonce,
            device_id: device_id.to_vec(),
            purpose: purpose.to_vec(),
            claims: claims.to_vec(),
        }
        .encode()?;
        let ci = channel_info(m1, &hello_s);
        let (ya, state) =
            CpaceInitiator::<Suite>::start(code.expose(), &ci, &hello_r.sid, AD_SENDER, rng)
                .map_err(|_| Error::Pake)?;
        let m2 = encode_m2(&hello_s, &ya)?;
        let pending = Self {
            clock,
            deadline_ms,
            state,
            hello_r: m1.to_vec(),
            hello_s,
            ya,
            sid: hello_r.sid,
        };
        Ok((m2, pending))
    }

    /// Verify M3 (wrong code or tampering fails here), return M4 and the channel.
    pub fn handle_m3(self, m3: &[u8]) -> Result<(Vec<u8>, SecureChannel)> {
        if self.clock.now_ms() >= self.deadline_ms {
            return Err(Error::Expired);
        }
        let (yb, tag_r) = decode_m3(m3)?;
        let out = self
            .state
            .finish(&yb, AD_RECEIVER, MODE)
            .map_err(|_| Error::Pake)?;
        let th = transcript_hash(&self.hello_r, &self.hello_s, &self.ya, &yb);
        let derived = Derived::derive(out.isk.as_bytes(), th)?;
        derived.verify(Side::Receiver, &tag_r)?;
        let m4 = encode_m4(&derived.tag(Side::Sender)?);
        let channel = SecureChannel::new(
            Role::Sender,
            self.sid,
            derived.key_s2r,
            derived.key_r2s,
            self.deadline_ms,
            self.clock.clone(),
        );
        Ok((m4, channel))
    }
}
