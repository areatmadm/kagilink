//! Canonical, length-delimited, strictly bounded handshake encodings.
//!
//! All integers are big endian. Parsers reject truncation, trailing bytes, unknown
//! versions/suites and over-long fields. Nothing here is a secret.

use crate::error::{Error, Result};

/// Protocol version carried in every hello.
pub const VERSION: u8 = 1;
/// Exact suite identifier. A peer proposing anything else is rejected (no downgrade).
pub const SUITE: &[u8] = b"KagiLink-P1/CPACE-RISTR255-SHA512/HKDF-SHA512/HMAC-SHA256/AES-256-GCM";
/// Length of the receiver-chosen session identifier.
pub const SID_LEN: usize = 16;
/// Length of each side's fresh nonce.
pub const NONCE_LEN: usize = 32;
/// CPace share length for the Ristretto255 suite.
pub const SHARE_LEN: usize = 32;
/// Confirmation tag length (HMAC-SHA-256).
pub const TAG_LEN: usize = 32;
/// Maximum device identifier length.
pub const MAX_ID: usize = 32;
/// Maximum purpose length.
pub const MAX_PURPOSE: usize = 32;
/// Maximum security-claims length. Claims are unverified assertions of the peer.
pub const MAX_CLAIMS: usize = 64;

const MAGIC: &[u8; 2] = b"KL";
const T_HELLO_R: u8 = 1;
const T_HELLO_S: u8 = 2;
pub(crate) const T_M2: u8 = 0x12;
pub(crate) const T_M3: u8 = 0x13;
pub(crate) const T_M4: u8 = 0x14;

pub(crate) fn put_lv(out: &mut Vec<u8>, data: &[u8], max: usize) -> Result<()> {
    if data.len() > max {
        return Err(Error::Malformed);
    }
    out.extend_from_slice(&(data.len() as u16).to_be_bytes());
    out.extend_from_slice(data);
    Ok(())
}

pub(crate) struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }
    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.0.len() < n {
            return Err(Error::Malformed);
        }
        let (head, tail) = self.0.split_at(n);
        self.0 = tail;
        Ok(head)
    }
    pub(crate) fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?.try_into().map_err(|_| Error::Malformed)
    }
    pub(crate) fn lv(&mut self, max: usize) -> Result<&'a [u8]> {
        let len = u16::from_be_bytes(self.array::<2>()?) as usize;
        if len > max {
            return Err(Error::Malformed);
        }
        self.take(len)
    }
    pub(crate) fn finish(self) -> Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(Error::Malformed)
        }
    }
}

fn header(out: &mut Vec<u8>, kind: u8) -> Result<()> {
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(kind);
    put_lv(out, SUITE, SUITE.len())
}

fn check_header(r: &mut Reader<'_>, kind: u8) -> Result<()> {
    if &r.array::<2>()? != MAGIC {
        return Err(Error::Malformed);
    }
    if r.byte()? != VERSION {
        return Err(Error::Unsupported);
    }
    if r.byte()? != kind {
        return Err(Error::WrongState);
    }
    if r.lv(SUITE.len())? != SUITE {
        return Err(Error::Unsupported);
    }
    Ok(())
}

/// Receiver hello (message M1).
#[derive(Clone, PartialEq, Eq)]
pub struct HelloR {
    /// Fresh random session identifier chosen by the receiver.
    pub sid: [u8; SID_LEN],
    /// Fresh receiver nonce.
    pub nonce: [u8; NONCE_LEN],
    /// Transient receiver device identifier (a claim, not verified identity).
    pub device_id: Vec<u8>,
    /// Receiver security claims (unverified).
    pub claims: Vec<u8>,
}

