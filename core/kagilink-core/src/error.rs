//! Errors carry no secret material and never echo input bytes.

/// Failure reasons. Every protocol failure is terminal for the current attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The 60-second code/session deadline has passed.
    Expired,
    /// The code session was consumed, discarded after three failures, or is not active.
    NotActive,
    /// Three failed attempts: the code has been destroyed.
    TooManyFailures,
    /// An attempt is already in progress for this code.
    AttemptInProgress,
    /// Message is malformed, oversized, has trailing bytes or unknown fields.
    Malformed,
    /// Unsupported protocol version or suite (no downgrade).
    Unsupported,
    /// Message arrived in the wrong protocol state.
    WrongState,
    /// The PAKE rejected a share (invalid point, identity, bad length).
    Pake,
    /// Key confirmation failed (wrong code or tampering).
    ConfirmationFailed,
    /// The code text is not exactly six ASCII digits.
    BadCode,
    /// AEAD authentication failed or the envelope is inconsistent.
    Authentication,
    /// Fragment was duplicated, missing, reordered, conflicting or out of bounds.
    Framing,
    /// Message length or kind is not allowed.
    Limits,
    /// Counter space for this direction is exhausted.
    CounterExhausted,
    /// The explicit one-transfer approval is missing or already consumed.
    NotApproved,
    /// The channel is closed after an error, expiry or completion.
    Closed,
    /// The operating system could not provide randomness.
    Randomness,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Error {}

/// Result alias.
pub type Result<T> = core::result::Result<T, Error>;
