//! On-chain compliance registries: identity, sanctions, jurisdiction.
//!
//! These registries store **deterministic snapshots** of external compliance
//! information (see `docs/registries.md`): normalized, versioned and
//! replaceable, never a live central database. The policy↔token bindings
//! live in [`crate::registry`].
//!
//! Roles follow `docs/security.md`: writes require the admin or a registry
//! authority; reads are public so hooks and audit tooling can resolve and
//! verify state. Every mutation publishes a typed event so `safeguard-audit`
//! can prove what changed and when.
//!
//! The engine never consults these registries during its own execution —
//! facts still arrive caller-resolved. What the registries add is an
//! **authoritative on-chain snapshot** that `evaluate` can resolve facts
//! from (and that hooks/audit can verify against), as documented in
//! [`crate::evaluate`].

pub mod identity;
pub mod jurisdiction;
pub mod sanctions;
