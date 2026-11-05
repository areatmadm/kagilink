#![no_main]
//! Hello decoders: never panic; anything accepted must re-encode to the same bytes.
use areatm_kagilink_core::wire::{HelloR, HelloS};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(h) = HelloR::decode(data) {
        assert_eq!(h.encode().expect("re-encode HelloR"), data);
    }
    if let Ok(h) = HelloS::decode(data) {
        assert_eq!(h.encode().expect("re-encode HelloS"), data);
    }
});
