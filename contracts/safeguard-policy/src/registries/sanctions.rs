//! On-chain sanctions screening registry.
//!
//! Stores normalized sanctions entries keyed by **32-byte subject hash** — no
//! raw identifiers or PII ever reach the ledger (see
//! `policy-schema/sanctions.schema.json` and `docs/security.md`). Entries are
//! never deleted: retiring one flips its status to inactive so audit history
//! stays readable. Adapters push dataset updates through the registry
//! authority role; reads are public so hooks and audit can screen and verify.

use soroban_sdk::{Address, Bytes, BytesN, Env};

use crate::admin;
use crate::error::ContractError;
use crate::storage::{self, SanctionsEntryRecord};

use safeguard_core::registries::sanctions::SanctionsStatus;

/// Write (or replace) a normalized sanctions entry for a subject hash.
///
/// Admin or registry authority. The status must be a known
/// [`SanctionsStatus`] code and the dataset version must be >= 1 (matching
/// the schema); anything else is rejected as invalid registry data.
#[allow(clippy::too_many_arguments)]
pub fn set_sanctions_entry(
    env: &Env,
    operator: &Address,
    subject_hash: &BytesN<32>,
    list_id: &BytesN<32>,
    status: u32,
    dataset_version: u32,
    effective_at: u64,
    source: &Bytes,
) -> Result<(), ContractError> {
    admin::require_admin_or_authority(env, operator)?;

    if SanctionsStatus::from_code(status).is_none() {
        return Err(ContractError::InvalidRegistryData);
    }
    if dataset_version == 0 {
        return Err(ContractError::InvalidRegistryData);
    }

    let record = SanctionsEntryRecord {
        list_id: list_id.clone(),
        status,
        dataset_version,
        effective_at,
        source: source.clone(),
    };
    storage::set_sanctions_entry(env, subject_hash, &record);
    crate::events::sanctions_entry_updated(env, subject_hash, &record);
    Ok(())
}

/// Retire a sanctions entry: flip it to inactive without deleting it.
///
/// Admin or registry authority. Idempotent; a missing entry is a no-op.
/// Retirement is how dataset corrections are recorded so the ledger keeps
/// the history (the entry stops screening subjects).
pub fn retire_sanctions_entry(
    env: &Env,
    operator: &Address,
    subject_hash: &BytesN<32>,
) -> Result<(), ContractError> {
    admin::require_admin_or_authority(env, operator)?;

    let Some(mut record) = storage::sanctions_entry(env, subject_hash) else {
        return Ok(());
    };
    record.status = SanctionsStatus::Inactive.to_code();
    storage::set_sanctions_entry(env, subject_hash, &record);
    crate::events::sanctions_entry_updated(env, subject_hash, &record);
    Ok(())
}

/// Read a subject's sanctions entry (public).
pub fn sanctions_entry(env: &Env, subject_hash: &BytesN<32>) -> Option<SanctionsEntryRecord> {
    storage::sanctions_entry(env, subject_hash)
}
