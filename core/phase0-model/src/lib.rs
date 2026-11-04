//! Synthetic feasibility model only. No cryptography, transport, secrets or FFI.
//! Simulation events below MUST NOT be used as production authentication evidence.
#![forbid(unsafe_code)]

pub const REAL_SECRET_TRANSFER_ENABLED: bool = false;
pub const TTL_MS: u64 = 60_000;
pub const MAX_MESSAGE: usize = 4096;
pub const MAX_FRAGMENT: usize = 256;
pub const MAX_FRAGMENTS: u16 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Closed,
    Expired,
    Order,
    Authentication,
    Limit,
    Frame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Pairing,
    Confirmed,
    Approved,
    Closed,
}

/// Clock values are synthetic monotonic milliseconds. No secret material is held.
pub struct Simulation {
    created: u64,
    last: u64,
    failures: u8,
    stage: Stage,
}

impl Simulation {
    pub fn new(now: u64) -> Self {
        Self {
            created: now,
            last: now,
            failures: 0,
            stage: Stage::Pairing,
        }
    }

    fn check(&mut self, now: u64) -> Result<(), Error> {
        if self.stage == Stage::Closed {
            return Err(Error::Closed);
        }
        if now < self.last || now.saturating_sub(self.created) >= TTL_MS {
            self.stage = Stage::Closed;
            return Err(Error::Expired);
        }
        self.last = now;
        Ok(())
    }

    /// Simulates a failed *fresh* PAKE attempt. No PAKE is implemented here.
    pub fn failed_attempt(&mut self, now: u64) -> Result<(), Error> {
        self.check(now)?;
        if self.stage != Stage::Pairing {
            return self.fail(Error::Order);
        }
        self.failures += 1;
        if self.failures >= 3 {
            return self.fail(Error::Limit);
        }
        Err(Error::Authentication)
    }

    /// Models mutual confirmation, not a verifier for received network messages.
    pub fn simulate_mutual_confirmation(&mut self, now: u64) -> Result<(), Error> {
        self.check(now)?;
        if self.stage != Stage::Pairing {
            return self.fail(Error::Order);
        }
        self.stage = Stage::Confirmed;
        Ok(())
    }

    pub fn simulate_approval(&mut self, now: u64) -> Result<(), Error> {
        self.check(now)?;
        if self.stage != Stage::Confirmed {
            return self.fail(Error::Order);
        }
        self.stage = Stage::Approved;
        Ok(())
    }

    /// Consumes the simulation. No payload parameter exists, and no input is sent.
    pub fn consume(&mut self, now: u64) -> Result<(), Error> {
        self.check(now)?;
        if self.stage != Stage::Approved {
            return self.fail(Error::Order);
        }
        self.stage = Stage::Closed;
        Ok(())
    }

    pub fn abort(&mut self) {
        self.stage = Stage::Closed;
    }

    fn fail<T>(&mut self, error: Error) -> Result<T, Error> {
        self.abort();
        Err(error)
    }
}

/// Public synthetic metadata, not the final wire format or authenticated data.
pub struct Fragment<'a> {
    pub session: u128,
    pub message: u32,
    pub index: u16,
    pub count: u16,
    pub total: usize,
    pub bytes: &'a [u8],
}

/// Bounded, strictly ordered, single-message synthetic assembler.
/// The production version must authenticate the full envelope before use.
pub struct Assembler {
    session: u128,
    message: u32,
    shape: Option<(u16, usize)>,
    next: u16,
    bytes: Vec<u8>,
    closed: bool,
}

impl Assembler {
    pub fn new(session: u128, message: u32) -> Self {
        Self {
            session,
            message,
            shape: None,
            next: 0,
            bytes: Vec::new(),
            closed: false,
        }
    }

    pub fn push(&mut self, f: Fragment<'_>) -> Result<Option<Vec<u8>>, Error> {
        if self.closed {
            return Err(Error::Closed);
        }
        if f.session != self.session
            || f.message != self.message
            || f.index != self.next
            || f.count == 0
            || f.count > MAX_FRAGMENTS
            || f.index >= f.count
            || f.total == 0
            || f.total > MAX_MESSAGE
            || f.bytes.is_empty()
            || f.bytes.len() > MAX_FRAGMENT
            || self.bytes.len() + f.bytes.len() > f.total
            || self.shape.is_some_and(|s| s != (f.count, f.total))
        {
            return self.reject();
        }
        self.shape = Some((f.count, f.total));
        self.bytes.extend_from_slice(f.bytes);
        self.next += 1;
        if self.next == f.count {
            if self.bytes.len() != f.total {
                return self.reject();
            }
            self.closed = true;
            return Ok(Some(std::mem::take(&mut self.bytes)));
        }
        if self.bytes.len() >= f.total {
            return self.reject();
        }
        Ok(None)
    }

    /// Disconnect/deadline: irreversibly discard incomplete synthetic data.
    pub fn abort(&mut self) {
        self.closed = true;
        self.bytes.clear(); // NOT a secret zeroization implementation.
    }

