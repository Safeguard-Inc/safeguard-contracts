//! On-chain identity verification registry.
//!
//! Stores one verification record per account: the normalized
//! [`IdentityStatus`] code, an attestation reference/hash (never the
//! attestation itself — no PII on-chain) and an expiry. Adapters and
//! KYC/attestation providers push updates through the registry authority
//! role; reads are public.

use soroban_sdk::{Address, BytesN, Env};

use crate::admin;
use crate::error::ContractError;
use crate::storage::{self, IdentityRecord};

use safeguard_core::registries::identity::IdentityStatus;

/// Write (or replace) an account's identity verification record.
///
/// Admin or registry authority. The status code must be a known
/// [`IdentityStatus`]; unknown codes are rejected rather than stored.
pub fn set_identity(
    env: &Env,
    operator: &Address,
    account: &Address,
    status: u32,
    attestation_ref: BytesN<32>,
    expires_at: u64,
) -> Result<(), ContractError> {
    admin::require_admin_or_authority(env, operator)?;

    if IdentityStatus::from_code(status).is_none() {
        return Err(ContractError::InvalidRegistryData);
    }

    let record = IdentityRecord {
        status,
        attestation_ref,
        expires_at,
    };
    storage::set_identity_record(env, account, &record);
    crate::events::identity_updated(env, account, &record);
    Ok(())
}

/// Remove an account's identity verification record.
///
/// Admin or registry authority. Removing is the way to retire an entry;
/// there is no "deleted but active" state.
pub fn remove_identity(
    env: &Env,
    operator: &Address,
    account: &Address,
) -> Result<(), ContractError> {
    admin::require_admin_or_authority(env, operator)?;

    if storage::identity_record(env, account).is_some() {
        storage::remove_identity_record(env, account);
        crate::events::identity_removed(env, account);
    }
    Ok(())
}

/// Read an account's identity verification record (public).
pub fn identity(env: &Env, account: &Address) -> Option<IdentityRecord> {
    storage::identity_record(env, account)
}
