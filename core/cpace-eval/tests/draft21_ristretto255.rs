//! Conformance of `pakery-cpace` 0.6.2 to draft-irtf-cfrg-cpace-21 Appendix B.3
//! (CPACE-RISTR255-SHA512). Vectors come from `data/mod.rs`, generated from the
//! draft text by tools/extract-cpace-vectors.py. Passing proves agreement with these
//! vectors only; it is not an audit and says nothing about side channels or erasure.
mod data;
use data::*;

use pakery_core::crypto::CpaceGroup;
use pakery_cpace::generator::{calculate_generator, generator_string};
use pakery_cpace::transcript::{derive_session_id, transcript_ir, transcript_oc, CpaceMode};
use pakery_cpace::{CpaceCiphersuite, CpaceInitiator, CpaceResponder};
use pakery_crypto::{Ristretto255Group, Sha512Hash};

struct Suite;
impl CpaceCiphersuite for Suite {
    type Group = Ristretto255Group;
    type Hash = Sha512Hash;
    const DSI: &'static [u8] = b"CPaceRistretto255";
    const HASH_BLOCK_SIZE: usize = 128;
    const FIELD_SIZE_BYTES: usize = 32;
}

fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0);
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// Replays a fixed scalar (little-endian, zero-extended to the wide sample).
struct FixedRng {
    scalar: [u8; 32],
    used: bool,
}
impl FixedRng {
    fn new(hex: &str) -> Self {
        Self {
            scalar: unhex(hex).try_into().unwrap(),
            used: false,
        }
    }
}
impl rand_core::TryRng for FixedRng {
    type Error = rand_core::Infallible;
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        unimplemented!()
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        unimplemented!()
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        dest.fill(0);
        if !self.used {
            let n = dest.len().min(32);
            dest[..n].copy_from_slice(&self.scalar[..n]);
            self.used = true;
        }
        Ok(())
    }
}
impl rand_core::TryCryptoRng for FixedRng {}

const AD_A: &[u8] = b"ADa";
const AD_B: &[u8] = b"ADb";

#[test]
fn generator_string_and_generator_match_draft() {
    let (prs, ci, sid) = (unhex(PRS_HEX), unhex(CI_HEX), unhex(SID_HEX));
    assert_eq!(
        generator_string::<Suite>(&prs, &ci, &sid),
        unhex(GENERATOR_STRING_HEX)
    );
    let g = calculate_generator::<Suite>(&prs, &ci, &sid).unwrap();
    assert_eq!(g.to_bytes(), unhex(GENERATOR_HEX));
}

#[test]
fn transcripts_match_draft() {
    let (ya, yb) = (unhex(YA_HEX), unhex(YB_HEX));
    assert_eq!(
        transcript_ir(&ya, AD_A, &yb, AD_B),
        unhex(TRANSCRIPT_IR_HEX)
    );
    assert_eq!(
        transcript_oc(&ya, AD_A, &yb, AD_B),
        unhex(TRANSCRIPT_OC_HEX)
    );
}

#[test]
fn session_id_output_matches_draft() {
    let (ya, yb) = (unhex(YA_HEX), unhex(YB_HEX));
    assert_eq!(
        derive_session_id::<Suite>(&ya, AD_A, &yb, AD_B, CpaceMode::InitiatorResponder),
        unhex(SID_OUT_IR_HEX)
    );
    assert_eq!(
        derive_session_id::<Suite>(&ya, AD_A, &yb, AD_B, CpaceMode::Symmetric),
        unhex(SID_OUT_OC_HEX)
    );
}

fn run(mode: CpaceMode, prs: &[u8]) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let (ci, sid) = (unhex(CI_HEX), unhex(SID_HEX));
    let mut rng_a = FixedRng::new(YA_SCALAR_HEX);
    let (ya, state) = CpaceInitiator::<Suite>::start(prs, &ci, &sid, AD_A, &mut rng_a).unwrap();
    let mut rng_b = FixedRng::new(YB_SCALAR_HEX);
    let (yb, out_b) = CpaceResponder::<Suite>::respond(
        &ya,
        &unhex(PRS_HEX),
        &ci,
        &sid,
        AD_A,
        AD_B,
        mode,
        &mut rng_b,
    )
    .unwrap();
    let out_a = state.finish(&yb, AD_B, mode).unwrap();
    (
        ya,
        yb,
        out_a.isk.as_bytes().to_vec(),
        out_b.isk.as_bytes().to_vec(),
    )
}

#[test]
fn end_to_end_shares_and_isk_match_draft() {
    for (mode, isk) in [
        (CpaceMode::InitiatorResponder, ISK_IR_HEX),
        (CpaceMode::Symmetric, ISK_OC_HEX),
    ] {
        let (ya, yb, isk_a, isk_b) = run(mode, &unhex(PRS_HEX));
        assert_eq!(ya, unhex(YA_HEX));
        assert_eq!(yb, unhex(YB_HEX));
        assert_eq!(isk_a, unhex(isk));
        assert_eq!(isk_b, unhex(isk));
    }
}

#[test]
fn wrong_password_gives_different_keys() {
    let (_, _, isk_a, isk_b) = run(CpaceMode::InitiatorResponder, b"Passw0rd");
    assert_ne!(isk_a, isk_b);
    assert_ne!(isk_a, unhex(ISK_IR_HEX));
}

#[test]
fn identity_share_is_rejected() {
    let (ci, sid) = (unhex(CI_HEX), unhex(SID_HEX));
    let identity = [0u8; 32];
    let mut rng = FixedRng::new(YB_SCALAR_HEX);
    assert!(CpaceResponder::<Suite>::respond(
        &identity,
        &unhex(PRS_HEX),
        &ci,
        &sid,
        AD_A,
        AD_B,
        CpaceMode::InitiatorResponder,
        &mut rng
    )
    .is_err());
    let mut rng_a = FixedRng::new(YA_SCALAR_HEX);
    let (_, state) =
        CpaceInitiator::<Suite>::start(&unhex(PRS_HEX), &ci, &sid, AD_A, &mut rng_a).unwrap();
    assert!(state
        .finish(&identity, AD_B, CpaceMode::InitiatorResponder)
        .is_err());
}

#[test]
fn malformed_shares_are_rejected() {
    let (ci, sid) = (unhex(CI_HEX), unhex(SID_HEX));
    for bad in [vec![], vec![1u8; 31], vec![0xffu8; 32], vec![0x01u8; 33]] {
        let mut rng = FixedRng::new(YB_SCALAR_HEX);
        assert!(
            CpaceResponder::<Suite>::respond(
                &bad,
                &unhex(PRS_HEX),
                &ci,
                &sid,
                AD_A,
                AD_B,
                CpaceMode::InitiatorResponder,
                &mut rng
            )
            .is_err(),
            "len {}",
            bad.len()
        );
    }
}
