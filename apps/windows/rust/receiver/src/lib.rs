//! KagiLink Windows receiver skeleton (D022, D023, D024). Synthetic data only.
//!
//! What is here: the receiver trust policy over TPM facts, the risk-notice gate that must be
//! acknowledged before any transfer, and an in-process loopback that drives the shared Rust core
//! (no FFI on Windows). What is NOT here: BLE, entering text into a target window, a vault, real
//! secrets. The TPM check is a trust *signal* for honest setups, never a safety verdict, and
//! KagiLink has no duty to detect malware already on the PC (D023).

#![forbid(unsafe_code)]

use areatm_kagilink_core::{
    kind, os_rng, Clock, Code, Error as CoreError, ReceiverSession, SenderPending, SystemClock,
};
use std::sync::Arc;

#[cfg(windows)]
pub mod ble;

pub use areatm_tpm_probe::{assess, Assessment, Attestation, TpmState};

/// Result of applying the D024 policy to what the TPM stack reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// The TPM reported a hardware-bound key that passed the local checks. Boot state is shown to
    /// the user only; it is not required.
    Proceed { secure_boot: Option<bool> },
    /// A TPM is present but the claim failed or was not produced: the transfer is blocked.
    Block { reason: String },
    /// No usable TPM: accurately labelled RAM-only mode with a strong warning.
    RamOnly { reason: String },
}

/// D024: (a) passed -> proceed; (b) TPM present but claim fails -> block; (c) no usable TPM ->
/// RAM-only. A PC that merely says it has no TPM reaches (c); the policy cannot prevent that.
pub fn decide(a: &Assessment) -> Decision {
    match &a.tpm {
        TpmState::Absent => Decision::RamOnly {
            reason: "no TPM is reported to this program".to_string(),
        },
        TpmState::Unusable(why) => Decision::RamOnly {
            reason: format!("the TPM cannot be used here ({why})"),
        },
        TpmState::Present {
            attestation: Attestation::Produced,
        } => Decision::Proceed {
            secure_boot: a.secure_boot,
        },
        TpmState::Present {
            attestation: Attestation::Failed(why),
        } => Decision::Block {
            reason: format!("the TPM attestation failed ({why})"),
        },
        TpmState::Present {
            attestation: Attestation::NotAttempted,
        } => Decision::Block {
            reason: "the TPM attestation was not attempted".to_string(),
        },
    }
}

/// Mode a permitted transfer runs in. It is shown to the user; it is not a safety verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferMode {
    /// The receiver key is claimed to be TPM-resident.
    HardwareClaim,
    /// No hardware protection; keys live only in memory.
    RamOnly,
}

impl TransferMode {
    /// Value for the unverified `claims` field of the hello message.
    pub fn claim_label(self) -> &'static [u8] {
        match self {
            TransferMode::HardwareClaim => b"mode=tpm-claim",
            TransferMode::RamOnly => b"mode=ram-only",
        }
    }
}

/// Why a transfer is not allowed yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GateError {
    /// Policy blocks transfers on this PC.
    Blocked(String),
    /// The user has not acknowledged the risk notice (D023).
    NotAcknowledged,
}

/// Short risk notice shown before the first transfer (D023; text from docs/RISK_NOTICE.md).
pub const NOTICE_KO: &str = "받는 PC에 키로거나 악성코드가 있으면 비밀번호가 노출될 수 있고, KagiLink는 이를 막지 못합니다.\n일회용 비밀번호만 보내고, 보낸 뒤에는 바꾸세요. 모든 책임은 사용자에게 있습니다.";
/// English version of the short notice.
pub const NOTICE_EN: &str = "If the receiving PC has a keylogger or malware, your password can be exposed and KagiLink cannot stop it.\nSend only one-time passwords and change them afterwards. All responsibility is yours.";
/// Extra warning for RAM-only mode.
pub const RAM_ONLY_WARNING_KO: &str = "경고: 이 PC에서는 하드웨어(TPM) 보호를 쓸 수 없어 RAM-only 모드로 동작합니다. 키는 메모리에만 있고 하드웨어 보호가 없습니다.";
/// English RAM-only warning.
pub const RAM_ONLY_WARNING_EN: &str = "Warning: hardware (TPM) protection is not available on this PC, so KagiLink runs in RAM-only mode. Keys exist only in memory with no hardware protection.";

