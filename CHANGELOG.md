# Changelog

All notable changes to the `safeguard-contracts` repository will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-11-03

### Added
- **Soroban Protocol 22+ Architecture**: Implemented full compatibility with Soroban SDK Protocol 22+ with zero unsafe blocks.
- **Timelocked Emergency Circuit Breaker**: Added `emergency_pause_with_duration` and `get_pause_status` entrypoints enabling guardians to enforce automatic timelocked operational pauses.
- **Role Authority Multi-party Delegation**: Implemented `batch_has_role_authority`, `add_policy_authority`, and two-step admin transfer workflows.
- **Dynamic Per-Token Spending Caps**: Added custom threshold routing per token (`set_token_spend_cap`, `get_token_spend_cap`, `remove_token_spend_cap`) diverting transfers to on-chain escrow.
- **Comprehensive Escrow Vault Engine**:
  - Partial escrow release (`release_escrow_partial`).
  - Dispute filing and multi-party arbitration (`dispute_escrow`, `arbitrate_escrow`, `extend_dispute_period`).
  - Atomic batch escrow release, refund, and dispute entrypoints (`batch_release_escrows`, `batch_refund_escrows`, `batch_dispute_escrows`).
  - Paged escrow queries (`get_active_escrows_paged`, `get_escrow_ids_by_status_paged`).
- **Deterministic Consensus Telemetry**: Added `get_gateway_telemetry_snapshot` and `get_payment_sequence_details` aggregating sequence counters, escrow reserves, and circuit breaker status.
- **Canonical 270 Error Codes Catalog**: Synchronized `safeguard-core` with 270 discrete error codes across Identity, Sanctions, Jurisdiction, Velocity, and Escrow domains.
- **Automated Ledger TTL Extensions**: Added automated storage rent extension for instance and persistent entries preserving on-chain contracts across network epochs.

### Changed
- Refactored `pay()` entrypoint to execute atomic SEP-41 SAC token transfers fail-closed on any policy violation.
- Normalized Soroban error handling to pin exact stable numeric error codes.

### Security
- Fail-closed denylist screening with zero state changes on reverts.
- Automated guardian circuit breakers with timelock expiration preventing permanent halts.
- Safe cross-contract verification against `safeguard-policy` registry.
