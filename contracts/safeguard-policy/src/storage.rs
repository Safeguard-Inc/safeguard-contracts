//! Contract storage: data keys, persisted records and typed accessors.
//!
//! # Layout
//!
//! **Instance storage** (lives with the contract instance):
//!
//! ```text
//! Admin          → Address
//! Authorities    → Vec<Address>
//! ```
//!
//! **Persistent storage** (ledger entries with explicit TTL extension):
//!
//! ```text
//! Version(VersionKey)   → PolicyVersionRecord
//! ActiveVersion(policy) → u32
//! TokenBindings(policy) → Vec<Address>
//! TokenPolicy(token)    → policy id (reverse index for `is_authorized`)
//! ```
//!
//! All multi-byte ids cross the boundary as [`BytesN::<32>`] so arbitrary
//! (non-UTF-8) ids are representable and serialize deterministically.
//!
//! Accessors are the only code that touches storage, keeping the key layout
//! reviewable in one place.

use soroban_sdk::{contracttype, vec, Address, Bytes, BytesN, Env, IntoVal, TryFromVal, Val, Vec};

use crate::error::ContractError;

/// How many ledgers ahead persistent entries are extended on write/read.
/// 5_000_000 ledgers ≈ 9.5 months at the ~5s Soroban ledger cadence.
pub const TTL_EXTEND_TO: u32 = 5_000_000;
/// Entries are only extended when their remaining TTL is below this.
pub const TTL_THRESHOLD: u32 = 500_000;

/// Composite key for one policy version's record.
#[contracttype]
#[derive(Clone)]
pub struct VersionKey {
    pub policy_id: BytesN<32>,
    pub version: u32,
}

/// Storage key namespace.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// instance: the administrator [`Address`].
    Admin,
    /// instance: additional registry authorities.
    Authorities,
    /// instance: policy authorities (may activate/deactivate versions).
    PolicyAuthorities,
    /// persistent: configuration record of a policy version.
    Version(VersionKey),
    /// persistent: the active version number of a policy.
    ActiveVersion(BytesN<32>),
    /// persistent: marker that a policy id has at least one registered
    /// version, written on first registration. Lets the registry reject
    /// binding tokens to a policy id that was never created.
    PolicyExists(BytesN<32>),
    /// persistent: tokens covered by a policy.
    TokenBindings(BytesN<32>),
    /// persistent: reverse index token → policy, maintained by the registry
    /// so the enforcement wire entry point (`is_authorized`) can resolve
    /// which policy governs a token without enumerating every policy.
    TokenPolicy(Address),
    /// persistent: identity verification record of an account.
    Identity(Address),
    /// persistent: normalized sanctions entry, keyed by subject hash.
    SanctionsEntry(BytesN<32>),
    /// persistent: jurisdiction classification of an account.
    Jurisdiction(Address),
}

/// On-chain rule record (category/action stored by stable core code).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleRecord {
    pub rule_id: BytesN<32>,
    pub rule_type: u32,
    pub action: u32,
}

/// On-chain record of one policy version.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyVersionRecord {
    pub policy_id: BytesN<32>,
    pub version: u32,
    /// [`safeguard_core::version::VersionStatus`] code.
    pub status: u32,
    /// Config hash (sha-256 of the serialized rule set).
    pub config_hash: BytesN<32>,
    pub rules: Vec<RuleRecord>,
}

/// On-chain identity verification record of an account.
///
/// Holds no PII: `attestation_ref` is a reference/hash to the attestation
/// held off-chain by the provider, never the attestation itself.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityRecord {
    /// [`safeguard_core::registries::identity::IdentityStatus`] code.
    pub status: u32,
    /// Reference/hash of the backing attestation (32-byte id width).
    pub attestation_ref: BytesN<32>,
    /// Ledger timestamp at which the attestation expires (0 = never).
    pub expires_at: u64,
}

/// On-chain normalized sanctions entry, keyed by 32-byte subject hash.
///
/// Mirrors `policy-schema/sanctions.schema.json` with `effective_at`
/// converted from RFC 3339 to epoch seconds by the adapter. Entries are
/// never deleted on-chain: retiring one flips its status to inactive.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SanctionsEntryRecord {
    /// Source list identifier (e.g. `OFAC-SDN`), 32-byte id width.
    pub list_id: BytesN<32>,
    /// [`safeguard_core::registries::sanctions::SanctionsStatus`] code.
    pub status: u32,
    /// Monotonic version of the dataset this entry belongs to.
    pub dataset_version: u32,
    /// Epoch seconds the listing became effective (RFC 3339 converted).
    pub effective_at: u64,
    /// Source identifier (adapter/authority), e.g. `ofac`.
    pub source: Bytes,
}

/// Extend the TTL of a persistent key past the next read/write.
fn extend(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

// ---------------------------------------------------------------- instance

/// Whether the contract has been initialized (an admin is set).
pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Admin)
}

pub fn admin(env: &Env) -> Result<Address, ContractError> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(ContractError::NotInitialized)
}

pub fn set_admin(env: &Env, address: &Address) {
    env.storage().instance().set(&DataKey::Admin, address);
}

pub fn authorities(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&DataKey::Authorities)
        .unwrap_or_else(|| vec![env])
}

