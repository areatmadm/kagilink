# Windows Rust receiver (D022, D024)

* `tpm-probe/` - read-only TPM probe (`run()` prints a report) and `assess()` (the same facts as
  data). Claims only; nothing persisted.
* `receiver/` - receiver skeleton: D024 trust policy (`decide`), risk-notice gate (`TransferGate`,
  D023) and an in-process loopback that drives the shared Rust core with a simulated phone and a
  synthetic secret. CLI: `kagilink-receiver check` and `kagilink-receiver loopback [--accept]`.

Not implemented yet: BLE (WinRT GATT), entering text into a target window, vault, real secrets,
phone-side verification of TPM claims. The TPM check is a trust signal, not a security boundary, and
KagiLink has no duty to detect malware already on the PC (D023). Synthetic data only.
