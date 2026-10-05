//! The exported [`PolicyContract`] entrypoints.
//!
//! Every public function is a thin, authorization-checked bridge to the
//! functional modules. No policy logic lives here.
//!
//! # Entrypoint inventory
//!
//! * `schema_version` — the decision/serialization schema this contract speaks
//! * `initialize` / role management — see [`crate::admin`]
//! * policy lifecycle — see [`crate::lifecycle`]
//! * token registry — see [`crate::registry`]
//! * `evaluate` — see [`crate::registry`] for request assembly and
//!   [`safeguard-core::evaluator`] for the decision itself

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Vec};

use crate::admin;
use crate::error::ContractError;
use crate::evaluate::{self, EvaluationInput, EvaluationResult};
use crate::lifecycle;
use crate::registries::{identity, jurisdiction, sanctions};
use crate::registry;
use crate::storage::{self, IdentityRecord, PolicyVersionRecord, RuleRecord, SanctionsEntryRecord};

use safeguard_core::decision::Decision;
use safeguard_core::rules::account_status::AccountStatus;

#[contract]
pub struct PolicyContract;

#[contractimpl]
impl PolicyContract {
    /// The version of the policy schema and decision serialization this
    /// contract speaks. Bumped independently of the contract itself (see
    /// `docs/versioning.md`); consumers should gate on it.
    pub fn schema_version(_env: Env) -> u32 {
        1
    }

    // ----------------------------------------------------------- bootstrap

