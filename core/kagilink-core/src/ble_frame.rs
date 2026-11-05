//! BLE chunk framing v1 (docs/BLE_GATT_SPEC.md section 5). Pure and I/O-free so every platform
//! can use the same code through the C ABI. Carries opaque bytes: it neither encrypts nor
//! authenticates anything and trusts nothing about the radio. Any error kills the reassembler.
//!
//! Chunk = `flags:u8 | seq:u8 | body`. `flags` bit0 FIRST, bit1 LAST, bits 2..7 reserved (0).
//! The FIRST chunk's body starts with `mtype:u8 | total_len:u16 BE`; later bodies are pure data.

use crate::error::{Error, Result};

/// Largest BLE message accepted (handshake messages are at most 274 bytes, fragments 269).
pub const MAX_BLE_MESSAGE: usize = 512;
/// Smallest chunk the codec can emit: 2 header + 3 first-chunk prefix + 1 data byte.
pub const MIN_CHUNK: usize = 6;
/// Largest chunk (an ATT value never exceeds 512 bytes).
pub const MAX_CHUNK: usize = 512;
/// Message type: one handshake message (M1..M4).
pub const MTYPE_HANDSHAKE: u8 = 1;
/// Message type: one secure-channel transport fragment.
pub const MTYPE_CHANNEL: u8 = 2;

const FIRST: u8 = 0b01;
const LAST: u8 = 0b10;
const RESERVED: u8 = !(FIRST | LAST);
const HEADER: usize = 2;
const PREFIX: usize = 3;

fn valid_mtype(m: u8) -> bool {
    m == MTYPE_HANDSHAKE || m == MTYPE_CHANNEL
}

/// Split one message into chunks of at most `max_chunk` bytes (ATT MTU - 3, capped by the caller).
pub fn chunk_message(mtype: u8, message: &[u8], max_chunk: usize) -> Result<Vec<Vec<u8>>> {
    if !valid_mtype(mtype) || message.is_empty() || message.len() > MAX_BLE_MESSAGE {
        return Err(Error::Limits);
    }
    if !(MIN_CHUNK..=MAX_CHUNK).contains(&max_chunk) {
        return Err(Error::Limits);
    }
    let total = message.len() as u16;
    let mut stream = Vec::with_capacity(PREFIX + message.len());
    stream.push(mtype);
    stream.extend_from_slice(&total.to_be_bytes());
    stream.extend_from_slice(message);

    let room = max_chunk - HEADER;
    let count = stream.len().div_ceil(room);
    if count > usize::from(u8::MAX) + 1 {
        return Err(Error::Limits);
    }
    let mut out = Vec::with_capacity(count);
    for (i, piece) in stream.chunks(room).enumerate() {
        let mut flags = 0;
        if i == 0 {
            flags |= FIRST;
        }
        if i + 1 == count {
            flags |= LAST;
        }
        let mut chunk = Vec::with_capacity(HEADER + piece.len());
        chunk.push(flags);
        chunk.push(i as u8);
        chunk.extend_from_slice(piece);
        out.push(chunk);
    }
    Ok(out)
}

enum State {
    Idle,
    Receiving {
        mtype: u8,
        total: usize,
        data: Vec<u8>,
        next_seq: u8,
    },
    Dead,
}

/// Strict in-order reassembler for one direction of one connection.
pub struct Reassembler {
    state: State,
}

impl Default for Reassembler {
    fn default() -> Self {
        Self::new()
    }
}

impl Reassembler {
    /// New reassembler waiting for a FIRST chunk.
    pub fn new() -> Self {
        Self { state: State::Idle }
    }

    /// True once an error has occurred; the connection must be dropped.
    pub fn is_dead(&self) -> bool {
        matches!(self.state, State::Dead)
    }

    fn die<T>(&mut self, e: Error) -> Result<T> {
        self.state = State::Dead;
        Err(e)
    }