    fn reject<T>(&mut self) -> Result<T, Error> {
        self.abort();
        Err(Error::Frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_is_one_use() {
        let mut s = Simulation::new(100);
        assert_eq!(s.simulate_mutual_confirmation(101), Ok(()));
        assert_eq!(s.simulate_approval(102), Ok(()));
        assert_eq!(s.consume(103), Ok(()));
        assert_eq!(s.consume(104), Err(Error::Closed));
    }

    #[test]
    fn no_transfer_before_confirmation_and_approval() {
        for confirmed in [false, true] {
            let mut s = Simulation::new(0);
            if confirmed {
                s.simulate_mutual_confirmation(1).unwrap();
            }
            assert_eq!(s.consume(2), Err(Error::Order));
            assert_eq!(s.simulate_approval(3), Err(Error::Closed));
        }
    }

    #[test]
    fn approval_before_confirmation_closes() {
        let mut s = Simulation::new(0);
        assert_eq!(s.simulate_approval(1), Err(Error::Order));
        assert_eq!(s.simulate_mutual_confirmation(2), Err(Error::Closed));
    }

    #[test]
    fn ttl_boundary_and_clock_regression_close() {
        for now in [TTL_MS, TTL_MS + 1] {
            let mut s = Simulation::new(0);
            assert_eq!(s.simulate_mutual_confirmation(now), Err(Error::Expired));
        }
        let mut s = Simulation::new(100);
        assert_eq!(s.simulate_mutual_confirmation(99), Err(Error::Expired));
        let mut s = Simulation::new(0);
        assert_eq!(s.simulate_mutual_confirmation(TTL_MS - 1), Ok(()));
        assert_eq!(s.simulate_approval(TTL_MS), Err(Error::Expired));
    }

    #[test]
    fn third_failure_discards_session() {
        let mut s = Simulation::new(0);
        assert_eq!(s.failed_attempt(1), Err(Error::Authentication));
        assert_eq!(s.failed_attempt(2), Err(Error::Authentication));
        assert_eq!(s.failed_attempt(3), Err(Error::Limit));
        assert_eq!(s.simulate_mutual_confirmation(4), Err(Error::Closed));
    }

    #[test]
    fn cancellation_and_disconnect_are_terminal() {
        let mut s = Simulation::new(0);
        s.abort();
        assert_eq!(s.simulate_mutual_confirmation(1), Err(Error::Closed));
    }

    fn fragment(index: u16, bytes: &[u8]) -> Fragment<'_> {
        Fragment {
            session: 1,
            message: 7,
            index,
            count: 2,
            total: 4,
            bytes,
        }
    }

    #[test]
    fn ordered_reassembly_is_one_use() {
        let mut a = Assembler::new(1, 7);
        assert_eq!(a.push(fragment(0, b"ab")), Ok(None));
        assert_eq!(a.push(fragment(1, b"cd")), Ok(Some(b"abcd".to_vec())));
        assert_eq!(a.push(fragment(0, b"ab")), Err(Error::Closed));
    }

    #[test]
    fn reordered_missing_and_duplicate_fragments_fail() {
        let mut a = Assembler::new(1, 7);
        assert_eq!(a.push(fragment(1, b"cd")), Err(Error::Frame));
        let mut a = Assembler::new(1, 7);
        a.push(fragment(0, b"ab")).unwrap();
        assert_eq!(a.push(fragment(0, b"ab")), Err(Error::Frame));
        let mut a = Assembler::new(1, 7);
        a.push(fragment(0, b"ab")).unwrap();
        a.abort(); // An outer monotonic deadline must call abort for a missing frame.
        assert_eq!(a.push(fragment(1, b"cd")), Err(Error::Closed));
    }

    #[test]
    fn prior_session_and_wrong_message_rejected() {
        for (session, message) in [(2, 7), (1, 8)] {
            let mut a = Assembler::new(session, message);
            assert_eq!(a.push(fragment(0, b"ab")), Err(Error::Frame));
        }
    }

    #[test]
    fn malformed_shape_and_limits_fail_closed() {
        for variant in 0..8 {
            let mut a = Assembler::new(1, 7);
            let large = [0; MAX_FRAGMENT + 1];
            let mut f = fragment(0, b"ab");
            match variant {
                0 => f.total = MAX_MESSAGE + 1,
                1 => f.count = 0,
                2 => f.count = MAX_FRAGMENTS + 1,
                3 => f.bytes = &[],
                4 => f.bytes = &large,
                5 => f.total = 1,
                6 => f.count = 1,
                _ => f.total = 0,
            }
            assert_eq!(a.push(f), Err(Error::Frame));
            assert_eq!(a.push(fragment(0, b"ab")), Err(Error::Closed));
        }
    }

    #[test]
    fn changed_metadata_and_short_final_fail() {
        for change_count in [false, true] {
            let mut a = Assembler::new(1, 7);
            a.push(fragment(0, b"ab")).unwrap();
            let mut f = fragment(1, b"c");
            if change_count {
                f.count = 3;
            }
            assert_eq!(a.push(f), Err(Error::Frame));
        }
    }
}