/// Gate that must be passed before any secret may be received. Acknowledgment is per receiver
/// session and is never persisted.
#[derive(Debug)]
pub struct TransferGate {
    decision: Decision,
    acknowledged: bool,
}

impl TransferGate {
    /// New gate for a decision; nothing is acknowledged yet.
    pub fn new(decision: Decision) -> Self {
        Self {
            decision,
            acknowledged: false,
        }
    }

    /// The decision this gate enforces.
    pub fn decision(&self) -> &Decision {
        &self.decision
    }

    /// Record that the user explicitly acknowledged the risk notice. A blocked PC cannot be
    /// acknowledged into a transfer.
    pub fn acknowledge(&mut self) -> Result<(), GateError> {
        if let Decision::Block { reason } = &self.decision {
            return Err(GateError::Blocked(reason.clone()));
        }
        self.acknowledged = true;
        Ok(())
    }

    /// Whether a transfer may proceed, and in which mode.
    pub fn permit(&self) -> Result<TransferMode, GateError> {
        match &self.decision {
            Decision::Block { reason } => Err(GateError::Blocked(reason.clone())),
            _ if !self.acknowledged => Err(GateError::NotAcknowledged),
            Decision::Proceed { .. } => Ok(TransferMode::HardwareClaim),
            Decision::RamOnly { .. } => Ok(TransferMode::RamOnly),
        }
    }
}

/// Failure of the loopback run.
#[derive(Debug)]
pub enum LoopbackError {
    /// The gate refused.
    Gate(GateError),
    /// The core reported an error.
    Core(CoreError),
}

impl From<GateError> for LoopbackError {
    fn from(e: GateError) -> Self {
        LoopbackError::Gate(e)
    }
}

impl From<CoreError> for LoopbackError {
    fn from(e: CoreError) -> Self {
        LoopbackError::Core(e)
    }
}

/// Outcome of a successful loopback run.
#[derive(Debug, PartialEq, Eq)]
pub struct LoopbackReport {
    /// Mode the transfer ran in.
    pub mode: TransferMode,
    /// Number of plaintext bytes received (the plaintext itself is never returned or printed).
    pub received_len: usize,
    /// Number of fragments the secret was split into.
    pub fragments: usize,
}