impl HelloR {
    /// Canonical encoding.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        header(&mut out, T_HELLO_R)?;
        out.extend_from_slice(&self.sid);
        out.extend_from_slice(&self.nonce);
        put_lv(&mut out, &self.device_id, MAX_ID)?;
        put_lv(&mut out, &self.claims, MAX_CLAIMS)?;
        Ok(out)
    }
    /// Strict decoding.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        check_header(&mut r, T_HELLO_R)?;
        let sid = r.array::<SID_LEN>()?;
        let nonce = r.array::<NONCE_LEN>()?;
        let device_id = r.lv(MAX_ID)?.to_vec();
        let claims = r.lv(MAX_CLAIMS)?.to_vec();
        r.finish()?;
        Ok(Self {
            sid,
            nonce,
            device_id,
            claims,
        })
    }
}

/// Sender hello (carried inside message M2).
#[derive(Clone, PartialEq, Eq)]
pub struct HelloS {
    /// Fresh sender nonce.
    pub nonce: [u8; NONCE_LEN],
    /// Transient sender device identifier (a claim).
    pub device_id: Vec<u8>,
    /// Requested transfer purpose.
    pub purpose: Vec<u8>,
    /// Sender security claims (unverified).
    pub claims: Vec<u8>,
}

impl HelloS {
    /// Canonical encoding.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        header(&mut out, T_HELLO_S)?;
        out.extend_from_slice(&self.nonce);
        put_lv(&mut out, &self.device_id, MAX_ID)?;
        put_lv(&mut out, &self.purpose, MAX_PURPOSE)?;
        put_lv(&mut out, &self.claims, MAX_CLAIMS)?;
        Ok(out)
    }
    /// Strict decoding.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        check_header(&mut r, T_HELLO_S)?;
        let nonce = r.array::<NONCE_LEN>()?;
        let device_id = r.lv(MAX_ID)?.to_vec();
        let purpose = r.lv(MAX_PURPOSE)?.to_vec();
        let claims = r.lv(MAX_CLAIMS)?.to_vec();
        r.finish()?;
        Ok(Self {
            nonce,
            device_id,
            purpose,
            claims,
        })
    }
}

/// Message M2: sender hello plus CPace share Ya.
pub(crate) fn encode_m2(hello_s: &[u8], ya: &[u8]) -> Result<Vec<u8>> {
    if ya.len() != SHARE_LEN {
        return Err(Error::Pake);
    }
    let mut out = vec![T_M2];
    put_lv(&mut out, hello_s, 512)?;
    out.extend_from_slice(ya);
    Ok(out)
}

pub(crate) fn decode_m2(bytes: &[u8]) -> Result<(&[u8], [u8; SHARE_LEN])> {
    let mut r = Reader::new(bytes);
    if r.byte()? != T_M2 {
        return Err(Error::WrongState);
    }
    let hello = r.lv(512)?;
    let ya = r.array::<SHARE_LEN>()?;
    r.finish()?;
    Ok((hello, ya))
}

/// Message M3: responder share Yb plus receiver confirmation tag.
pub(crate) fn encode_m3(yb: &[u8], tag: &[u8; TAG_LEN]) -> Result<Vec<u8>> {
    if yb.len() != SHARE_LEN {
        return Err(Error::Pake);
    }
    let mut out = vec![T_M3];
    out.extend_from_slice(yb);
    out.extend_from_slice(tag);
    Ok(out)
}

pub(crate) fn decode_m3(bytes: &[u8]) -> Result<([u8; SHARE_LEN], [u8; TAG_LEN])> {
    let mut r = Reader::new(bytes);
    if r.byte()? != T_M3 {
        return Err(Error::WrongState);
    }
    let yb = r.array::<SHARE_LEN>()?;
    let tag = r.array::<TAG_LEN>()?;
    r.finish()?;
    Ok((yb, tag))
}

/// Message M4: sender confirmation tag.
pub(crate) fn encode_m4(tag: &[u8; TAG_LEN]) -> Vec<u8> {
    let mut out = vec![T_M4];
    out.extend_from_slice(tag);
    out
}

pub(crate) fn decode_m4(bytes: &[u8]) -> Result<[u8; TAG_LEN]> {
    let mut r = Reader::new(bytes);
    if r.byte()? != T_M4 {
        return Err(Error::WrongState);
    }
    let tag = r.array::<TAG_LEN>()?;
    r.finish()?;
    Ok(tag)
}