    /// Initialize the contract with an administrator. Callable once.
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        admin::initialize(&env, &admin)
    }

    // ---------------------------------------------------------------- admin

    /// The current administrator (public read).
    pub fn admin(env: Env) -> Result<Address, ContractError> {
        admin::get_admin(&env)
    }

    /// Replace the administrator. Requires the current admin's auth.
    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), ContractError> {
        admin::set_admin(&env, &new_admin)
    }

    /// The current registry authorities (public read).
    pub fn authorities(env: Env) -> Vec<Address> {
        admin::get_authorities(&env)
    }

    /// Add a registry authority. Requires the admin's auth.
    pub fn add_authority(env: Env, authority: Address) -> Result<(), ContractError> {
        admin::add_authority(&env, &authority)
    }

    /// Remove a registry authority. Requires the admin's auth.
    pub fn remove_authority(env: Env, authority: Address) -> Result<(), ContractError> {
        admin::remove_authority(&env, &authority)
    }

    /// The current policy authorities (public read).
    pub fn policy_authorities(env: Env) -> Vec<Address> {
        admin::get_policy_authorities(&env)
    }

    /// Add a policy authority (may activate/deactivate versions).
    /// Requires the admin's auth.
    pub fn add_policy_authority(env: Env, authority: Address) -> Result<(), ContractError> {
        admin::add_policy_authority(&env, &authority)
    }

    /// Remove a policy authority. Requires the admin's auth.
    pub fn remove_policy_authority(env: Env, authority: Address) -> Result<(), ContractError> {
        admin::remove_policy_authority(&env, &authority)
    }

    // ------------------------------------------------------------- lifecycle

    /// Register a new draft version of a policy. Admin only; append-only.
    pub fn register_version(
        env: Env,
        policy_id: BytesN<32>,
        version: u32,
        config_hash: BytesN<32>,
        rules: Vec<RuleRecord>,
    ) -> Result<(), ContractError> {
        lifecycle::register_version(&env, &policy_id, version, &config_hash, &rules)
    }

    /// Activate a draft version, superseding the previous active version.
    /// Admin or policy authority.
    pub fn activate_version(
        env: Env,
        operator: Address,
        policy_id: BytesN<32>,
        version: u32,
    ) -> Result<(), ContractError> {
        lifecycle::activate_version(&env, &operator, &policy_id, version)
    }

    /// Deactivate the active version of a policy. Admin or policy authority.
    pub fn deactivate_version(
        env: Env,
        operator: Address,
        policy_id: BytesN<32>,
        version: u32,
    ) -> Result<(), ContractError> {
        lifecycle::deactivate_version(&env, &operator, &policy_id, version)
    }

    /// The record of a specific policy version (public read).
    pub fn get_version(
        env: Env,
        policy_id: BytesN<32>,
        version: u32,
    ) -> Result<PolicyVersionRecord, ContractError> {
        lifecycle::get_version(&env, &policy_id, version)
    }

    /// The record of the active version of a policy (public read).
    pub fn get_active_version(
        env: Env,
        policy_id: BytesN<32>,
    ) -> Result<PolicyVersionRecord, ContractError> {
        lifecycle::get_active_version(&env, &policy_id)
    }

    // ---------------------------------------------------------------- tokens

    /// Bind a token to a policy. Admin or registry authority. Idempotent.
    pub fn bind_token(
        env: Env,
        operator: Address,
        policy_id: BytesN<32>,
        token: Address,
    ) -> Result<(), ContractError> {
        registry::bind_token(&env, &operator, &policy_id, &token)
    }

    /// Unbind a token from a policy. Admin or registry authority. Idempotent.
    pub fn unbind_token(
        env: Env,
        operator: Address,
        policy_id: BytesN<32>,
        token: Address,
    ) -> Result<(), ContractError> {
        registry::unbind_token(&env, &operator, &policy_id, &token)
    }

    /// The tokens bound to a policy (public read).
    pub fn bound_tokens(env: Env, policy_id: BytesN<32>) -> Vec<Address> {
        registry::bound_tokens(&env, &policy_id)
    }

    // ------------------------------------------------------------ registries

    /// Write (or replace) an account's identity verification record.
    /// Admin or registry authority.
    pub fn set_identity(
        env: Env,
        operator: Address,
        account: Address,
        status: u32,
        attestation_ref: BytesN<32>,
        expires_at: u64,
    ) -> Result<(), ContractError> {
        identity::set_identity(
            &env,
            &operator,
            &account,
            status,
            attestation_ref,
            expires_at,
        )
    }

    /// Remove an account's identity verification record.
    /// Admin or registry authority.
    pub fn remove_identity(
        env: Env,
        operator: Address,
        account: Address,
    ) -> Result<(), ContractError> {
        identity::remove_identity(&env, &operator, &account)
    }

    /// An account's identity verification record (public read).
    pub fn identity(env: Env, account: Address) -> Option<IdentityRecord> {
        identity::identity(&env, &account)
    }

    /// Write (or replace) a normalized sanctions entry for a subject hash.
    /// Admin or registry authority.
    #[allow(clippy::too_many_arguments)]
    pub fn set_sanctions_entry(
        env: Env,
        operator: Address,
        subject_hash: BytesN<32>,
        list_id: BytesN<32>,
        status: u32,
        dataset_version: u32,
        effective_at: u64,
        source: soroban_sdk::Bytes,
    ) -> Result<(), ContractError> {
        sanctions::set_sanctions_entry(
            &env,
            &operator,
            &subject_hash,
            &list_id,
            status,
            dataset_version,
            effective_at,
            &source,
        )
    }

    /// Retire a sanctions entry (flip to inactive, never delete).
    /// Admin or registry authority.
    pub fn retire_sanctions_entry(
        env: Env,
        operator: Address,
        subject_hash: BytesN<32>,
    ) -> Result<(), ContractError> {
        sanctions::retire_sanctions_entry(&env, &operator, &subject_hash)
    }

    /// A subject's sanctions entry (public read).
    pub fn sanctions_entry(env: Env, subject_hash: BytesN<32>) -> Option<SanctionsEntryRecord> {
        sanctions::sanctions_entry(&env, &subject_hash)
    }

    /// Set (or replace) an account's region classification.
    /// Admin or registry authority.
    pub fn set_jurisdiction(
        env: Env,
        operator: Address,
        account: Address,
        region: u32,
    ) -> Result<(), ContractError> {
        jurisdiction::set_jurisdiction(&env, &operator, &account, region)
    }

    /// Remove an account's region classification. Admin or registry
    /// authority.
    pub fn clear_jurisdiction(
        env: Env,
        operator: Address,
        account: Address,
    ) -> Result<(), ContractError> {
        jurisdiction::clear_jurisdiction(&env, &operator, &account)
    }

    /// An account's stored region code (public read).
    pub fn jurisdiction(env: Env, account: Address) -> Option<u32> {
        jurisdiction::jurisdiction(&env, &account)
    }

    // ------------------------------------------------------------ evaluation

    /// Evaluate a subject against the active version of a policy for a token.
    ///
    /// Public read; deterministic; never writes state. The caller supplies
    /// compliance facts (`EvaluationInput`); the contract supplies rule
    /// configuration from the active policy version and returns the decision.
    pub fn evaluate(
        env: Env,
        policy_id: BytesN<32>,
        token: Address,
        input: EvaluationInput,
    ) -> Result<EvaluationResult, ContractError> {
        evaluate::evaluate(&env, &policy_id, &token, &input)
    }

    /// The enforcement wire entry point: is `account` authorized on `token`?
    ///
    /// This is the `is_authorized(account, token) -> bool` seam the
    /// ENFORCE layer (`safeguard-hooks` policy-client) calls. It never
    /// reverts — the answer is always a boolean — and it never returns
    /// `true` unless the decision is an explicit **approve**: a block, a
    /// flag (unknown identity, restricted jurisdiction), an inactive
    /// policy, or an unbound token all answer `false` (fail closed).
    ///
    /// The decision is derived exclusively from this contract's on-chain
    /// state — the wire carries only an account and a token:
    ///
    /// * the policy governing `token` is resolved through the registry's
    ///   reverse index and must be bound and active;
    /// * `account`'s status comes from its identity record, and an account
    ///   with no record is `Unknown` (fail closed); an account whose record
    ///   carries an `expires_at` in the past is likewise `Unknown`: a
    ///   verification with a stated lifetime must not keep approving after
    ///   that lifetime ends. `expires_at == 0` means no expiry — the status
    ///   is permanent until the registry authority rewrites it. (Note the
    ///   ledger timestamp is only available in this env-backed entry point;
    ///   the pure `evaluate` flow resolves expiry off-chain, where the
    ///   caller's clock is authoritative.)
    /// * allowlist/denylist/sanctions membership are caller-supplied
    ///   compliance facts in the richer `evaluate` flow and are treated as
    ///   absent here — a deployment wiring ENFORCE to this contract must
    ///   use a rule set that is decidable from on-chain state (identity
    ///   status, sanctions/jurisdiction registries);
    /// * jurisdiction and sanctions follow the same on-chain registry
    ///   override as `evaluate`.
    pub fn is_authorized(env: Env, account: Address, token: Address) -> bool {
        // Resolve coverage through the registry's reverse index. The index
        // is maintained atomically with the bindings (set on bind, cleared
        // on unbind), so its presence proves the token is bound to the
        // policy it names — no need to re-read the full binding list.
        let Some(policy_id) = storage::token_policy(&env, &token) else {
            return false;
        };
        let Some(active) = storage::active_version(&env, &policy_id) else {
            return false;
        };

        // The account's structural status: an account with no verification
        // record is Unknown, which the evaluator flags (fail closed). A
        // record whose expiry has passed degrades to Unknown as well — an
        // expired verification must not keep approving on the enforcement
        // wire. Zero means "no expiry" (see `set_identity`).
        let account_status = match storage::identity_record(&env, &account) {
            Some(record) if record.expires_at == 0 => record.status,
            Some(record) if record.expires_at > env.ledger().timestamp() => record.status,
            _ => AccountStatus::Unknown.to_code(),
        };

        let input = EvaluationInput {
            account_status,
            allowlist_member: false,
            denylist_matched: false,
            sanctions_matched: false,
            jurisdiction: storage::jurisdiction(&env, &account).unwrap_or(0),
            subject: BytesN::from_array(&env, &[0u8; 32]),
            account: account.clone(),
        };

        match evaluate::evaluate_active(&env, &policy_id, active, &input) {
            // Only an explicit approve authorizes; a flag (e.g. unknown
            // status) is a review outcome, not a pass, on this wire.
            Ok(result) => result.decision == Decision::Approve.to_code(),
            Err(_) => false,
        }
    }
}
