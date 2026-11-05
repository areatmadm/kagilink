//! Receiver-side BLE session driver (docs/BLE_GATT_SPEC.md sections 4-6), transport-agnostic.
//!
//! The driver talks to a `GattLink`: write one chunk to the RX characteristic, wait for one chunk
//! from TX. It owns chunking, reassembly, the application ACK for Notify mode, per-message
//! timeouts, and the handshake M1..M4 plus the SECRET/RESULT exchange. All cryptography and the
//! 60 s / failure-budget rules stay in the shared core. Synthetic data only; the received
//! plaintext is never returned, logged or printed (only its length).

use areatm_kagilink_core::ble_frame::{
    ack_chunk, chunk_message, Reassembler, MTYPE_CHANNEL, MTYPE_HANDSHAKE,
};
use areatm_kagilink_core::{kind, Error as CoreError, ReceiverSession};
use rand_core::CryptoRng;
use std::time::{Duration, Instant};

/// A first chunk of the next message must arrive within this time (the phone runs CPace).
pub const FIRST_CHUNK_WAIT: Duration = Duration::from_secs(10);
/// A message must complete within this time of its FIRST chunk (spec section 6).
pub const MESSAGE_COMPLETE: Duration = Duration::from_secs(5);
/// Largest payload per channel fragment the receiver asks for when it must seal (RESULT).
const FRAGMENT_PAYLOAD: usize = 200;

/// Link-level failure (not a protocol verdict).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkError {
    /// The peer disconnected or the write failed.
    Disconnected,
}

/// One GATT connection as the driver sees it.
pub trait GattLink {
    /// Write one chunk to RX with response (central -> peripheral).
    fn write_rx(&mut self, chunk: &[u8]) -> Result<(), LinkError>;
    /// Wait up to `timeout` for one chunk from TX (indication or notification).
    fn next_tx(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, LinkError>;
    /// Largest value that fits one ATT write/notification (ATT MTU - 3); at least 20.
    fn chunk_size(&self) -> usize;
    /// True when TX is Notify, so every received chunk must be answered with an application ACK.
    fn tx_needs_ack(&self) -> bool;
    /// Drop the connection.
    fn disconnect(&mut self);
}

/// Why a session attempt ended without a transfer.
#[derive(Debug, PartialEq, Eq)]
pub enum SessionError {
    /// The shared core rejected something (wrong code, tamper, expiry, budget, state ...).
    Core(CoreError),
    /// The link failed.
    Link(LinkError),
    /// A message did not start or complete in time.
    Timeout,
    /// The peer sent a message type that is not allowed at this point.
    UnexpectedMessage,
    /// The peer sent a SECRET-free or wrongly kinded channel message.
    UnexpectedKind,
}

impl From<CoreError> for SessionError {
    fn from(e: CoreError) -> Self {
        SessionError::Core(e)
    }
}

impl From<LinkError> for SessionError {
    fn from(e: LinkError) -> Self {
        SessionError::Link(e)
    }
}

/// What a successful attempt delivered (never the plaintext).
#[derive(Debug, PartialEq, Eq)]
pub struct ReceiveReport {
    /// Length of the received secret.
    pub received_len: usize,
    /// Number of channel fragments received.
    pub fragments: usize,
}

fn send_message<L: GattLink>(link: &mut L, mtype: u8, message: &[u8]) -> Result<(), SessionError> {
    let chunks = chunk_message(mtype, message, link.chunk_size())?;
    for c in &chunks {
        link.write_rx(c)?;
    }
    Ok(())
}

/// Receive one complete BLE message (mtype, data). In Notify mode each chunk is ACKed.
fn recv_message<L: GattLink>(link: &mut L) -> Result<(u8, Vec<u8>), SessionError> {
    let mut re = Reassembler::new();
    let first_deadline = Instant::now() + FIRST_CHUNK_WAIT;
    let mut message_deadline: Option<Instant> = None;
    loop {
        let deadline = message_deadline.unwrap_or(first_deadline);
        let now = Instant::now();
        if now >= deadline {
            return Err(SessionError::Timeout);
        }
        let chunk = match link.next_tx(deadline - now)? {
            Some(c) => c,
            None => return Err(SessionError::Timeout),
        };
        if message_deadline.is_none() {
            message_deadline = Some(Instant::now() + MESSAGE_COMPLETE);
        }
        let seq = chunk.get(1).copied();
        let pushed = re.push(&chunk)?;
        if link.tx_needs_ack() {
            if let Some(s) = seq {
                link.write_rx(&ack_chunk(s))?;
            }
        }
        if let Some(done) = pushed {
            return Ok(done);
        }
    }
}

/// Run ONE attempt on an established connection with the sender chosen by the user.
///
/// The caller owns `rx` so the failure budget survives across connections: a fresh connection
/// is a fresh attempt on the same code. The caller displays the code (`rx.code()`) before the
/// sender needs it. On any error the link is dropped; the core has already applied D026 counting.
pub fn run_attempt<L: GattLink>(
    link: &mut L,
    rx: &mut ReceiverSession,
    rng: &mut impl CryptoRng,
) -> Result<ReceiveReport, SessionError> {
    let result = run_attempt_inner(link, rx, rng);
    link.disconnect();
    result
}

fn run_attempt_inner<L: GattLink>(
    link: &mut L,
    rx: &mut ReceiverSession,
    rng: &mut impl CryptoRng,
) -> Result<ReceiveReport, SessionError> {
    let m1 = rx.begin_attempt(rng)?;
    send_message(link, MTYPE_HANDSHAKE, &m1)?;

    let (t, m2) = recv_message(link)?;
    if t != MTYPE_HANDSHAKE {
        return Err(SessionError::UnexpectedMessage);
    }
    let m3 = rx.handle_m2(rng, &m2)?;
    send_message(link, MTYPE_HANDSHAKE, &m3)?;

    let (t, m4) = recv_message(link)?;
    if t != MTYPE_HANDSHAKE {
        return Err(SessionError::UnexpectedMessage);
    }
    let mut channel = rx.handle_m4(&m4)?;
    channel.set_ready_to_receive(true);

    let mut fragments = 0usize;
    let received_len = loop {
        let (t, frag) = recv_message(link)?;
        if t != MTYPE_CHANNEL {
            channel.close();
            return Err(SessionError::UnexpectedMessage);
        }
        fragments += 1;
        match channel.open_fragment(&frag) {
            Ok(Some(msg)) => {
                if msg.kind != kind::SECRET {
                    channel.close();
                    return Err(SessionError::UnexpectedKind);
                }
                break msg.plaintext.len();
            }
            Ok(None) => {}
            Err(e) => return Err(e.into()),
        }
    };

    // RESULT report: only the length is reported, never content.
    let report = format!("received:{received_len}");
    match channel.seal(kind::RESULT, report.as_bytes(), FRAGMENT_PAYLOAD) {
        Ok(frags) => {
            for f in &frags {
                send_message(link, MTYPE_CHANNEL, f)?;
            }
        }
        Err(e) => {
            channel.close();
            return Err(e.into());
        }
    }
    channel.close();
    Ok(ReceiveReport {
        received_len,
        fragments,
    })
}
