//! Monotonic time source so deadlines are testable and never use wall-clock time.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Monotonic milliseconds since an arbitrary origin.
pub trait Clock: Send + Sync {
    /// Current monotonic time in milliseconds.
    fn now_ms(&self) -> u64;
}

/// Production clock based on [`Instant`].
pub struct SystemClock(Instant);

impl SystemClock {
    /// Create a clock whose origin is now.
    pub fn new() -> Self {
        Self(Instant::now())
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Manually advanced clock for tests.
#[derive(Default)]
pub struct ManualClock(AtomicU64);

impl ManualClock {
    /// Start at zero.
    pub fn new() -> Self {
        Self::default()
    }
    /// Advance by `ms` milliseconds.
    pub fn advance(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
