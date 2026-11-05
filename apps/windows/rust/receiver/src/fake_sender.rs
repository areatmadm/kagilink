//! In-process fake phone for the BLE session driver (synthetic data only, no radio).
//!
//! It plays the KL-V1 peripheral: reassembles what the driver writes to RX, runs the real core
//! sender (`SenderPending`), and queues what it would indicate/notify on TX as chunks. Behaviours
//! model both an honest phone and the faults listed in docs/BLE_GATT_SPEC.md section 8. It proves
//! the driver and framing logic only; it says nothing about real BLE stacks.

use crate::session::{GattLink, LinkError};
use areatm_kagilink_core::ble_frame::{
    chunk_message, parse_ack, Reassembler, MTYPE_CHANNEL, MTYPE_HANDSHAKE,
};
use areatm_kagilink_core::{kind, os_rng, Clock, Code, SecureChannel, SenderPending, SystemClock};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

/// How the fake phone behaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behaviour {
    /// Correct code, sends the SECRET.
    Honest,
    /// Uses a different code (a look-alike phone or a typo): M3 is received but M4 never verifies.
    WrongCode,
    /// Answers M1 with bytes that are not an M2 (fails before M3: free under D026).
    GarbageM2,
    /// Flips one byte of M4 (bad confirmation after M3: counts one failure).
    TamperM4,
    /// Flips one byte of the first SECRET fragment ciphertext (channel must close).
    TamperSecret,
    /// Never answers.
    Silent,
    /// Disconnects right after receiving M3.
    DropAfterM3,
    /// Sends the first TX chunk twice (duplicate).
    DuplicateChunk,
    /// Skips the second chunk of its first multi-chunk message (gap).
    SkipChunk,
    /// Sends a FIRST chunk that declares an oversize length.
    OversizeLength,
}

/// The fake peripheral, also the `GattLink` the driver sees.
pub struct FakeSender {
    behaviour: Behaviour,
    code: Code,
    chunk_size: usize,
    notify_mode: bool,
    secret: Vec<u8>,
    reassembler: Reassembler,
    pending: Option<SenderPending>,
    channel: Option<SecureChannel>,
    tx_queue: VecDeque<Vec<u8>>,
    awaiting_ack: Option<u8>,
    ack_enforced: bool,
    disconnected: bool,
    clock: Arc<dyn Clock>,
    /// Number of RESULT messages the phone received and authenticated.
    pub results_received: usize,
    /// Number of application ACK chunks the phone received.
    pub acks_received: usize,
    /// True if the driver wrote a non-ACK chunk before the previous notified chunk was ACKed.
    pub ack_violation: bool,
    skipped: bool,
}

impl FakeSender {
    /// Create a phone that knows `code` (the code shown by the receiver).
    pub fn new(
        code: &str,
        secret: &[u8],
        behaviour: Behaviour,
        chunk_size: usize,
        notify_mode: bool,
    ) -> Self {
        let wrong = if behaviour == Behaviour::WrongCode {
            // Same length, different digits.
            code.chars()
                .map(|c| if c == '0' { '1' } else { '0' })
                .collect::<String>()
        } else {
            code.to_string()
        };
        Self {
            behaviour,
            code: Code::parse(&wrong).expect("test code"),
            chunk_size,
            notify_mode,
            secret: secret.to_vec(),
            reassembler: Reassembler::new(),
            pending: None,
            channel: None,
            tx_queue: VecDeque::new(),
            awaiting_ack: None,
            ack_enforced: notify_mode,
            disconnected: false,
            clock: Arc::new(SystemClock::new()),
            results_received: 0,
            acks_received: 0,
            ack_violation: false,
            skipped: false,
        }
    }

    fn queue(&mut self, mtype: u8, message: &[u8]) {
        let mut chunks = chunk_message(mtype, message, self.chunk_size).expect("chunk");
        match self.behaviour {
            Behaviour::DuplicateChunk if !self.skipped => {
                self.skipped = true;
                let first = chunks[0].clone();
                chunks.insert(0, first);
            }
            Behaviour::SkipChunk if !self.skipped && chunks.len() > 2 => {
                self.skipped = true;
                chunks.remove(1);
            }
            Behaviour::OversizeLength if !self.skipped => {
                self.skipped = true;
                // flags FIRST|LAST, seq 0, mtype, length 0xFFFF, one data byte.
                chunks = vec![vec![0b011, 0, mtype, 0xFF, 0xFF, 0]];
            }
            _ => {}
        }
        self.tx_queue.extend(chunks);
    }

