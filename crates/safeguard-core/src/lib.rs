//! # safeguard-core
//!
//! Deterministic compliance policy engine for Stellar Confidential Tokens.
//!
//! This crate is the **definition layer** of the Safeguard polyrepo family:
//! it decides what compliance rules apply to an account, token and
//! jurisdiction — it never enforces a transfer. Enforcement belongs to
//! `safeguard-hooks`; proving what happened belongs to `safeguard-audit`.
//!
//! Design constraints:
//!
//! * **`no_std`** — the crate compiles into the Soroban contract artifact, so
//!   it must not depend on the host `std`.
//! * **Deterministic** — identical input plus identical policy state must
//!   always produce the identical decision. No randomness, no wall-clock
//!   time, no hidden state, no network access.
//! * **Dependency-free** — the engine has zero external dependencies so the
//!   evaluation path stays auditable and fast to test.
//! * **Pure** — the crate holds no storage. Storage, registries and
//!   authorization live in the contract; callers snapshot the state they need
//!   and hand it to the engine.
//!
//! # Evaluation model
//!
//! Every evaluation resolves to one of [`Decision`]::[`Approve`](Decision::Approve),
//! [`Decision`]::[`Block`](Decision::Block) or [`Decision`]::[`Flag`](Decision::Flag)
//! together with a machine-readable [`ReasonCode`] and, when a rule produced
//! the outcome, the matching rule reference.
//!
//! See [`evaluator`] for the precedence-ordered engine, [`rule`] for the rule
//! model, [`version`] for policy versioning and [`policy`] for the policy
//! configuration carried into evaluation.
//!
#![no_std]

#[cfg(test)]
extern crate std;

pub mod decision;
pub mod evaluation;
pub mod evaluator;
pub mod policy;
pub mod registries;
pub mod rule;
pub mod rules;
pub mod version;