    /// Feed one received chunk. Returns the finished `(mtype, message)` when the last chunk of a
    /// message arrives. Any error is final.
    pub fn push(&mut self, chunk: &[u8]) -> Result<Option<(u8, Vec<u8>)>> {
        if self.is_dead() {
            return Err(Error::Closed);
        }
        if chunk.len() <= HEADER || chunk.len() > MAX_CHUNK {
            return self.die(Error::Malformed);
        }
        let (flags, seq, body) = (chunk[0], chunk[1], &chunk[HEADER..]);
        if flags & RESERVED != 0 {
            return self.die(Error::Malformed);
        }
        let first = flags & FIRST != 0;
        let last = flags & LAST != 0;

        let (mtype, total, mut data, expected) =
            match std::mem::replace(&mut self.state, State::Dead) {
                State::Dead => return Err(Error::Closed),
                State::Idle => {
                    if !first || seq != 0 || body.len() < PREFIX {
                        return Err(Error::Framing);
                    }
                    let mtype = body[0];
                    let total = usize::from(u16::from_be_bytes([body[1], body[2]]));
                    if !valid_mtype(mtype) {
                        return Err(Error::Unsupported);
                    }
                    if total == 0 || total > MAX_BLE_MESSAGE {
                        return Err(Error::Limits);
                    }
                    (mtype, total, Vec::with_capacity(total), 0u8)
                }
                State::Receiving {
                    mtype,
                    total,
                    data,
                    next_seq,
                } => {
                    if first || seq != next_seq {
                        return Err(Error::Framing);
                    }
                    (mtype, total, data, next_seq)
                }
            };
        let payload = if first { &body[PREFIX..] } else { body };
        if data.len() + payload.len() > total {
            return Err(Error::Framing);
        }
        data.extend_from_slice(payload);

        if data.len() == total {
            if !last {
                return Err(Error::Framing);
            }
            self.state = State::Idle;
            return Ok(Some((mtype, data)));
        }
        if last {
            return Err(Error::Framing);
        }
        let next_seq = match expected.checked_add(1) {
            Some(n) => n,
            None => return Err(Error::Framing),
        };
        self.state = State::Receiving {
            mtype,
            total,
            data,
            next_seq,
        };
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(chunks: &[Vec<u8>]) -> Result<Option<(u8, Vec<u8>)>> {
        let mut r = Reassembler::new();
        let mut out = None;
        for c in chunks {
            out = r.push(c)?;
        }
        Ok(out)
    }

    fn msg(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i * 31 + 7) as u8).collect()
    }

    #[test]
    fn round_trip_all_lengths_and_chunk_sizes() {
        for max_chunk in [MIN_CHUNK, 20, 23, 100, 185, 244, 512] {
            for len in 1..=MAX_BLE_MESSAGE {
                let m = msg(len);
                let chunks = chunk_message(MTYPE_CHANNEL, &m, max_chunk).unwrap();
                assert!(chunks.iter().all(|c| c.len() <= max_chunk));
                let (t, back) = feed(&chunks).unwrap().unwrap();
                assert_eq!((t, back), (MTYPE_CHANNEL, m));
            }
        }
    }

    #[test]
    fn two_messages_back_to_back() {
        let mut r = Reassembler::new();
        for (t, len) in [
            (MTYPE_HANDSHAKE, 65),
            (MTYPE_CHANNEL, 269),
            (MTYPE_HANDSHAKE, 33),
        ] {
            let m = msg(len);
            let mut got = None;
            for c in chunk_message(t, &m, 20).unwrap() {
                got = r.push(&c).unwrap();
            }
            assert_eq!(got, Some((t, m)));
        }
    }

    #[test]
    fn rejects_bad_inputs_when_chunking() {
        assert!(chunk_message(0, b"x", 20).is_err());
        assert!(chunk_message(3, b"x", 20).is_err());
        assert!(chunk_message(MTYPE_CHANNEL, b"", 20).is_err());
        assert!(chunk_message(MTYPE_CHANNEL, &msg(MAX_BLE_MESSAGE + 1), 20).is_err());
        assert!(chunk_message(MTYPE_CHANNEL, b"x", MIN_CHUNK - 1).is_err());
        assert!(chunk_message(MTYPE_CHANNEL, b"x", MAX_CHUNK + 1).is_err());
    }

    #[test]
    fn duplicate_gap_and_reorder_are_fatal() {
        let chunks = chunk_message(MTYPE_CHANNEL, &msg(200), 20).unwrap();
        assert!(chunks.len() > 4);
        // duplicate
        let mut r = Reassembler::new();
        r.push(&chunks[0]).unwrap();
        assert!(r.push(&chunks[0]).is_err());
        assert!(r.is_dead());
        assert!(matches!(r.push(&chunks[1]), Err(Error::Closed)));
        // gap
        let mut r = Reassembler::new();
        r.push(&chunks[0]).unwrap();
        assert!(r.push(&chunks[2]).is_err());
        // reorder
        let mut r = Reassembler::new();
        r.push(&chunks[0]).unwrap();
        r.push(&chunks[1]).unwrap();
        assert!(r.push(&chunks[3]).is_err());
    }

    #[test]
    fn must_start_with_first_chunk_at_seq_zero() {
        let chunks = chunk_message(MTYPE_CHANNEL, &msg(100), 20).unwrap();
        assert!(Reassembler::new().push(&chunks[1]).is_err());
        let mut forged = chunks[0].clone();
        forged[1] = 1; // FIRST but seq 1
        assert!(Reassembler::new().push(&forged).is_err());
    }

    #[test]
    fn first_chunk_inside_an_unfinished_message_is_fatal() {
        let chunks = chunk_message(MTYPE_CHANNEL, &msg(100), 20).unwrap();
        let mut r = Reassembler::new();
        r.push(&chunks[0]).unwrap();
        assert!(r.push(&chunks[0]).is_err());
    }

    #[test]
    fn reserved_bits_and_short_chunks_are_malformed() {
        let chunks = chunk_message(MTYPE_HANDSHAKE, &msg(40), 20).unwrap();
        let mut bad = chunks[0].clone();
        bad[0] |= 0b100;
        assert!(matches!(
            Reassembler::new().push(&bad),
            Err(Error::Malformed)
        ));
        assert!(Reassembler::new().push(&[]).is_err());
        assert!(Reassembler::new().push(&[FIRST]).is_err());
        assert!(Reassembler::new().push(&[FIRST, 0]).is_err());
        assert!(Reassembler::new()
            .push(&vec![FIRST; MAX_CHUNK + 1])
            .is_err());
    }

    #[test]
    fn unknown_mtype_and_bad_lengths_are_rejected() {
        let mut c = chunk_message(MTYPE_CHANNEL, &msg(10), 20)
            .unwrap()
            .remove(0);
        c[2] = 9; // mtype
        assert!(matches!(
            Reassembler::new().push(&c),
            Err(Error::Unsupported)
        ));
        let mut c = chunk_message(MTYPE_CHANNEL, &msg(10), 20)
            .unwrap()
            .remove(0);
        c[3] = 0;
        c[4] = 0; // total 0
        assert!(matches!(Reassembler::new().push(&c), Err(Error::Limits)));
        let mut c = chunk_message(MTYPE_CHANNEL, &msg(10), 20)
            .unwrap()
            .remove(0);
        c[3] = 0x02;
        c[4] = 0x01; // 513
        assert!(matches!(Reassembler::new().push(&c), Err(Error::Limits)));
    }

    #[test]
    fn length_and_last_flag_must_agree() {
        let chunks = chunk_message(MTYPE_CHANNEL, &msg(60), 20).unwrap();
        // LAST chunk without LAST flag
        let mut missing_last = chunks.clone();
        let n = missing_last.len() - 1;
        missing_last[n][0] &= !LAST;
        assert!(feed(&missing_last).is_err());
        // LAST flag too early
        let mut early = chunks.clone();
        early[1][0] |= LAST;
        assert!(feed(&early).is_err());
        // declared length smaller than data sent
        let mut short_decl = chunks.clone();
        short_decl[0][3] = 0;
        short_decl[0][4] = 30;
        assert!(feed(&short_decl).is_err());
        // declared length larger than data sent
        let mut long_decl = chunks;
        long_decl[0][3] = 0;
        long_decl[0][4] = 100;
        assert!(feed(&long_decl).is_err());
    }

    #[test]
    fn random_structure_mutations_never_panic() {
        let m = msg(300);
        let chunks = chunk_message(MTYPE_CHANNEL, &m, 20).unwrap();
        let mut x = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = || {
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        };
        for _ in 0..3000 {
            let mut set = chunks.clone();
            let ci = (next() as usize) % set.len();
            let bi = (next() as usize) % set[ci].len();
            set[ci][bi] ^= (next() as u8) | 1;
            if let Ok(Some((_, back))) = feed(&set) {
                // A flip inside the data region is not detected here: the layer above
                // (AES-GCM / key confirmation) authenticates. Only structure is checked.
                assert_eq!(back.len(), m.len());
            }
        }
    }
}
