//! Six-digit one-time code. It is a PAKE password, never an encryption key.

use crate::error::{Error, Result};
use rand_core::CryptoRng;
use zeroize::Zeroize;

/// Largest multiple of 10^6 not above 2^32; samples at or above it are rejected so every
/// digit string is equally likely (no modulo bias).
const LIMIT: u32 = 4_294_000_000;

/// A six-digit code held as ASCII; erased on drop. Deliberately no `Debug`/`Display`.
pub struct Code([u8; 6]);

impl Code {
    /// Draw a uniformly random code from the supplied cryptographic RNG.
    pub fn generate(rng: &mut impl CryptoRng) -> Self {
        loop {
            let mut raw = [0u8; 4];
            rng.fill_bytes(&mut raw);
            let value = u32::from_le_bytes(raw);
            raw.zeroize();
            if value < LIMIT {
                let mut n = value % 1_000_000;
                let mut digits = [b'0'; 6];
                for slot in digits.iter_mut().rev() {
                    *slot = b'0' + (n % 10) as u8;
                    n /= 10;
                }
                return Self(digits);
            }
        }
    }

    /// Parse user input: exactly six ASCII digits.
    pub fn parse(text: &str) -> Result<Self> {
        let bytes = text.as_bytes();
        if bytes.len() != 6 || !bytes.iter().all(u8::is_ascii_digit) {
            return Err(Error::BadCode);
        }
        let mut digits = [0u8; 6];
        digits.copy_from_slice(bytes);
        Ok(Self(digits))
    }

    /// The ASCII digits, for display to the user or as the PAKE password only.
    pub fn expose(&self) -> &[u8; 6] {
        &self.0
    }

    /// Text form for on-screen display.
    pub fn as_str(&self) -> &str {
        // Invariant: always six ASCII digits.
        core::str::from_utf8(&self.0).unwrap_or("000000")
    }
}

impl Drop for Code {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
