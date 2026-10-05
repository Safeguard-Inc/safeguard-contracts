//! On-chain error codes returned (or panicked with) by the contract.
//!
//! Codes are **stable public API** in the same way as
//! [`safeguard-core`](safeguard_core) reason codes: `safeguard-hooks` and
//! `safeguard-audit` may observe them, so new errors are appended and never
//! renumbered. Codes are non-dense after the completeness audit: removed
//! codes are never reissued to a new variant, so numbers observed on-chain
//! or in audit tooling never shift meaning.

use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    /// The contract was already initialized.
    AlreadyInitialized = 2,
    /// The contract has not been initialized.
    NotInitialized = 3,
    /// No policy with this id exists.
    PolicyNotFound = 4,
    /// No version of this policy exists.
    VersionNotFound = 5,
    /// The version is not a draft and cannot be activated.
    VersionNotDraft = 6,
    /// The supplied rule set is invalid (duplicate category or id).
    InvalidRuleSet = 7,
    /// The policy has no active version.
    PolicyNotActive = 8,
    /// The token is not bound to the policy.
    TokenNotBound = 9,
    /// A version with this policy id and number already exists (append-only).
    VersionExists = 11,
    /// The version exists but is not the active version.
    VersionNotActive = 12,
    /// Registry data carries an unknown status/region code.
    InvalidRegistryData = 13,
    /// The caller is not the admin or a declared registry authority.
    RegistryAuthorityRequired = 14,
    /// The caller is not the admin or a declared policy authority.
    PolicyAuthorityRequired = 15,
}
