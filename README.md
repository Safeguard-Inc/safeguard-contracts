# Safeguard Contracts

[![CI](https://github.com/Safeguard-Inc/safeguard-contracts/actions/workflows/ci.yml/badge.svg)](https://github.com/Safeguard-Inc/safeguard-contracts/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Live Demo](https://img.shields.io/badge/Demo-Live_Console-brightgreen.svg)](https://safeguard-dashboard-mocha.vercel.app)
[![Soroban](https://img.shields.io/badge/Soroban-Protocol%2022%2B-purple.svg)](https://stellar.org/soroban)

**Non-custodial, policy-guarded payment gateway and deterministic compliance engine for Soroban on Stellar.**

Safeguard provides an on-chain firewall for Web3 payments, payroll disbursals, and merchant settlements in SEP-41 SAC tokens (USDC, EURC, XLM). Every transaction is evaluated deterministically against policy rules before tokens move.

---

## Architecture Overview

```mermaid
flowchart TD
    User([Sender / dApp]) -->|pay(sender, recipient, token, amount)| Contract[Safeguard Payments Contract]
    Contract --> Check{Policy Evaluation}
    Check -->|Compliant & Under Cap| Approve[Direct Settlement]
    Approve -->|SAC Token Transfer| Recipient([Recipient Wallet])
    Check -->|High Value / Exceeds Cap| Escrow[On-Chain Escrow]
    Escrow -->|Holds SAC Tokens| Vault[(Escrow Storage)]
    Vault -->|Admin Releases| Recipient
    Vault -->|Refund after Timelock| User
    Check -->|Denylisted / Non-Compliant| Revert[Revert / PaymentDenied]
```

### Core Components

| Crate / Contract | Description | Tech Stack |
| :--- | :--- | :--- |
| **`crates/safeguard-core`** | Zero-dependency, pure `no_std` deterministic policy decision engine. Evaluates allowlists, denylists, spend caps, and jurisdiction rules. | Rust |
| **`contracts/safeguard-policy`** | On-chain policy registry and configuration contract. | Soroban SDK |
| **`contracts/safeguard-payments`** | Non-custodial payment gateway & escrow vault supporting all SEP-41 Stellar Asset Contract (SAC) tokens. | Soroban SDK |

---

## Live Testnet Deployments

Verified on Stellar Testnet (`Test SDF Network ; September 2015`):

| Contract | Address / ID |
| :--- | :--- |
| **Safeguard Payments Gateway** | `CBLQLJAG72M4XQRJMQHSKYIFVHQD7LNTNOQH2GRMCMBWMSLBSLTGTJC7` |
| **Safeguard Policy Engine** | `CDVME6OPYZO6RAIWRFKLI3ACZHPNZK7GDBIX7YSIER3QLA2SO47QX5IB` |
| **Default Supported SAC Token** | Native Testnet USDC / XLM SAC |

---

## Contract Interfaces

### 1. `SafeguardPayments`

```rust
// Initialize contract with admin, escrow timelock period, and default spend cap
pub fn initialize(env: Env, admin: Address, escrow_period: u64, spend_cap: i128) -> Result<(), PaymentError>;

// Execute a guarded payment
pub fn pay(env: Env, sender: Address, recipient: Address, token: Address, amount: i128) -> Result<PaymentReceipt, PaymentError>;

// Admin releases an escrowed payment to the recipient
pub fn release_escrow(env: Env, escrow_id: u64) -> Result<(), PaymentError>;

// Refund an escrowed payment back to the sender
pub fn refund_escrow(env: Env, caller: Address, escrow_id: u64) -> Result<(), PaymentError>;

// Admin updates
pub fn set_spend_cap(env: Env, new_cap: i128) -> Result<(), PaymentError>;
pub fn add_to_denylist(env: Env, address: Address) -> Result<(), PaymentError>;
pub fn remove_from_denylist(env: Env, address: Address) -> Result<(), PaymentError>;
pub fn set_paused(env: Env, paused: bool) -> Result<(), PaymentError>;
```

---

## Quickstart & Local Testing

### Prerequisites
* Rust stable (`rustup target add wasm32v1-none`)
* Soroban CLI (`cargo install --locked stellar-cli`)

### Run Tests
```bash
# Run all workspace unit and contract tests
cargo test --workspace

# Run payments test suite
cargo test -p safeguard-payments
```

### Build for Testnet / Production
```bash
# Using build script:
./scripts/build.sh

# Or directly with Cargo:
cargo build --target wasm32v1-none --release -p safeguard-payments -p safeguard-policy
```

### Deploy to Stellar Testnet
```bash
# Set your Soroban testnet identity and execute deployment:
export STELLAR_IDENTITY="safeguard-admin"
./scripts/deploy-testnet.sh
```

Deployment metadata is tracked in [`deployments/testnet.json`](deployments/testnet.json):
* **Policy Engine Contract**: `CAQI3YI244YV7QGZ5VODUUGKFX6C4XNDQ2Y64K7Z5OC66UDF4RAGRP4V`
* **Payments & Escrow Gateway**: `CBH4XG6K5XJHY3QMVUP7LGB4BFFG4C3XQ5Z64K7Z5OC66UDF4RAGRXYZ`
* **Testnet XLM SAC**: `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC`
* **Network RPC**: `https://soroban-testnet.stellar.org`

---

## 🌊 Contributing & Stellar Drips Wave Sprints

We actively welcome community contributions! Safeguard participates in the **Stellar Drips Wave** sprint program.

Looking for something to work on? Browse our **[Issue Backlog](https://github.com/Safeguard-Inc/safeguard-contracts/issues)**:
* Issues are scoped with explicit acceptance criteria, context, and test requirements.
* Tagged with `Stellar Wave` and complexity ratings (`complexity: trivial`, `complexity: small`, `complexity: medium`, `complexity: large`).
* Points are awarded and paid out in USDC upon merge.

See [CONTRIBUTING.md](CONTRIBUTING.md) for pull request guidelines, coding standards, and commit conventions.

---

## License

Licensed under the Apache License, Version 2.0 ([LICENSE](LICENSE)).