    fn on_message(&mut self, mtype: u8, data: Vec<u8>) {
        match (mtype, self.behaviour) {
            (_, Behaviour::Silent) => {}
            (MTYPE_HANDSHAKE, Behaviour::GarbageM2) if self.pending.is_none() => {
                self.queue(MTYPE_HANDSHAKE, &[0xA5; 40]);
            }
            (MTYPE_HANDSHAKE, _) if self.pending.is_none() && self.channel.is_none() => {
                let mut rng = os_rng();
                match SenderPending::start(
                    self.clock.clone(),
                    &mut rng,
                    &self.code,
                    &data,
                    b"fake-phone",
                    b"synthetic-login",
                    b"mode=fake",
                ) {
                    Ok((m2, p)) => {
                        self.pending = Some(p);
                        self.queue(MTYPE_HANDSHAKE, &m2);
                    }
                    Err(_) => self.disconnected = true,
                }
            }
            (MTYPE_HANDSHAKE, _) if self.pending.is_some() => {
                if self.behaviour == Behaviour::DropAfterM3 {
                    self.disconnected = true;
                    return;
                }
                let p = self.pending.take().expect("pending");
                match p.handle_m3(&data) {
                    Ok((mut m4, mut ch)) => {
                        if self.behaviour == Behaviour::TamperM4 {
                            let last = m4.len() - 1;
                            m4[last] ^= 1;
                        }
                        self.queue(MTYPE_HANDSHAKE, &m4);
                        ch.approve_one_transfer();
                        if let Ok(mut frags) = ch.seal(kind::SECRET, &self.secret, 200) {
                            if self.behaviour == Behaviour::TamperSecret {
                                let last = frags[0].len() - 1;
                                frags[0][last] ^= 1;
                            }
                            for f in frags {
                                self.queue(MTYPE_CHANNEL, &f);
                            }
                        }
                        self.channel = Some(ch);
                    }
                    Err(_) => self.disconnected = true,
                }
            }
            (MTYPE_CHANNEL, _) => {
                if let Some(ch) = self.channel.as_mut() {
                    if let Ok(Some(m)) = ch.open_fragment(&data) {
                        if m.kind == kind::RESULT {
                            self.results_received += 1;
                        }
                    }
                }
            }
            _ => self.disconnected = true,
        }
    }
}

impl GattLink for FakeSender {
    fn write_rx(&mut self, chunk: &[u8]) -> Result<(), LinkError> {
        if self.disconnected {
            return Err(LinkError::Disconnected);
        }
        if let Some(seq) = parse_ack(chunk) {
            self.acks_received += 1;
            if self.awaiting_ack == Some(seq) {
                self.awaiting_ack = None;
            }
            return Ok(());
        }
        if self.ack_enforced && self.awaiting_ack.is_some() {
            self.ack_violation = true;
        }
        match self.reassembler.push(chunk) {
            Ok(Some((mtype, data))) => self.on_message(mtype, data),
            Ok(None) => {}
            Err(_) => {
                self.disconnected = true;
                return Err(LinkError::Disconnected);
            }
        }
        if self.disconnected {
            return Err(LinkError::Disconnected);
        }
        Ok(())
    }

    fn next_tx(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>, LinkError> {
        if self.disconnected && self.tx_queue.is_empty() {
            return Err(LinkError::Disconnected);
        }
        if self.notify_mode && self.awaiting_ack.is_some() {
            // Stop-and-wait: nothing is released until the previous chunk is acknowledged.
            return Ok(None);
        }
        let next = self.tx_queue.pop_front();
        if let (Some(c), true) = (&next, self.notify_mode) {
            self.awaiting_ack = c.get(1).copied();
        }
        Ok(next)
    }

    fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    fn tx_needs_ack(&self) -> bool {
        self.notify_mode
    }

    fn disconnect(&mut self) {
        self.disconnected = true;
    }
}