/// Drive the shared core end to end in one process: this program plays the receiver and a
/// simulated phone plays the sender. No real BLE and no real secret: `synthetic` is test data.
/// The received plaintext is dropped (and zeroized by the core) without being shown.
pub fn run_loopback(
    gate: &TransferGate,
    synthetic: &[u8],
) -> Result<LoopbackReport, LoopbackError> {
    let mode = gate.permit()?;
    let clock: Arc<dyn Clock> = Arc::new(SystemClock::new());
    let mut rng = os_rng();

    let mut rx =
        ReceiverSession::new(clock.clone(), &mut rng, b"win-receiver", mode.claim_label())?;
    let code = Code::parse(rx.code()?)?;
    let m1 = rx.begin_attempt(&mut rng)?;

    // Simulated phone.
    let (m2, pending) = SenderPending::start(
        clock,
        &mut rng,
        &code,
        &m1,
        b"loopback-phone",
        b"synthetic-login",
        b"mode=simulated",
    )?;
    let m3 = rx.handle_m2(&mut rng, &m2)?;
    let (m4, mut phone) = pending.handle_m3(&m3)?;
    let mut channel = rx.handle_m4(&m4)?;

    // The phone user approves one transfer; the receiver says a safe target exists (simulated).
    phone.approve_one_transfer();
    channel.set_ready_to_receive(true);
    let fragments = phone.seal(kind::SECRET, synthetic, 64)?;
    let count = fragments.len();
    let mut received_len = 0;
    for f in &fragments {
        if let Some(msg) = channel.open_fragment(f)? {
            received_len = msg.plaintext.len();
        }
    }
    channel.close();
    phone.close();
    Ok(LoopbackReport {
        mode,
        received_len,
        fragments: count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assessment(tpm: TpmState) -> Assessment {
        Assessment {
            tpm,
            secure_boot: Some(false),
            elevated: Some(false),
        }
    }

    #[test]
    fn policy_matches_d024() {
        assert_eq!(
            decide(&assessment(TpmState::Present {
                attestation: Attestation::Produced
            })),
            Decision::Proceed {
                secure_boot: Some(false)
            }
        );
        assert!(matches!(
            decide(&assessment(TpmState::Present {
                attestation: Attestation::Failed("x".into())
            })),
            Decision::Block { .. }
        ));
        assert!(matches!(
            decide(&assessment(TpmState::Present {
                attestation: Attestation::NotAttempted
            })),
            Decision::Block { .. }
        ));
        assert!(matches!(
            decide(&assessment(TpmState::Absent)),
            Decision::RamOnly { .. }
        ));
        assert!(matches!(
            decide(&assessment(TpmState::Unusable("tpm 1.2".into()))),
            Decision::RamOnly { .. }
        ));
    }

    #[test]
    fn boot_state_is_display_only() {
        for sb in [Some(true), Some(false), None] {
            let mut a = assessment(TpmState::Present {
                attestation: Attestation::Produced,
            });
            a.secure_boot = sb;
            assert_eq!(decide(&a), Decision::Proceed { secure_boot: sb });
        }
    }

    #[test]
    fn gate_requires_acknowledgment() {
        let mut g = TransferGate::new(Decision::Proceed { secure_boot: None });
        assert_eq!(g.permit(), Err(GateError::NotAcknowledged));
        g.acknowledge().unwrap();
        assert_eq!(g.permit(), Ok(TransferMode::HardwareClaim));
    }

    #[test]
    fn ram_only_also_requires_acknowledgment_and_is_labelled() {
        let mut g = TransferGate::new(Decision::RamOnly {
            reason: "no tpm".into(),
        });
        assert_eq!(g.permit(), Err(GateError::NotAcknowledged));
        g.acknowledge().unwrap();
        let mode = g.permit().unwrap();
        assert_eq!(mode, TransferMode::RamOnly);
        assert_eq!(mode.claim_label(), b"mode=ram-only");
    }

    #[test]
    fn blocked_pc_cannot_be_acknowledged_into_a_transfer() {
        let mut g = TransferGate::new(Decision::Block {
            reason: "bad".into(),
        });
        assert!(matches!(g.acknowledge(), Err(GateError::Blocked(_))));
        assert!(matches!(g.permit(), Err(GateError::Blocked(_))));
        assert!(matches!(
            run_loopback(&g, b"synthetic"),
            Err(LoopbackError::Gate(GateError::Blocked(_)))
        ));
    }

    #[test]
    fn loopback_refuses_without_acknowledgment() {
        let g = TransferGate::new(Decision::Proceed { secure_boot: None });
        assert!(matches!(
            run_loopback(&g, b"synthetic"),
            Err(LoopbackError::Gate(GateError::NotAcknowledged))
        ));
    }

    #[test]
    fn loopback_round_trip_in_both_modes() {
        let secret = b"synthetic-secret-for-the-loopback-test-0123456789";
        for decision in [
            Decision::Proceed {
                secure_boot: Some(true),
            },
            Decision::RamOnly {
                reason: "test".into(),
            },
        ] {
            let mut g = TransferGate::new(decision);
            g.acknowledge().unwrap();
            let report = run_loopback(&g, secret).unwrap();
            assert_eq!(report.received_len, secret.len());
            assert!(report.fragments > 1);
        }
    }
}
