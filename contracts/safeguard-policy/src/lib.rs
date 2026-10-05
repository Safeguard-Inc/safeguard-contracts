//! # safeguard-contract
//!
//! The on-chain policy contract of the Safeguard stack.
//!
//! This crate is deliberately **thin**. All rule semantics and evaluation
//! logic live in [`safeguard-core`]; the contract owns what only a contract
//! can own:
//!
//! * **State** — which policy versions exist, which is active, which tokens a
//!   policy covers, and who administers the contract.
//! * **Authorization** — role checks (`require_auth`) before any state change.
//! * **Boundary translation** — Soroban types in, [`safeguard-core`] snapshot
//!   types out, decisions back into Soroban values.
//! * **Lifecycle events** — `policy_created`, `policy_activated`, … so
//!   `safeguard-audit` can later prove changes to the compliance
//!   configuration itself.
//!
//! # Storage model
//!
//! * instance storage — admin, authorities, and the active-version pointer
//! * persistent storage — per-version configuration and policy↔token bindings
//!
//! See [`storage`] for the data-key layout and each functional module
//! ([`admin`], [`lifecycle`], [`registry`]) for entrypoint behavior.
//!
//! # Determinism and fail-closed behavior
//!
//! Evaluations never perform network calls, never read wall-clock time and
//! never depend on ordering beyond the engine's documented precedence.
//! Missing compliance data produces a conservative (never-approving) outcome;
//! see `safeguard-core` and `docs/security.md`.

#![no_std]

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod test;

mod admin;
mod contract;
mod error;
mod evaluate;
mod events;
mod lifecycle;
mod registries;
mod registry;
mod storage;

pub use contract::{PolicyContract, PolicyContractClient};
