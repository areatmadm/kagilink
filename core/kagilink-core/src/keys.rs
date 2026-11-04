//! Key schedule: CPace ISK -> HKDF-SHA-512 -> confirmation and directional AES keys.
//!
//! The six-digit code never reaches this module; only the CPace output does. All derived
//! keys are bound to the full transcript hash (both hellos and both shares).

use crate::error::{Error, Result};
use crate::wire::TAG_LEN;
use hkdf::Hkdf;
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256, Sha512};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

type HmacSha256 = Hmac<Sha256>;

const L_CONF_R: &[u8] = b"kagilink/v1/conf-key/receiver";
const L_CONF_S: &[u8] = b"kagilink/v1/conf-key/sender";
const L_KEY_S2R: &[u8] = b"kagilink/v1/aead-key/sender-to-receiver";
const L_KEY_R2S: &[u8] = b"kagilink/v1/aead-key/receiver-to-sender";
const L_TAG_R: &[u8] = b"kagilink/v1/confirm/receiver";
const L_TAG_S: &[u8] = b"kagilink/v1/confirm/sender";

/// Which side computes or verifies a confirmation tag.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Sender,
    Receiver,
}

/// Transcript hash input: lv(hello_r) || lv(hello_s) || lv(Ya) || lv(Yb).
pub(crate) fn transcript_hash(hello_r: &[u8], hello_s: &[u8], ya: &[u8], yb: &[u8]) -> [u8; 64] {
    let mut hasher = Sha512::new();
    hasher.update(b"kagilink/v1/transcript");
    for part in [hello_r, hello_s, ya, yb] {
        hasher.update((part.len() as u32).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize().into()
}

/// Derived key material; erased on drop.
#[derive(Zeroize, ZeroizeOnDrop)]
pub(crate) struct Derived {
    conf_receiver: [u8; 32],
    conf_sender: [u8; 32],
    pub(crate) key_s2r: [u8; 32],
    pub(crate) key_r2s: [u8; 32],
    th: [u8; 64],
}

fn expand(hk: &Hkdf<Sha512>, label: &[u8], th: &[u8; 64]) -> Result<[u8; 32]> {
    let mut info = Zeroizing::new(Vec::with_capacity(label.len() + th.len() + 4));
    info.extend_from_slice(&(label.len() as u16).to_be_bytes());
    info.extend_from_slice(label);
    info.extend_from_slice(&(th.len() as u16).to_be_bytes());
    info.extend_from_slice(th);
    let mut okm = [0u8; 32];
    hk.expand(&info, &mut okm)
        .map_err(|_| Error::Authentication)?;
    Ok(okm)
}

impl Derived {
    /// `isk` is the CPace intermediate session key (64 bytes for SHA-512).
    pub(crate) fn derive(isk: &[u8], th: [u8; 64]) -> Result<Self> {
        let hk = Hkdf::<Sha512>::new(Some(&th), isk);
        Ok(Self {
            conf_receiver: expand(&hk, L_CONF_R, &th)?,
            conf_sender: expand(&hk, L_CONF_S, &th)?,
            key_s2r: expand(&hk, L_KEY_S2R, &th)?,
            key_r2s: expand(&hk, L_KEY_R2S, &th)?,
            th,
        })
    }

    pub(crate) fn tag(&self, side: Side) -> Result<[u8; TAG_LEN]> {
        let (key, label) = match side {
            Side::Receiver => (&self.conf_receiver, L_TAG_R),
            Side::Sender => (&self.conf_sender, L_TAG_S),
        };
        let mut mac =
            <HmacSha256 as KeyInit>::new_from_slice(key).map_err(|_| Error::Authentication)?;
        mac.update(&(label.len() as u16).to_be_bytes());
        mac.update(label);
        mac.update(&self.th);
        Ok(mac.finalize().into_bytes().into())
    }

    /// Constant-time verification of the peer's confirmation tag.
    pub(crate) fn verify(&self, side: Side, tag: &[u8; TAG_LEN]) -> Result<()> {
        let expected = self.tag(side)?;
        if bool::from(expected.ct_eq(tag)) {
            Ok(())
        } else {
            Err(Error::ConfirmationFailed)
        }
    }
}
