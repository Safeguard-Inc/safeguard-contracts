//! Role-based administration of the policy contract.
//!
//! Roles follow `docs/security.md`:
//!
//! * **Admin** — bootstrap, change the admin, manage both authority sets,
//!   and create policy versions.
//! * **Policy authority** — activates and deactivates policy versions
//!   (the spec's separation of *creating* a version from *promoting* it to
//!   active, so no single role can both write rules and ship them); added
//!   and removed only by the admin.
//! * **Registry authority** — manages the policy↔token registry and the
//!   compliance registries; added and removed only by the admin.
//! * **Everyone else** — read-only (queries) or subject to evaluation.
//!
//! Every state-changing function authenticates the acting address with
//! `require_auth`; no operation is callable by an anonymous caller.

use soroban_sdk::{vec, Address, Env, Vec};

use crate::error::ContractError;
use crate::storage;

/// Initialize the contract with an admin. Fails if already initialized.
///
/// Publishes an [`crate::events::AdminSet`] event naming the genesis admin,
/// so audit can prove who held the role from the first block.
pub fn initialize(env: &Env, admin: &Address) -> Result<(), ContractError> {
    if storage::is_initialized(env) {
        return Err(ContractError::AlreadyInitialized);
    }
    admin.require_auth();
    storage::set_admin(env, admin);
    storage::set_authorities(env, &vec![env]);
    crate::events::admin_set(env, admin);
    Ok(())
}

/// Read the current admin (public read; auditors may query state).
pub fn get_admin(env: &Env) -> Result<Address, ContractError> {
    storage::admin(env)
}

/// Replace the admin. Only the current admin may do this.
///
/// Publishes an [`crate::events::AdminSet`] event naming the successor on a
/// real rotation; re-setting the same admin is a no-op that emits nothing
/// (events describe state changes).
pub fn set_admin(env: &Env, new_admin: &Address) -> Result<(), ContractError> {
    let current = storage::admin(env)?;
    current.require_auth();
    new_admin.require_auth();
    if &current != new_admin {
        storage::set_admin(env, new_admin);
        crate::events::admin_set(env, new_admin);
    }
    Ok(())
}

/// The current registry authorities.
pub fn get_authorities(env: &Env) -> Vec<Address> {
    storage::authorities(env)
}

/// Add a registry authority. Only the admin may do this.
///
/// Publishes an [`crate::events::AuthorityAdded`] event when the set
/// actually changes, so audit can prove who held the role when.
pub fn add_authority(env: &Env, authority: &Address) -> Result<(), ContractError> {
    let current = storage::admin(env)?;
    current.require_auth();

    let mut list = storage::authorities(env);
    if !list.contains(authority) {
        list.push_back(authority.clone());
        storage::set_authorities(env, &list);
        crate::events::authority_added(env, authority);
    }
    Ok(())
}

/// Remove a registry authority. Only the admin may do this.
///
/// Publishes an [`crate::events::AuthorityRemoved`] event when an address
/// was actually removed.
pub fn remove_authority(env: &Env, authority: &Address) -> Result<(), ContractError> {
    let current = storage::admin(env)?;
    current.require_auth();

    let list = storage::authorities(env);
    let mut filtered: Vec<Address> = vec![env];
    let mut removed = false;
    for entry in list.iter() {
        if &entry != authority {
            filtered.push_back(entry);
        } else {
            removed = true;
        }
    }
    if removed {
        storage::set_authorities(env, &filtered);
        crate::events::authority_removed(env, authority);
    }
    Ok(())
}

/// Whether an address is the admin.
pub fn is_admin(env: &Env, address: &Address) -> bool {
    storage::admin(env)
        .map(|admin| &admin == address)
        .unwrap_or(false)
}

/// Whether an address is the admin or a registry authority.
pub fn is_admin_or_authority(env: &Env, address: &Address) -> bool {
    if is_admin(env, address) {
        return true;
    }
    storage::authorities(env).contains(address)
}

/// The current policy authorities.
pub fn get_policy_authorities(env: &Env) -> Vec<Address> {
    storage::policy_authorities(env)
}

/// Add a policy authority. Only the admin may do this.
///
/// Publishes a [`crate::events::PolicyAuthorityAdded`] event when the set
/// actually changes.
pub fn add_policy_authority(env: &Env, authority: &Address) -> Result<(), ContractError> {
    let current = storage::admin(env)?;
    current.require_auth();

    let mut list = storage::policy_authorities(env);
    if !list.contains(authority) {
        list.push_back(authority.clone());
        storage::set_policy_authorities(env, &list);
        crate::events::policy_authority_added(env, authority);
    }
    Ok(())
}

/// Remove a policy authority. Only the admin may do this.
///
/// Publishes a [`crate::events::PolicyAuthorityRemoved`] event when an
/// address was actually removed.
pub fn remove_policy_authority(env: &Env, authority: &Address) -> Result<(), ContractError> {
    let current = storage::admin(env)?;
    current.require_auth();

    let list = storage::policy_authorities(env);
    let mut filtered: Vec<Address> = vec![env];
    let mut removed = false;
    for entry in list.iter() {
        if &entry != authority {
            filtered.push_back(entry);
        } else {
            removed = true;
        }
    }
    if removed {
        storage::set_policy_authorities(env, &filtered);
        crate::events::policy_authority_removed(env, authority);
    }
    Ok(())
}

/// Whether an address is the admin or a policy authority.
pub fn is_admin_or_policy_authority(env: &Env, address: &Address) -> bool {
    if is_admin(env, address) {
        return true;
    }
    storage::policy_authorities(env).contains(address)
}

/// Authenticate an address that must be the admin or a policy authority.
///
/// Mirrors [`require_admin_or_authority`]: membership is checked before
/// `require_auth`, so a non-member cannot even attempt authorization.
pub fn require_admin_or_policy_authority(
    env: &Env,
    declared: &Address,
) -> Result<Address, ContractError> {
    if !is_admin_or_policy_authority(env, declared) {
        return Err(ContractError::PolicyAuthorityRequired);
    }
    declared.require_auth();
    Ok(declared.clone())
}

/// Authenticate an address that must be the admin.
pub fn require_admin(env: &Env) -> Result<Address, ContractError> {
    let current = storage::admin(env)?;
    current.require_auth();
    Ok(current)
}

/// Authenticate an address that must be the admin or a registry authority.
///
/// The caller declares who they are; the contract verifies membership and
/// requires that the declared address authorized the call.
pub fn require_admin_or_authority(env: &Env, declared: &Address) -> Result<Address, ContractError> {
    if !is_admin_or_authority(env, declared) {
        return Err(ContractError::RegistryAuthorityRequired);
    }
    declared.require_auth();
    Ok(declared.clone())
}
