# Contributing

Read AGENTS.md, docs/DECISIONS.md and docs/IMPLEMENTATION_STATUS.md first.
Use synthetic data. Explain the threat addressed and provide negative-path evidence.
Run cargo fmt, clippy and tests. Platform tests must name OS build, hardware,
privilege level and observed result; record skipped tests as NOT RUN.
Do not introduce cryptographic dependencies before the review in
docs/DEPENDENCY_REVIEW.md. Do not add mock authentication to a production path.
Contributions are under Apache-2.0. Do not publish releases or modify external
infrastructure without the owner's explicit approval.
