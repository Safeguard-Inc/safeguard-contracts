//! Typed policy lifecycle events.
//!
//! These events make changes to the compliance configuration itself auditable
//! — they are the difference between "the policy changed" and "we can prove
//! which policy version was active when". `safeguard-audit` consumes them;
//! the event names are part of the stable interface between the polyrepos.
//!
//! Note the boundary: **transfer-level** events (`transfer_approved`,
//! `transfer_blocked`) belong to `safeguard-hooks`, not here.

use soroban_sdk::{contractevent, Address, BytesN, Env};

/// A token was bound to a policy's scope (which tokens a policy governs is
/// itself compliance configuration, so the change must be provable).
#[contractevent]
pub struct TokenBound {
    #[topic]
    pub policy_id: BytesN<32>,
    #[topic]
    pub token: Address,
}

/// Publish a policy↔token binding.
pub fn token_bound(env: &Env, policy_id: &BytesN<32>, token: &Address) {
    TokenBound {
        policy_id: policy_id.clone(),
        token: token.clone(),
    }
    .publish(env);
}

/// A token was unbound from a policy's scope.
#[contractevent]
pub struct TokenUnbound {
    #[topic]
    pub policy_id: BytesN<32>,
    #[topic]
    pub token: Address,
}

/// Publish a policy↔token unbinding.
pub fn token_unbound(env: &Env, policy_id: &BytesN<32>, token: &Address) {
    TokenUnbound {
        policy_id: policy_id.clone(),
        token: token.clone(),
    }
    .publish(env);
}

/// The administrative authority was recorded or replaced.
///
/// Emitted on `initialize` (the genesis admin) and on every real `set_admin`
/// rotation, so audit can reconstruct who held the most powerful role on the
/// contract at any point in time — the same guarantee the authority-set
/// events provide for the lesser roles.
#[contractevent]
pub struct AdminSet {
    #[topic]
    pub admin: Address,
}

/// Publish an admin write or rotation.
pub fn admin_set(env: &Env, admin: &Address) {
    AdminSet {
        admin: admin.clone(),
    }
    .publish(env);
}

/// A new draft version of a policy was registered.
///
/// Registration always creates a *version* — the spec's `policy_created`
/// and `policy_version_created` map to this single event, emitted once per
/// `register_version` call.
#[contractevent]
pub struct PolicyCreated {
    #[topic]
    pub policy_id: BytesN<32>,
    pub version: u32,
    pub config_hash: BytesN<32>,
}

/// One rule was registered as part of a policy version.
///
/// Emitted once per rule in the `register_version` call, so audit can
/// prove which rule set — and each individual rule within it — was in a
/// given version. `rule_type` and `action` are the stable core codes.
#[contractevent]
pub struct RuleRegistered {
    #[topic]
    pub policy_id: BytesN<32>,
    #[topic]
    pub version: u32,
    #[topic]
    pub rule_id: BytesN<32>,
    pub rule_type: u32,
    pub action: u32,
}

/// Publish a rule registration for a version.
pub fn rule_registered(
    env: &Env,
    policy_id: &BytesN<32>,
    version: u32,
    rule_id: &BytesN<32>,
    rule_type: u32,
    action: u32,
) {
    RuleRegistered {
        policy_id: policy_id.clone(),
        version,
        rule_id: rule_id.clone(),
        rule_type,
        action,
    }
    .publish(env);
}

/// A draft version was activated (any previously active version is
/// superseded).
#[contractevent]
pub struct PolicyActivated {
    #[topic]
    pub policy_id: BytesN<32>,
    pub version: u32,
    pub config_hash: BytesN<32>,
}

/// The active version of a policy was deactivated (policy now has no active
/// version until a new one is activated).
#[contractevent]
pub struct PolicyDeactivated {
    #[topic]
    pub policy_id: BytesN<32>,
    pub version: u32,
}

/// An account's identity verification record was written or replaced.
///
/// Part of the `registry_updated` event family: audit consumes these to
/// prove compliance data changes. `status` is an
/// [`safeguard_core::registries::identity::IdentityStatus`] code.
#[contractevent]
pub struct IdentityUpdated {
    #[topic]
    pub account: Address,
    pub status: u32,
    pub attestation_ref: BytesN<32>,
    pub expires_at: u64,
}

