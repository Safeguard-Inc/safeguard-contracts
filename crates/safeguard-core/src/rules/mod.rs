//! Individual compliance checks that the evaluator composes.
//!
//! Each submodule models one rule category and its source of truth:
//!
//! * [`account_status`] — structural account state (frozen, suspended, …);
//!   always evaluated first, never configured as a rule.
//! * [`allowlist`] — membership in the policy's allowlist.
//! * [`denylist`] — presence in the policy's denylist.
//! * [`sanctions`] — matches against normalized sanctions data.
//! * [`jurisdiction`] — permitted/restricted/prohibited regions.
//!
//! Checks are pure functions over snapshot values: they hold no state and
//! never touch storage or the network, which keeps evaluation deterministic
//! and independently testable.

pub mod account_status;
pub mod allowlist;
pub mod denylist;
pub mod jurisdiction;
pub mod sanctions;
