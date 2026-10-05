//! On-chain jurisdiction registry.
//!
//! Maps an account to a normalized region classification (a
//! [`safeguard_core::rules::jurisdiction::RegionStatus`] code). This is the
//! authoritative snapshot `evaluate` resolves jurisdiction from when a
//! deployment maintains it; adapters and geo providers push updates through
//! the registry authority role, reads are public.

use soroban_sdk::{Address, Env};

use crate::admin;
use crate::error::ContractError;
use crate::storage;

use safeguard_core::rules::jurisdiction::RegionStatus;

/// Set (or replace) an account's region classification.
///
/// Admin or registry authority. The region code must be a known
/// [`RegionStatus`]; unknown codes are rejected rather than stored.
pub fn set_jurisdiction(
    env: &Env,
    operator: &Address,
    account: &Address,
    region: u32,
) -> Result<(), ContractError> {
    admin::require_admin_or_authority(env, operator)?;

    if RegionStatus::from_code(region).is_none() {
        return Err(ContractError::InvalidRegistryData);
    }

    storage::set_jurisdiction(env, account, region);
    crate::events::jurisdiction_updated(env, account, region);
    Ok(())
}

/// Remove an account's region classification.
///
/// Admin or registry authority. After removal, `evaluate` falls back to the
/// caller-resolved region (fail-closed: an unknown region never approves).
pub fn clear_jurisdiction(
    env: &Env,
    operator: &Address,
    account: &Address,
) -> Result<(), ContractError> {
    admin::require_admin_or_authority(env, operator)?;

    if storage::jurisdiction(env, account).is_some() {
        storage::clear_jurisdiction(env, account);
        crate::events::jurisdiction_cleared(env, account);
    }
    Ok(())
}

/// Read an account's stored region code (public).
pub fn jurisdiction(env: &Env, account: &Address) -> Option<u32> {
    storage::jurisdiction(env, account)
}
