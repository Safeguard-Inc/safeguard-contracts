//! Policy lifecycle: version registration, activation, deactivation, queries.
//!
//! The lifecycle is **append-only**: every change to a policy creates a new
//! version; versions never mutate their rule set in place. This module drives
//! the state machine of [`safeguard_core::version::VersionStatus`]:
//!
//! ```text
//! Draft ──activate──▶ Active ──supersede──▶ Superseded
//!    │                    │
//!    └───deactivate───────┘──────────────▶ Disabled
//! ```
//!
//! Registration requires the admin's auth; activating and deactivating
//! require the admin or a policy authority (the spec's Policy Admin / Policy
//! Authority split). Lifecycle events are published for `safeguard-audit`.

use soroban_sdk::{Address, BytesN, Env, Vec};

use crate::admin;
use crate::error::ContractError;
use crate::events::{PolicyActivated, PolicyCreated, PolicyDeactivated};
use crate::storage::{self, PolicyVersionRecord, RuleRecord};

use safeguard_core::policy::RuleSet;
use safeguard_core::rule::{Rule, RuleAction, RuleId, RuleType};
use safeguard_core::version::VersionStatus;

/// Validate a raw rule list into a normalized core rule set.
///
/// Enforces the invariant the engine depends on: at most one rule per
/// category, unique rule ids, known category/action codes.
fn normalize_rules(rules: &Vec<RuleRecord>) -> Result<RuleSet, ContractError> {
    let mut set = RuleSet::empty();
    for record in rules.iter() {
        let rule_type =
            RuleType::from_code(record.rule_type).ok_or(ContractError::InvalidRuleSet)?;
        let action = RuleAction::from_code(record.action).ok_or(ContractError::InvalidRuleSet)?;
        let rule = Rule {
            id: RuleId::from_bytes(record.rule_id.to_array()),
            rule_type,
            action,
        };
        set.insert(rule)
            .map_err(|_| ContractError::InvalidRuleSet)?;
    }
    Ok(set)
}

/// Register a new draft version of a policy. Admin only. Append-only.
pub fn register_version(
    env: &Env,
    policy_id: &BytesN<32>,
    version: u32,
    config_hash: &BytesN<32>,
    rules: &Vec<RuleRecord>,
) -> Result<(), ContractError> {
    admin::require_admin(env)?;

    // Reject a version that already exists so history cannot be rewritten.
    if storage::version_record(env, policy_id, version).is_some() {
        return Err(ContractError::VersionExists);
    }

    // Validate the rule set before persisting anything.
    normalize_rules(rules)?;

    // First registration under this policy id: record the existence marker
    // so the registry layer can reject binding tokens to a policy that was
    // never created (append-only history means this is written exactly
    // once per policy).
    storage::set_policy_exists(env, policy_id);

    storage::set_version_record(
        env,
        &PolicyVersionRecord {
            policy_id: policy_id.clone(),
            version,
            status: VersionStatus::Draft.to_code(),
            config_hash: config_hash.clone(),
            rules: rules.clone(),
        },
    );
    PolicyCreated {
        policy_id: policy_id.clone(),
        version,
        config_hash: config_hash.clone(),
    }
    .publish(env);
    // One event per rule, so audit can prove the exact rule set — each rule
    // within the version — that was registered.
    for record in rules.iter() {
        crate::events::rule_registered(
            env,
            policy_id,
            version,
            &record.rule_id,
            record.rule_type,
            record.action,
        );
    }
    Ok(())
}

/// Activate a draft version, superseding any currently active version.
/// Admin or policy authority.
pub fn activate_version(
    env: &Env,
    operator: &Address,
    policy_id: &BytesN<32>,
    version: u32,
) -> Result<(), ContractError> {
    admin::require_admin_or_policy_authority(env, operator)?;

    let mut record =
        storage::version_record(env, policy_id, version).ok_or(ContractError::VersionNotFound)?;

    if record.status != VersionStatus::Draft.to_code() {
        return Err(ContractError::VersionNotDraft);
    }

    // Supersede the previously active version, if any.
    if let Some(previous) = storage::active_version(env, policy_id) {
        if previous != version {
            if let Some(mut old) = storage::version_record(env, policy_id, previous) {
                old.status = VersionStatus::Superseded.to_code();
                storage::set_version_record(env, &old);
            }
        }
    }

    record.status = VersionStatus::Active.to_code();
    storage::set_version_record(env, &record);
    storage::set_active_version(env, policy_id, version);
    PolicyActivated {
        policy_id: policy_id.clone(),
        version,
        config_hash: record.config_hash.clone(),
    }
    .publish(env);
    Ok(())
}

/// Deactivate a version (only the currently active one may be deactivated).
/// Admin or policy authority.
pub fn deactivate_version(
    env: &Env,
    operator: &Address,
    policy_id: &BytesN<32>,
    version: u32,
) -> Result<(), ContractError> {
    admin::require_admin_or_policy_authority(env, operator)?;

    let mut record =
        storage::version_record(env, policy_id, version).ok_or(ContractError::VersionNotFound)?;

    if record.status != VersionStatus::Active.to_code() {
        return Err(ContractError::VersionNotActive);
    }

    record.status = VersionStatus::Disabled.to_code();
    storage::set_version_record(env, &record);
    if storage::active_version(env, policy_id) == Some(version) {
        storage::clear_active_version(env, policy_id);
    }
    PolicyDeactivated {
        policy_id: policy_id.clone(),
        version,
    }
    .publish(env);
    Ok(())
}

/// The record of a specific version (public read).
pub fn get_version(
    env: &Env,
    policy_id: &BytesN<32>,
    version: u32,
) -> Result<PolicyVersionRecord, ContractError> {
    storage::version_record(env, policy_id, version).ok_or(ContractError::VersionNotFound)
}

/// The record of the active version of a policy (public read).
pub fn get_active_version(
    env: &Env,
    policy_id: &BytesN<32>,
) -> Result<PolicyVersionRecord, ContractError> {
    let version = storage::active_version(env, policy_id).ok_or(ContractError::PolicyNotActive)?;
    storage::version_record(env, policy_id, version).ok_or(ContractError::VersionNotFound)
}
