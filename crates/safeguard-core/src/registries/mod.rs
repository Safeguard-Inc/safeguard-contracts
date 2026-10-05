//! Compliance registry semantics.
//!
//! Registries turn external compliance information (identity/KYC status,
//! sanctions screening, jurisdiction) into deterministic, normalized values
//! the policy engine can consume. This module owns the **semantic enums**
//! with their stable codes and labels; the on-chain storage records live in
//! `safeguard-contract` and reference these codes.
//!
//! The engine itself still consumes resolved facts ([`crate::evaluation`]);
//! registries are the authoritative snapshot layer that produces those facts
//! on-chain where a deployment chooses to maintain them.

pub mod identity;
pub mod sanctions;
