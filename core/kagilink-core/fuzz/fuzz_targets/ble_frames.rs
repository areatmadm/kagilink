#![no_main]
//! BLE chunk reassembler: arbitrary chunk sequences never panic; a finished message always has a
//! valid type and a length in 1..=512; after an error the reassembler stays dead.
use areatm_kagilink_core::ble_frame::{
    chunk_message, Reassembler, MAX_BLE_MESSAGE, MTYPE_CHANNEL, MTYPE_HANDSHAKE,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Part 1: the input as a stream of length-prefixed chunks.
    let mut r = Reassembler::new();
    let mut rest = data;
    while let Some((&n, tail)) = rest.split_first() {
        let n = usize::from(n).min(tail.len());
        let (chunk, next) = tail.split_at(n);
        rest = next;
        let was_dead = r.is_dead();
        match r.push(chunk) {
            Ok(Some((mtype, msg))) => {
                assert!(!was_dead);
                assert!(mtype == MTYPE_HANDSHAKE || mtype == MTYPE_CHANNEL);
                assert!((1..=MAX_BLE_MESSAGE).contains(&msg.len()));
            }
            Ok(None) => assert!(!was_dead),
            Err(_) => assert!(r.is_dead()),
        }
    }
    // Part 2: a genuine chunking of (a prefix of) the input must round trip.
    if let Some((&size, body)) = data.split_first() {
        let body = &body[..body.len().min(MAX_BLE_MESSAGE)];
        let max_chunk = 6 + usize::from(size) % 238;
        if !body.is_empty() {
            let chunks = chunk_message(MTYPE_CHANNEL, body, max_chunk).expect("valid input");
            let mut r = Reassembler::new();
            let mut got = None;
            for c in &chunks {
                got = r.push(c).expect("own chunks are accepted");
            }
            assert_eq!(got, Some((MTYPE_CHANNEL, body.to_vec())));
        }
    }
});