pub fn set_authorities(env: &Env, addresses: &Vec<Address>) {
    env.storage()
        .instance()
        .set(&DataKey::Authorities, addresses);
}

pub fn policy_authorities(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&DataKey::PolicyAuthorities)
        .unwrap_or_else(|| vec![env])
}

pub fn set_policy_authorities(env: &Env, addresses: &Vec<Address>) {
    env.storage()
        .instance()
        .set(&DataKey::PolicyAuthorities, addresses);
}

// -------------------------------------------------------------- persistent

/// Set a persistent value, extending its TTL in the same call.
fn set_persistent<T: IntoVal<Env, Val>>(env: &Env, key: &DataKey, value: &T) {
    env.storage().persistent().set(key, value);
    extend(env, key);
}

/// Read a persistent value, extending its TTL when present.
fn get_persistent<T: TryFromVal<Env, Val>>(env: &Env, key: &DataKey) -> Option<T> {
    let value: Option<T> = env.storage().persistent().get(key);
    if value.is_some() {
        extend(env, key);
    }
    value
}

pub fn version_record(
    env: &Env,
    policy_id: &BytesN<32>,
    version: u32,
) -> Option<PolicyVersionRecord> {
    get_persistent(
        env,
        &DataKey::Version(VersionKey {
            policy_id: policy_id.clone(),
            version,
        }),
    )
}

pub fn set_version_record(env: &Env, record: &PolicyVersionRecord) {
    set_persistent(
        env,
        &DataKey::Version(VersionKey {
            policy_id: record.policy_id.clone(),
            version: record.version,
        }),
        record,
    );
}

/// Whether any version has ever been registered under this policy id.
///
/// The marker is written on first registration and never removed (policy
/// history is append-only), so it is the authoritative "policy exists"
/// check for the registry layer.
pub fn policy_exists(env: &Env, policy_id: &BytesN<32>) -> bool {
    get_persistent::<bool>(env, &DataKey::PolicyExists(policy_id.clone())).is_some()
}

pub fn set_policy_exists(env: &Env, policy_id: &BytesN<32>) {
    set_persistent(env, &DataKey::PolicyExists(policy_id.clone()), &true);
}

pub fn active_version(env: &Env, policy_id: &BytesN<32>) -> Option<u32> {
    get_persistent(env, &DataKey::ActiveVersion(policy_id.clone()))
}

pub fn set_active_version(env: &Env, policy_id: &BytesN<32>, version: u32) {
    set_persistent(env, &DataKey::ActiveVersion(policy_id.clone()), &version);
}

pub fn clear_active_version(env: &Env, policy_id: &BytesN<32>) {
    env.storage()
        .persistent()
        .remove(&DataKey::ActiveVersion(policy_id.clone()));
}

pub fn token_bindings(env: &Env, policy_id: &BytesN<32>) -> Vec<Address> {
    get_persistent(env, &DataKey::TokenBindings(policy_id.clone())).unwrap_or_else(|| vec![env])
}

pub fn set_token_bindings(env: &Env, policy_id: &BytesN<32>, tokens: &Vec<Address>) {
    set_persistent(env, &DataKey::TokenBindings(policy_id.clone()), tokens);
}

/// The policy a token's binding points at, when one has ever been bound.
///
/// The reverse index is maintained by the registry (`bind_token` writes it,
/// `unbind_token` clears it); `is_authorized` resolves policy coverage for a
/// token through it. Last binding wins when a token is bound to several
/// policies — the enforcement entry point is single-policy by design, and
/// the registry also verifies the resolved policy still binds the token
/// before evaluating.
pub fn token_policy(env: &Env, token: &Address) -> Option<BytesN<32>> {
    get_persistent(env, &DataKey::TokenPolicy(token.clone()))
}

pub fn set_token_policy(env: &Env, token: &Address, policy_id: &BytesN<32>) {
    set_persistent(env, &DataKey::TokenPolicy(token.clone()), policy_id);
}

pub fn clear_token_policy(env: &Env, token: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::TokenPolicy(token.clone()));
}

// ------------------------------------------------------------- registries

pub fn identity_record(env: &Env, account: &Address) -> Option<IdentityRecord> {
    get_persistent(env, &DataKey::Identity(account.clone()))
}

pub fn set_identity_record(env: &Env, account: &Address, record: &IdentityRecord) {
    set_persistent(env, &DataKey::Identity(account.clone()), record);
}

pub fn remove_identity_record(env: &Env, account: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::Identity(account.clone()));
}

pub fn sanctions_entry(env: &Env, subject_hash: &BytesN<32>) -> Option<SanctionsEntryRecord> {
    get_persistent(env, &DataKey::SanctionsEntry(subject_hash.clone()))
}

pub fn set_sanctions_entry(env: &Env, subject_hash: &BytesN<32>, record: &SanctionsEntryRecord) {
    set_persistent(env, &DataKey::SanctionsEntry(subject_hash.clone()), record);
}

pub fn jurisdiction(env: &Env, account: &Address) -> Option<u32> {
    get_persistent(env, &DataKey::Jurisdiction(account.clone()))
}

pub fn set_jurisdiction(env: &Env, account: &Address, region: u32) {
    set_persistent(env, &DataKey::Jurisdiction(account.clone()), &region);
}

pub fn clear_jurisdiction(env: &Env, account: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::Jurisdiction(account.clone()));
}