/// An account's identity verification record was removed.
#[contractevent]
pub struct IdentityRemoved {
    #[topic]
    pub account: Address,
}

/// Publish an identity record write/replacement.
pub fn identity_updated(env: &Env, account: &Address, record: &crate::storage::IdentityRecord) {
    IdentityUpdated {
        account: account.clone(),
        status: record.status,
        attestation_ref: record.attestation_ref.clone(),
        expires_at: record.expires_at,
    }
    .publish(env);
}

/// Publish an identity record removal.
pub fn identity_removed(env: &Env, account: &Address) {
    IdentityRemoved {
        account: account.clone(),
    }
    .publish(env);
}

/// A subject's sanctions entry was written, replaced or retired.
/// `status` is a [`safeguard_core::registries::sanctions::SanctionsStatus`]
/// code; retirement is an update to `inactive`, never a deletion.
#[contractevent]
pub struct SanctionsEntryUpdated {
    #[topic]
    pub subject_hash: BytesN<32>,
    pub status: u32,
    pub dataset_version: u32,
}

/// Publish a sanctions entry write/replace/retire.
pub fn sanctions_entry_updated(
    env: &Env,
    subject_hash: &BytesN<32>,
    record: &crate::storage::SanctionsEntryRecord,
) {
    SanctionsEntryUpdated {
        subject_hash: subject_hash.clone(),
        status: record.status,
        dataset_version: record.dataset_version,
    }
    .publish(env);
}

/// An account's region classification was written or replaced.
/// `region` is a [`safeguard_core::rules::jurisdiction::RegionStatus`] code.
#[contractevent]
pub struct JurisdictionUpdated {
    #[topic]
    pub account: Address,
    pub region: u32,
}

/// An account's region classification was removed.
#[contractevent]
pub struct JurisdictionCleared {
    #[topic]
    pub account: Address,
}

/// Publish a jurisdiction classification write/replace.
pub fn jurisdiction_updated(env: &Env, account: &Address, region: u32) {
    JurisdictionUpdated {
        account: account.clone(),
        region,
    }
    .publish(env);
}

/// Publish a jurisdiction classification removal.
pub fn jurisdiction_cleared(env: &Env, account: &Address) {
    JurisdictionCleared {
        account: account.clone(),
    }
    .publish(env);
}

/// An address was added to the registry-authority set.
///
/// Together with [`AuthorityRemoved`] this is the `registry_authority_changed`
/// family from the audit event list: audit needs to prove who could write
/// compliance data at any point in time, and role changes are part of that.
#[contractevent]
pub struct AuthorityAdded {
    #[topic]
    pub authority: Address,
}

/// An address was removed from the registry-authority set.
#[contractevent]
pub struct AuthorityRemoved {
    #[topic]
    pub authority: Address,
}

/// An address was added to the policy-authority set (may activate and
/// deactivate policy versions).
///
/// The spec's role model separates *creating* versions (admin) from
/// *activating* them (policy authority); like the registry-authority events,
/// role changes are published so audit can prove who could transition the
/// active policy version at any point in time.
#[contractevent]
pub struct PolicyAuthorityAdded {
    #[topic]
    pub authority: Address,
}

/// An address was removed from the policy-authority set.
#[contractevent]
pub struct PolicyAuthorityRemoved {
    #[topic]
    pub authority: Address,
}

/// Publish a registry-authority addition.
pub fn authority_added(env: &Env, authority: &Address) {
    AuthorityAdded {
        authority: authority.clone(),
    }
    .publish(env);
}

/// Publish a registry-authority removal.
pub fn authority_removed(env: &Env, authority: &Address) {
    AuthorityRemoved {
        authority: authority.clone(),
    }
    .publish(env);
}

/// Publish a policy-authority addition.
pub fn policy_authority_added(env: &Env, authority: &Address) {
    PolicyAuthorityAdded {
        authority: authority.clone(),
    }
    .publish(env);
}

/// Publish a policy-authority removal.
pub fn policy_authority_removed(env: &Env, authority: &Address) {
    PolicyAuthorityRemoved {
        authority: authority.clone(),
    }
    .publish(env);
}
