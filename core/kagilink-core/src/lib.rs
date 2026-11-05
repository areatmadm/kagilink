//! KagiLink Phase 1 core (decisions D012-D014).
//!
//! CPace (Ristretto255) handshake with explicit mutual key confirmation, HKDF key schedule,
//! and AES-256-GCM framed transport. Policy: synthetic data only until an independent
//! review of the exact locked dependencies exists; this crate is not a release component.
//! The six-digit code is a PAKE password, never a key. Errors never include secrets.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod ble_frame;
pub mod clock;
pub mod code;
pub mod error;
pub mod handshake;
mod keys;
pub mod transport;
pub mod wire;

pub use clock::{Clock, ManualClock, SystemClock};
pub use code::Code;
pub use error::{Error, Result};
pub use handshake::{ReceiverSession, SenderPending, CODE_TTL_MS, MAX_FAILURES};
pub use transport::{kind, Message, Role, SecureChannel};

/// Operating-system CSPRNG adapter (panics only if the OS cannot supply randomness).
pub fn os_rng() -> impl rand_core::CryptoRng {
    rand_core::UnwrapErr(getrandom::SysRng)
}
