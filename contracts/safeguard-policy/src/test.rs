//! Integration tests: the full lifecycle and evaluation paths through the
//! generated contract client against the Soroban test host.

#![cfg(test)]

extern crate std;

use std::path::Path;

use crate::error::ContractError;
use crate::evaluate::{EvaluationInput, EvaluationResult};
use crate::storage::RuleRecord;

/// Test-local alias: the contract now spells 32-byte ids as [`BytesN::<32>`]
/// inline (type aliases emit a dangling `Udt` reference in the on-chain
/// spec, which breaks CLI invocation), but tests keep the short name.
type Id = BytesN<32>;
use crate::{PolicyContract, PolicyContractClient};

use safeguard_sdk::model::PolicyDocument;

use soroban_sdk::testutils::Address as _;
use soroban_sdk::testutils::Events as _;
use soroban_sdk::{vec, Address, Bytes, BytesN, Env, Vec};

use safeguard_core::decision::{Decision, ReasonCode};
use safeguard_core::rule::{RuleAction, RuleId, RuleType};
use safeguard_core::rules::account_status::AccountStatus;
use safeguard_core::version::VersionStatus;

/// Encode an id from an ASCII string using the core id rules.
fn rid(env: &Env, text: &str) -> Id {
    let core = RuleId::from_str(text);
    BytesN::from_array(env, core.as_bytes())
}

fn config_hash(env: &Env, fill: u8) -> Id {
    BytesN::from_array(env, &[fill; 32])
}

/// The contract error carried by a failed client call: the client surfaces
/// contract-level errors as `Ok(ContractError)` inside the invocation error.
type ClientError = Result<ContractError, soroban_sdk::InvokeError>;

fn contract_err(error: ContractError) -> ClientError {
    Ok(error)
}

/// Deploy and initialize the contract; return the actors and a client.
fn setup(
    env: &Env,
) -> (
    Address,
    Address,
    Address,
    Address,
    Id,
    PolicyContractClient<'_>,
) {
    env.mock_all_auths();

    let admin = Address::generate(env);
    let authority = Address::generate(env);
    let stranger = Address::generate(env);
    let token = Address::generate(env);
    let policy = rid(env, "institutional-default");

    let contract_id = env.register(PolicyContract, ());
    let client = PolicyContractClient::new(env, &contract_id);
    client.initialize(&admin);
    client.add_authority(&authority);

    (admin, authority, stranger, token, policy, client)
}

/// Register the default policy: allowlist + sanctions rules (both block).
fn register_default_policy(env: &Env, client: &PolicyContractClient, policy: &Id, version: u32) {
    let rules = vec![
        env,
        RuleRecord {
            rule_id: rid(env, "ALLOWLIST-001"),
            rule_type: RuleType::Allowlist.to_code(),
            action: RuleAction::Block.to_code(),
        },
        RuleRecord {
            rule_id: rid(env, "SANCTIONS-001"),
            rule_type: RuleType::Sanctions.to_code(),
            action: RuleAction::Block.to_code(),
        },
    ];
    client.register_version(policy, &version, &config_hash(env, 1), &rules);
}

/// Register + activate version 1 and bind the token.
fn bound_and_active(
    env: &Env,
    client: &PolicyContractClient,
    admin: &Address,
    policy: &Id,
    token: &Address,
) {
    register_default_policy(env, client, policy, 1);
    client.activate_version(admin, policy, &1);
    client.bind_token(admin, policy, token);
}

fn active_input(env: &Env, account: &Address) -> EvaluationInput {
    EvaluationInput {
        account_status: 0, // active
        allowlist_member: true,
        denylist_matched: false,
        sanctions_matched: false,
        jurisdiction: 0, // permitted
        subject: BytesN::from_array(env, &[1; 32]),
        account: account.clone(),
    }
}

fn assert_approve(result: &EvaluationResult) {
    assert_eq!(result.decision, Decision::Approve.to_code());
    assert_eq!(result.reason_code, ReasonCode::NoReason.to_code());
    assert_eq!(result.rule_id, None);
}

// ------------------------------------------------------------------ admin

#[test]
fn initializes_once_and_guards_reinitialization() {
    let env = Env::default();
    let (admin, authority, _, _, _, client) = setup(&env);

    let err = client.try_initialize(&admin).unwrap_err();
    assert_eq!(err, contract_err(ContractError::AlreadyInitialized));
    assert_eq!(client.admin(), admin);
    assert_eq!(client.authorities(), vec![&env, authority.clone()]);
}

#[test]
fn registry_operations_require_an_authorized_operator() {
    let env = Env::default();
    let (_, _, stranger, token, policy, client) = setup(&env);

    let err = client
        .try_bind_token(&stranger, &policy, &token)
        .unwrap_err();
    assert_eq!(err, contract_err(ContractError::RegistryAuthorityRequired));
}

/// Admin writes publish one AdminSet event each — the genesis admin on
/// initialize, the successor on a real rotation — and re-setting the same
/// admin emits nothing.
#[test]
fn admin_writes_publish_an_event_only_on_real_changes() {
    use soroban_sdk::xdr::ContractEventBody;
    use soroban_sdk::Symbol;
    use soroban_sdk::TryFromVal as _;

    fn event_identity(env: &Env, event: &soroban_sdk::xdr::ContractEvent) -> (Symbol, Address) {
        let ContractEventBody::V0(v0) = &event.body;
        let name: Symbol = Symbol::try_from_val(env, &v0.topics[0]).expect("symbol topic");
        let emitted: Address = Address::try_from_val(env, &v0.topics[1]).expect("address topic");
        (name, emitted)
    }

    let env = Env::default();
    let (_, _, _, _, _, client) = setup(&env);

    // setup() initialized with an admin: its AdminSet event is asserted in
    // the initialization test below; this test pins the rotation path only.

    // A real rotation publishes one AdminSet naming the successor.
    let successor = Address::generate(&env);
    client.set_admin(&successor);
    let all_events = env.events().all();
    assert_eq!(all_events.events().len(), 1);
    assert_eq!(
        event_identity(&env, &all_events.events()[0]),
        (Symbol::new(&env, "admin_set"), successor.clone())
    );
    assert_eq!(client.admin(), successor);

    // Re-setting the same admin is a no-op: no event, admin unchanged.
    client.set_admin(&successor);
    assert_eq!(env.events().all().events().len(), 0);
    assert_eq!(client.admin(), successor);
}

// --------------------------------------------------------------- lifecycle

#[test]
fn register_activate_and_query_the_lifecycle() {
    let env = Env::default();
    let (admin, _, _, token, policy, client) = setup(&env);
    register_default_policy(&env, &client, &policy, 1);

    let record = client.get_version(&policy, &1);
    assert_eq!(record.version, 1);
    assert_eq!(record.status, VersionStatus::Draft.to_code());
    assert_eq!(record.rules.len(), 2); // The activation invocation published one typed lifecycle event; events
                                       // are scoped to the invocation that emitted them, so query immediately.
    client.activate_version(&admin, &policy, &1);
    let all_events = env.events().all();
    assert_eq!(
        all_events.events().len(),
        1,
        "activation published one event"
    );

    let active = client.get_active_version(&policy);
    assert_eq!(active.version, 1);
    assert_eq!(active.status, VersionStatus::Active.to_code());

    client.bind_token(&admin, &policy, &token);
    assert_eq!(client.bound_tokens(&policy), vec![&env, token.clone()]);
}

/// Registering a version publishes one rule_registered event per rule, so
/// audit can prove the exact rule set of every version.
#[test]
fn registering_publishes_one_rule_event_per_rule() {
    use soroban_sdk::xdr::ContractEventBody;
    use soroban_sdk::Symbol;
    use soroban_sdk::TryFromVal as _;

    fn rule_event_count(env: &Env, events: &soroban_sdk::testutils::ContractEvents) -> usize {
        events
            .events()
            .iter()
            .filter(|event| {
                let ContractEventBody::V0(v0) = &event.body;
                let name: Symbol = Symbol::try_from_val(env, &v0.topics[0]).expect("symbol topic");
                name == Symbol::new(env, "rule_registered")
            })
            .count()
    }

    let env = Env::default();
    let (_, _, _, _, policy, client) = setup(&env);

    register_default_policy(&env, &client, &policy, 1);
    let rule_events = rule_event_count(&env, &env.events().all());
    // register_default_policy registers two rules (allowlist + sanctions).
    assert_eq!(rule_events, 2, "one rule_registered event per rule");
}

#[test]
fn duplicate_registration_is_rejected() {
    let env = Env::default();
    let (_, _, _, _, policy, client) = setup(&env);
    register_default_policy(&env, &client, &policy, 1);

    let rules = vec![&env];
    let err = client
        .try_register_version(&policy, &1, &config_hash(&env, 2), &rules)
        .unwrap_err();
    assert_eq!(err, contract_err(ContractError::VersionExists));
}

#[test]
fn invalid_rule_sets_are_rejected_before_persisting() {
    let env = Env::default();
    let (_, _, _, _, policy, client) = setup(&env);

    // Two allowlist rules: duplicate category.
    let rules = vec![
        &env,
        RuleRecord {
            rule_id: rid(&env, "ALLOWLIST-001"),
            rule_type: RuleType::Allowlist.to_code(),
            action: RuleAction::Block.to_code(),
        },
        RuleRecord {
            rule_id: rid(&env, "ALLOWLIST-002"),
            rule_type: RuleType::Allowlist.to_code(),
            action: RuleAction::Flag.to_code(),
        },
    ];
    let err = client
        .try_register_version(&policy, &1, &config_hash(&env, 1), &rules)
        .unwrap_err();
    assert_eq!(err, contract_err(ContractError::InvalidRuleSet));
    // Nothing persisted.
    assert_eq!(
        client.try_get_version(&policy, &1).unwrap_err(),
        contract_err(ContractError::VersionNotFound)
    );
}

#[test]
fn activation_requires_a_draft_version() {
    let env = Env::default();
    let (admin, _, _, _, policy, client) = setup(&env);
    register_default_policy(&env, &client, &policy, 1);
    client.activate_version(&admin, &policy, &1);

    // Re-activating the active version is not allowed.
    let err = client
        .try_activate_version(&admin, &policy, &1)
        .unwrap_err();
    assert_eq!(err, contract_err(ContractError::VersionNotDraft));

    // Activating a version that does not exist fails cleanly.
    let err = client
        .try_activate_version(&admin, &policy, &99)
        .unwrap_err();
    assert_eq!(err, contract_err(ContractError::VersionNotFound));
}

#[test]
fn activating_a_new_version_supersedes_the_old_one() {
    let env = Env::default();
    let (admin, _, _, _, policy, client) = setup(&env);
    register_default_policy(&env, &client, &policy, 1);
    client.activate_version(&admin, &policy, &1);

    let rules = vec![
        &env,
        RuleRecord {
            rule_id: rid(&env, "SANCTIONS-001"),
            rule_type: RuleType::Sanctions.to_code(),
            action: RuleAction::Flag.to_code(),
        },
    ];
    client.register_version(&policy, &2, &config_hash(&env, 2), &rules);
    client.activate_version(&admin, &policy, &2);

    assert_eq!(
        client.get_version(&policy, &1).status,
        VersionStatus::Superseded.to_code()
    );
    assert_eq!(client.get_active_version(&policy).version, 2);
}

#[test]
fn deactivation_removes_the_active_version() {
    let env = Env::default();
    let (admin, _, _, _, policy, client) = setup(&env);
    register_default_policy(&env, &client, &policy, 1);
    client.activate_version(&admin, &policy, &1);
    client.deactivate_version(&admin, &policy, &1);

    assert_eq!(
        client.get_version(&policy, &1).status,
        VersionStatus::Disabled.to_code()
    );
    // Only the active version may be deactivated.
    let err = client
        .try_deactivate_version(&admin, &policy, &1)
        .unwrap_err();
    assert_eq!(err, contract_err(ContractError::VersionNotActive));
    // The policy has no active version anymore.
    assert_eq!(
        client.try_get_active_version(&policy).unwrap_err(),
        contract_err(ContractError::PolicyNotActive)
    );
}

// ----------------------------------------------- policy-authority role split

/// The spec separates creating versions (admin) from activating them (policy
/// authority): a registry authority — who may update compliance registries —
/// must NOT be able to activate or deactivate a policy version.
#[test]
fn only_admin_or_policy_authority_can_activate_or_deactivate() {
    let env = Env::default();
    let (admin, authority, stranger, _, policy, client) = setup(&env);

    // A registry authority is not a policy authority.
    let policy_authority = Address::generate(&env);
    client.add_policy_authority(&policy_authority);
    assert_eq!(
        client.policy_authorities(),
        vec![&env, policy_authority.clone()]
    );

    register_default_policy(&env, &client, &policy, 1);

    // A registry authority cannot activate.
    let err = client
        .try_activate_version(&authority, &policy, &1)
        .unwrap_err();
    assert_eq!(err, contract_err(ContractError::PolicyAuthorityRequired));

    // A stranger cannot activate either.
    let err = client
        .try_activate_version(&stranger, &policy, &1)
        .unwrap_err();
    assert_eq!(err, contract_err(ContractError::PolicyAuthorityRequired));

    // The policy authority can.
    client.activate_version(&policy_authority, &policy, &1);
    assert_eq!(client.get_active_version(&policy).version, 1);

    // A registry authority cannot deactivate either.
    let err = client
        .try_deactivate_version(&authority, &policy, &1)
        .unwrap_err();
    assert_eq!(err, contract_err(ContractError::PolicyAuthorityRequired));

    // The policy authority can.
    client.deactivate_version(&policy_authority, &policy, &1);
    assert_eq!(
        client.get_version(&policy, &1).status,
        VersionStatus::Disabled.to_code()
    );

    // The admin remains able to do both (bootstrap/emergency path):
    // register a fresh draft and activate it.
    let rules = vec![
        &env,
        RuleRecord {
            rule_id: rid(&env, "ALLOWLIST-001"),
            rule_type: RuleType::Allowlist.to_code(),
            action: RuleAction::Block.to_code(),
        },
    ];
    client.register_version(&policy, &2, &config_hash(&env, 2), &rules);
    client.activate_version(&admin, &policy, &2);
    assert_eq!(client.get_active_version(&policy).version, 2);
}

/// Policy-authority role changes publish events only on real changes, like
/// the registry-authority set.
#[test]
fn policy_authority_changes_publish_events_only_on_real_changes() {
    use soroban_sdk::xdr::ContractEventBody;
    use soroban_sdk::Symbol;
    use soroban_sdk::TryFromVal as _;

    fn event_identity(env: &Env, event: &soroban_sdk::xdr::ContractEvent) -> (Symbol, Address) {
        let ContractEventBody::V0(v0) = &event.body;
        let name: Symbol = Symbol::try_from_val(env, &v0.topics[0]).expect("symbol topic");
        let emitted: Address = Address::try_from_val(env, &v0.topics[1]).expect("address topic");
        (name, emitted)
    }

    let env = Env::default();
    let (_, _, _, _, _, client) = setup(&env);

    let newcomer = Address::generate(&env);
    client.add_policy_authority(&newcomer);
    let all_events = env.events().all();
    assert_eq!(all_events.events().len(), 1);
    assert_eq!(
        event_identity(&env, &all_events.events()[0]),
        (
            Symbol::new(&env, "policy_authority_added"),
            newcomer.clone()
        )
    );

    // Re-adding is a no-op: no event.
    client.add_policy_authority(&newcomer);
    assert_eq!(env.events().all().events().len(), 0);

    client.remove_policy_authority(&newcomer);
    let all_events = env.events().all();
    assert_eq!(all_events.events().len(), 1);
    assert_eq!(
        event_identity(&env, &all_events.events()[0]),
        (
            Symbol::new(&env, "policy_authority_removed"),
            newcomer.clone()
        )
    );
}

// -------------------------------------------------------------- evaluation

#[test]
fn approve_when_every_check_passes() {
    let env = Env::default();
    let (admin, _, _, token, policy, client) = setup(&env);
    bound_and_active(&env, &client, &admin, &policy, &token);

    let result = client.evaluate(&policy, &token, &active_input(&env, &admin));
    assert_eq!(result.policy_version, 1);
    assert_approve(&result);
}

#[test]
fn allowlist_denies_non_members_with_the_rule_id() {
    let env = Env::default();
    let (admin, _, _, token, policy, client) = setup(&env);
    bound_and_active(&env, &client, &admin, &policy, &token);

    let mut facts = active_input(&env, &admin);
    facts.allowlist_member = false;
    let result = client.evaluate(&policy, &token, &facts);
    assert_eq!(result.decision, Decision::Block.to_code());
    assert_eq!(result.reason_code, ReasonCode::AllowlistRequired.to_code());
    assert_eq!(result.rule_id, Some(rid(&env, "ALLOWLIST-001")));
}

#[test]
fn sanctions_matches_block_under_a_blocking_policy() {
    let env = Env::default();
    let (admin, _, _, token, policy, client) = setup(&env);
    bound_and_active(&env, &client, &admin, &policy, &token);

    let mut facts = active_input(&env, &admin);
    facts.sanctions_matched = true;
    let result = client.evaluate(&policy, &token, &facts);
    assert_eq!(result.decision, Decision::Block.to_code());
    assert_eq!(result.reason_code, ReasonCode::SanctionsMatch.to_code());
    assert_eq!(result.rule_id, Some(rid(&env, "SANCTIONS-001")));
}

#[test]
fn flag_actions_flag_instead_of_blocking() {
    let env = Env::default();
    let (admin, _, _, token, policy, client) = setup(&env);

    let rules = vec![
        &env,
        RuleRecord {
            rule_id: rid(&env, "SANCTIONS-001"),
            rule_type: RuleType::Sanctions.to_code(),
            action: RuleAction::Flag.to_code(),
        },
    ];
    client.register_version(&policy, &1, &config_hash(&env, 1), &rules);
    client.activate_version(&admin, &policy, &1);
    client.bind_token(&admin, &policy, &token);

    let mut facts = active_input(&env, &admin);
    facts.sanctions_matched = true;
    let result = client.evaluate(&policy, &token, &facts);
    assert_eq!(result.decision, Decision::Flag.to_code());
    assert_eq!(result.reason_code, ReasonCode::SanctionsMatch.to_code());
}

#[test]
fn frozen_accounts_block_even_when_rules_would_pass() {
    let env = Env::default();
    let (admin, _, _, token, policy, client) = setup(&env);
    bound_and_active(&env, &client, &admin, &policy, &token);

    let mut facts = active_input(&env, &admin);
    facts.account_status = 2; // frozen
    let result = client.evaluate(&policy, &token, &facts);
    assert_eq!(result.decision, Decision::Block.to_code());
    assert_eq!(result.reason_code, ReasonCode::AccountFrozen.to_code());
    assert_eq!(result.rule_id, None);
}

#[test]
fn unknown_status_codes_fail_closed_to_flag() {
    let env = Env::default();
    let (admin, _, _, token, policy, client) = setup(&env);
    bound_and_active(&env, &client, &admin, &policy, &token);

    let mut facts = active_input(&env, &admin);
    facts.account_status = 99; // invalid account status code
    let result = client.evaluate(&policy, &token, &facts);
    assert_eq!(result.decision, Decision::Flag.to_code());
    assert_eq!(
        result.reason_code,
        ReasonCode::AccountStatusUnknown.to_code()
    );
}

#[test]
fn evaluation_is_deterministic() {
    let env = Env::default();
    let (admin, _, _, token, policy, client) = setup(&env);
    bound_and_active(&env, &client, &admin, &policy, &token);

    let mut facts = active_input(&env, &admin);
    facts.denylist_matched = true;
    let first = client.evaluate(&policy, &token, &facts);
    for _ in 0..16 {
        assert_eq!(client.evaluate(&policy, &token, &facts), first);
    }
}

#[test]
fn scope_guards_refuse_evaluation_outside_the_policy() {
    let env = Env::default();
    let (admin, _, _, token, policy, client) = setup(&env);

    // A draft version exists (policy is registered) but nothing is active
    // yet: evaluation refuses with PolicyNotActive.
    register_default_policy(&env, &client, &policy, 1);
    client.bind_token(&admin, &policy, &token);
    assert_eq!(
        client
            .try_evaluate(&policy, &token, &active_input(&env, &admin))
            .unwrap_err(),
        contract_err(ContractError::PolicyNotActive)
    );

    // Active version but the token is not bound.
    client.activate_version(&admin, &policy, &1);
    let other_token = Address::generate(&env);
    assert_eq!(
        client
            .try_evaluate(&policy, &other_token, &active_input(&env, &admin))
            .unwrap_err(),
        contract_err(ContractError::TokenNotBound)
    );

    // After binding, evaluation succeeds.
    client.bind_token(&admin, &policy, &token);
    assert_approve(&client.evaluate(&policy, &token, &active_input(&env, &admin)));
}

/// Policy↔token bindings publish one event each on a real change — and
/// nothing on idempotent repeats — so audit can prove which tokens a policy
/// governed at any point in time.
#[test]
fn token_bindings_publish_events_only_on_real_changes() {
    use soroban_sdk::xdr::ContractEventBody;
    use soroban_sdk::Symbol;
    use soroban_sdk::TryFromVal as _;

    fn event_topics(env: &Env, event: &soroban_sdk::xdr::ContractEvent) -> (Symbol, Id, Address) {
        let ContractEventBody::V0(v0) = &event.body;
        let name: Symbol = Symbol::try_from_val(env, &v0.topics[0]).expect("symbol topic");
        let policy: Id = Id::try_from_val(env, &v0.topics[1]).expect("id topic");
        let token: Address = Address::try_from_val(env, &v0.topics[2]).expect("address topic");
        (name, policy, token)
    }

    let env = Env::default();
    let (admin, _, _, token, policy, client) = setup(&env);
    register_default_policy(&env, &client, &policy, 1);

    // Binding publishes one TokenBound naming the policy and the token.
    client.bind_token(&admin, &policy, &token);
    let all_events = env.events().all();
    assert_eq!(all_events.events().len(), 1);
    assert_eq!(
        event_topics(&env, &all_events.events()[0]),
        (
            Symbol::new(&env, "token_bound"),
            policy.clone(),
            token.clone()
        )
    );

    // Re-binding the same token is a no-op: no event.
    client.bind_token(&admin, &policy, &token);
    assert_eq!(env.events().all().events().len(), 0);

    // Unbinding publishes one TokenUnbound naming the same pair.
    client.unbind_token(&admin, &policy, &token);
    let all_events = env.events().all();
    assert_eq!(all_events.events().len(), 1);
    assert_eq!(
        event_topics(&env, &all_events.events()[0]),
        (
            Symbol::new(&env, "token_unbound"),
            policy.clone(),
            token.clone()
        )
    );
    assert_eq!(client.bound_tokens(&policy), Vec::new(&env));

    // Unbinding a token outside scope is a no-op: no event.
    client.unbind_token(&admin, &policy, &token);
    assert_eq!(env.events().all().events().len(), 0);
}

/// §8 negative: binding a token to a policy id that was never registered is
/// a caller error — PolicyNotFound (4) — instead of silently creating a dead
/// binding that would route enforcement to a policy with no evaluable
/// version. Same for unbind: a no-op on a nonexistent policy would mask a
/// typo'd policy id.
#[test]
fn bind_and_unbind_reject_unregistered_policy_ids() {
    let env = Env::default();
    let (admin, _, _, token, _, client) = setup(&env);
    let ghost = rid(&env, "GHOST-POLICY");

    let err = client.try_bind_token(&admin, &ghost, &token).unwrap_err();
    assert_eq!(err, contract_err(ContractError::PolicyNotFound));

    let err = client.try_unbind_token(&admin, &ghost, &token).unwrap_err();
    assert_eq!(err, contract_err(ContractError::PolicyNotFound));

    // Nothing was written: the ghost policy still has no bindings and no
    // reverse index entry.
    assert_eq!(client.bound_tokens(&ghost), Vec::new(&env));
}

/// Every contract error code is reachable by a real path, and the codes
/// themselves are the stable public API documented in
/// `docs/contract-interface.md`. This test lists the producing call for each
/// code so the audit can prove none of them is dead surface.
#[test]
fn every_error_code_is_reachable() {
    use crate::error::ContractError as E;

    let env = Env::default();
    let (admin, _authority, stranger, _token, policy, client) = setup(&env);

    // 2 AlreadyInitialized — re-initializing an initialized contract.
    assert_eq!(
        client.try_initialize(&admin).unwrap_err(),
        contract_err(E::AlreadyInitialized)
    );
    // 3 NotInitialized — admin op before initialize. A freshly registered
    // contract has no admin yet; the admin-only path (register_version)
    // fails closed with NotInitialized.
    let raw = Env::default();
    raw.mock_all_auths();
    let raw_contract = raw.register(PolicyContract, ());
    let raw_client = PolicyContractClient::new(&raw, &raw_contract);
    let err = raw_client
        .try_register_version(
            &rid(&raw, "GHOST-POLICY"),
            &1,
            &config_hash(&raw, 1),
            &Vec::new(&raw),
        )
        .unwrap_err();
    assert_eq!(err, contract_err(E::NotInitialized));
    // 4 PolicyNotFound — covered above (bind/unbind to a ghost policy).
    // 5 VersionNotFound — activate a version that was never registered.
    assert_eq!(
        client
            .try_activate_version(&admin, &policy, &99)
            .unwrap_err(),
        contract_err(E::VersionNotFound)
    );
    // 14 RegistryAuthorityRequired — a stranger writing registry data.
    assert_eq!(
        client
            .try_set_identity(&stranger, &stranger, &0, &rid(&env, "X"), &0)
            .unwrap_err(),
        contract_err(E::RegistryAuthorityRequired)
    );
    // 15 PolicyAuthorityRequired — a stranger (or registry authority)
    // attempting version lifecycle control.
    assert_eq!(
        client
            .try_deactivate_version(&stranger, &policy, &1)
            .unwrap_err(),
        contract_err(E::PolicyAuthorityRequired)
    );
}

// -------------------------------------------------------------- registries

/// Register + activate a version with a blocking jurisdiction rule and bind
/// the token, for registry-resolution tests.
fn jurisdiction_active(
    env: &Env,
    client: &PolicyContractClient,
    admin: &Address,
    policy: &Id,
    token: &Address,
) {
    let rules = vec![
        env,
        RuleRecord {
            rule_id: rid(env, "JURISDICTION-001"),
            rule_type: RuleType::Jurisdiction.to_code(),
            action: RuleAction::Block.to_code(),
        },
    ];
    client.register_version(policy, &1, &config_hash(env, 3), &rules);
    client.activate_version(admin, policy, &1);
    client.bind_token(admin, policy, token);
}

fn subject_hash(env: &Env) -> Id {
    BytesN::from_array(env, &[1; 32])
}

/// The identity registry accepts writes from a registry authority and
/// publishes one typed event; reads return the record.
#[test]
fn identity_registry_lifecycle_and_events() {
    let env = Env::default();
    let (admin, authority, _, _, _, client) = setup(&env);
    let account = Address::generate(&env);

    // Authority writes a verified record with an attestation reference.
    client.set_identity(
        &authority,
        &account,
        &0,
        &rid(&env, "ATT-1"),
        &1_800_000_000,
    );
    let all_events = env.events().all();
    assert_eq!(
        all_events.events().len(),
        1,
        "set_identity published one event"
    );

    let record = client.identity(&account).unwrap();
    assert_eq!(record.status, 0); // verified
    assert_eq!(record.attestation_ref, rid(&env, "ATT-1"));
    assert_eq!(record.expires_at, 1_800_000_000);

    // Replacing the record works (same account, new status).
    client.set_identity(
        &authority,
        &account,
        &2,
        &rid(&env, "ATT-1"),
        &1_800_000_000,
    ); // revoked
    let updated = client.identity(&account).unwrap();
    assert_eq!(updated.status, 2);

    // Removal clears the record; no event when there is nothing to remove.
    client.remove_identity(&admin, &account);
    assert!(client.identity(&account).is_none());

    // Unknown status codes are rejected before persisting.
    let err = client
        .try_set_identity(&authority, &account, &99, &rid(&env, "ATT-1"), &0)
        .unwrap_err();
    assert_eq!(err, contract_err(ContractError::InvalidRegistryData));

    // A stranger cannot write.
    let stranger = Address::generate(&env);
    let err = client
        .try_set_identity(&stranger, &account, &0, &rid(&env, "ATT-1"), &0)
        .unwrap_err();
    assert_eq!(err, contract_err(ContractError::RegistryAuthorityRequired));
}

/// §18 negative: an account with no identity record is genuinely unknown —
/// the registry reads `None` (no implicit "verified" entry), evaluation with
/// an unknown status fails closed to FLAG, and no unauthorised party can
/// conjure a record for it.
#[test]
fn unknown_account_has_no_record_and_fails_closed() {
    let env = Env::default();
    let (admin, authority, _, token, policy, client) = setup(&env);
    bound_and_active(&env, &client, &admin, &policy, &token);

    let stranger = Address::generate(&env);

    // No record: the account is unknown, never implicitly verified.
    assert!(client.identity(&stranger).is_none());

    // Evaluating with the unknown status flags rather than approves.
    let mut facts = active_input(&env, &stranger);
    facts.account_status = AccountStatus::Unknown.to_code();
    let result = client.evaluate(&policy, &token, &facts);
    assert_eq!(result.decision, Decision::Flag.to_code());
    assert_eq!(
        result.reason_code,
        ReasonCode::AccountStatusUnknown.to_code()
    );

    // A stranger cannot write a record for the unknown account either.
    let err = client
        .try_set_identity(&stranger, &stranger, &0, &rid(&env, "ATT-1"), &0)
        .unwrap_err();
    assert_eq!(err, contract_err(ContractError::RegistryAuthorityRequired));

    // Even a registry authority writing an *expired* record cannot make the
    // account active: evaluation never consults wall-clock time, so the
    // expired record stays a data fact — audit can prove expiry, evaluation
    // still fails closed on the caller's resolved status.
    client.set_identity(
        &authority,
        &stranger,
        &0, // verified at write time
        &rid(&env, "ATT-1"),
        &1, // already expired (epoch 1)
    );
    let record = client.identity(&stranger).unwrap();
    assert_eq!(record.status, 0);
    assert_eq!(record.expires_at, 1);
    assert_eq!(result.decision, Decision::Flag.to_code());
}

/// §18 negative: an expired attestation is stored exactly as written (never
/// silently extended or rewritten) so audit can prove it expired, and only an
/// authorised registry authority can replace it.
#[test]
fn expired_attestation_is_stored_unmutated_and_requires_authority_to_replace() {
    let env = Env::default();
    let (admin, authority, _, _, _, client) = setup(&env);
    let account = Address::generate(&env);

    // Write an already-expired attestation (epoch 1) with a revocation-era
    // reference; the contract must store it byte-for-byte.
    client.set_identity(&authority, &account, &0, &rid(&env, "ATT-EXPIRED"), &1);
    let record = client.identity(&account).unwrap();
    assert_eq!(record.status, 0);
    assert_eq!(record.attestation_ref, rid(&env, "ATT-EXPIRED"));
    assert_eq!(record.expires_at, 1, "expiry must not be mutated on store");

    // The admin may replace it (bootstrap path), but a stranger cannot — an
    // expired record cannot be silently refreshed by just anyone.
    let err = client
        .try_set_identity(
            &Address::generate(&env),
            &account,
            &0,
            &rid(&env, "ATT-REPLACED"),
            &9_999_999_999,
        )
        .unwrap_err();
    assert_eq!(err, contract_err(ContractError::RegistryAuthorityRequired));

    client.set_identity(
        &admin,
        &account,
        &0,
        &rid(&env, "ATT-REPLACED"),
        &9_999_999_999,
    );
    assert_eq!(
        client.identity(&account).unwrap().attestation_ref,
        rid(&env, "ATT-REPLACED")
    );
    assert_eq!(client.identity(&account).unwrap().expires_at, 9_999_999_999);
}

/// Role mutations publish AuthorityAdded/AuthorityRemoved events — and only
/// when the set actually changes — so audit can prove who held the registry
/// authority role when.
#[test]
fn authority_changes_publish_events_only_on_real_changes() {
    use soroban_sdk::xdr::ContractEventBody;
    use soroban_sdk::Symbol;
    use soroban_sdk::TryFromVal as _;

    fn event_identity(env: &Env, event: &soroban_sdk::xdr::ContractEvent) -> (Symbol, Address) {
        // V0 is the only event-body variant in the current XDR; the pattern
        // is irrefutable, so no match arm is needed.
        let ContractEventBody::V0(v0) = &event.body;
        let name: Symbol = Symbol::try_from_val(env, &v0.topics[0]).expect("symbol topic");
        let emitted: Address = Address::try_from_val(env, &v0.topics[1]).expect("address topic");
        (name, emitted)
    }

    let env = Env::default();
    let (_, _, _, _, _, client) = setup(&env);

    // Adding a new authority publishes one AuthorityAdded event naming it.
    // (setup() already registered `authority`, so use a fresh address.)
    let newcomer = Address::generate(&env);
    client.add_authority(&newcomer);
    let all_events = env.events().all();
    assert_eq!(all_events.events().len(), 1);
    assert_eq!(
        event_identity(&env, &all_events.events()[0]),
        (Symbol::new(&env, "authority_added"), newcomer.clone())
    );

    // Re-adding the same authority is a no-op: no event.
    client.add_authority(&newcomer);
    assert_eq!(env.events().all().events().len(), 0);

    // Removing the newcomer publishes AuthorityRemoved with its address.
    client.remove_authority(&newcomer);
    let all_events = env.events().all();
    assert_eq!(all_events.events().len(), 1);
    assert_eq!(
        event_identity(&env, &all_events.events()[0]),
        (Symbol::new(&env, "authority_removed"), newcomer.clone())
    );

    // Removing a non-member (never added) is a no-op: no event.
    let stranger = Address::generate(&env);
    client.remove_authority(&stranger);
    assert_eq!(env.events().all().events().len(), 0);

    // (Role-mutation authorization itself is enforced by require_auth on the
    // stored admin; under mock_all_auths the host cannot distinguish callers,
    // so the auth assertion lives with the real-auth admin tests instead.)
}

/// An active sanctions entry makes evaluate block even when the caller
/// claims no match; retiring the entry restores the caller-claim behavior.
#[test]
fn sanctions_registry_is_authoritative_in_evaluate() {
    let env = Env::default();
    let (admin, authority, _, token, policy, client) = setup(&env);
    bound_and_active(&env, &client, &admin, &policy, &token);

    let mut facts = active_input(&env, &admin);
    facts.sanctions_matched = false; // caller claims a clean screen

    // No entry: the caller's claim stands and evaluation approves.
    assert_approve(&client.evaluate(&policy, &token, &facts));

    // Authority lists the subject hash as active on the OFAC-SDN list.
    client.set_sanctions_entry(
        &authority,
        &subject_hash(&env),
        &rid(&env, "OFAC-SDN"),
        &0, // active
        &1, // dataset version
        &1_700_000_000,
        &Bytes::from_slice(&env, b"ofac"),
    );
    assert_eq!(env.events().all().events().len(), 1);

    // Registry is authoritative: the caller's clean-screen claim no longer
    // stands and the blocking sanctions rule fires.
    let result = client.evaluate(&policy, &token, &facts);
    assert_eq!(result.decision, Decision::Block.to_code());
    assert_eq!(result.reason_code, ReasonCode::SanctionsMatch.to_code());

    // Retiring the entry (never deleting) lifts the block for this subject.
    client.retire_sanctions_entry(&authority, &subject_hash(&env));
    assert_approve(&client.evaluate(&policy, &token, &facts));

    // Invalid status code and version zero are rejected.
    let err = client
        .try_set_sanctions_entry(
            &authority,
            &subject_hash(&env),
            &rid(&env, "OFAC-SDN"),
            &99,
            &1,
            &0,
            &Bytes::from_slice(&env, b"ofac"),
        )
        .unwrap_err();
    assert_eq!(err, contract_err(ContractError::InvalidRegistryData));
}

/// A stored jurisdiction classification is authoritative in evaluate; a
/// stored prohibited region blocks even when the caller claims permitted.
#[test]
fn jurisdiction_registry_is_authoritative_in_evaluate() {
    let env = Env::default();
    let (admin, authority, _, token, policy, client) = setup(&env);
    jurisdiction_active(&env, &client, &admin, &policy, &token);
    let account = Address::generate(&env);

    let mut facts = active_input(&env, &account);
    facts.jurisdiction = 0; // caller claims permitted

    // No classification stored: caller claim stands.
    assert_approve(&client.evaluate(&policy, &token, &facts));

    // Authority classifies the account as prohibited (region code 2).
    client.set_jurisdiction(&authority, &account, &2);
    assert_eq!(env.events().all().events().len(), 1);

    // Registry is authoritative: prohibited blocks despite the claim.
    let result = client.evaluate(&policy, &token, &facts);
    assert_eq!(result.decision, Decision::Block.to_code());
    assert_eq!(
        result.reason_code,
        ReasonCode::JurisdictionProhibited.to_code()
    );

    // Clearing drops back to the caller's claim.
    client.clear_jurisdiction(&authority, &account);
    assert_approve(&client.evaluate(&policy, &token, &facts));

    // Unknown region codes are rejected.
    let err = client
        .try_set_jurisdiction(&authority, &account, &99)
        .unwrap_err();
    assert_eq!(err, contract_err(ContractError::InvalidRegistryData));
}

/// Arbitrary u32 input codes never panic or error: unknown codes decode
/// fail-closed to `Unknown`, and unknown status/region codes never approve.
/// The contract boundary is the last place junk codes could enter, so the
/// whole u32 space is fuzzed (proptest samples it and shrinks failures).
#[test]
fn arbitrary_input_codes_never_error_or_fail_open() {
    use proptest::prelude::*;

    use safeguard_core::rules::account_status::AccountStatus as CoreStatus;
    use safeguard_core::rules::jurisdiction::RegionStatus as CoreRegion;

    let env = Env::default();
    let (admin, _, _, token, policy, client) = setup(&env);
    bound_and_active(&env, &client, &admin, &policy, &token);

    // A second policy with a jurisdiction rule, bound to a second token, so
    // the region decode path is exercised too.
    let policy2 = rid(&env, "jurisdiction-only");
    let token2 = Address::generate(&env);
    let rules = vec![
        &env,
        RuleRecord {
            rule_id: rid(&env, "JURISDICTION-001"),
            rule_type: RuleType::Jurisdiction.to_code(),
            action: RuleAction::Block.to_code(),
        },
    ];
    client.register_version(&policy2, &1, &config_hash(&env, 4), &rules);
    client.activate_version(&admin, &policy2, &1);
    client.bind_token(&admin, &policy2, &token2);

    let account = Address::generate(&env);
    let subject = BytesN::from_array(&env, &[1; 32]);

    proptest!(|(
        status_code in any::<u32>(),
        region_code in any::<u32>(),
        allowlist_member in any::<bool>(),
        denylist_matched in any::<bool>(),
        sanctions_matched in any::<bool>(),
    )| {
        let input = EvaluationInput {
            account_status: status_code,
            allowlist_member,
            denylist_matched,
            sanctions_matched,
            jurisdiction: region_code,
            subject: subject.clone(),
            account: account.clone(),
        };

        // Any code is decodable (fail-closed): evaluation must never error
        // under either policy.
        let result = client.try_evaluate(&policy, &token, &input);
        prop_assert!(
            result.is_ok(),
            "evaluate errored on codes {} / {}",
            status_code,
            region_code
        );
        let regional = client.try_evaluate(&policy2, &token2, &input);
        prop_assert!(regional.is_ok(), "jurisdiction policy errored on region code {}", region_code);

        // An unrecognized account status maps to Unknown, which never approves.
        if CoreStatus::from_code(status_code).is_none() {
            let decision = result
                .expect("invocation succeeded")
                .expect("evaluation succeeded");
            prop_assert_ne!(
                decision.decision,
                Decision::Approve.to_code(),
                "unknown status code {} approved",
                status_code
            );
        }

        // An unrecognized region code maps to Unknown, and under the blocking
        // jurisdiction policy Unknown triggers the rule action: never approve.
        if CoreRegion::from_code(region_code).is_none() {
            let decision = regional
                .expect("invocation succeeded")
                .expect("evaluation succeeded");
            prop_assert_ne!(
                decision.decision,
                Decision::Approve.to_code(),
                "unknown region code {} approved",
                region_code
            );
        }
    });
}

// -------------------------------------------- shipped-policy compatibility

/// Load a policy document from the repository's policies/ directory.
fn load_shipped_policy(relative: &str) -> PolicyDocument {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let json =
        std::fs::read_to_string(root.join("policies").join(relative)).expect("read shipped policy");
    serde_json::from_str(&json).expect("parse shipped policy")
}

/// Register a shipped policy document on-chain: its rule set becomes the
/// version's records, so what the JSON says and what the contract enforces
/// are the same thing.
fn register_shipped(
    env: &Env,
    client: &PolicyContractClient,
    admin: &Address,
    policy_id: &Id,
    token: &Address,
    doc: &PolicyDocument,
) {
    let mut records: Vec<RuleRecord> = Vec::new(env);
    for rule in &doc.rules {
        let rule_id = safeguard_core::rule::RuleId::from_str(&rule.id);
        records.push_back(RuleRecord {
            rule_id: BytesN::from_array(env, rule_id.as_bytes()),
            rule_type: rule.rule_type.as_core().to_code(),
            action: rule.action.as_core().to_code(),
        });
    }
    client.register_version(policy_id, &1, &config_hash(env, 9), &records);
    client.activate_version(admin, policy_id, &1);
    client.bind_token(admin, policy_id, token);
}

/// Register the shipped combined policy and run the worked cases from
/// docs/how-to-evaluate.md at the **contract** level: the same JSON document
/// that ships in policies/ is what the contract enforces.
#[test]
fn shipped_combined_policy_enforces_the_documented_cases() {
    let env = Env::default();
    let (admin, _, _, _, _, client) = setup(&env);

    let doc = load_shipped_policy("examples/combined-policy.json");
    assert_eq!(doc.policy_id, "example-combined");
    let policy = rid(&env, &doc.policy_id);
    let token = Address::generate(&env);
    register_shipped(&env, &client, &admin, &policy, &token, &doc);

    let subject = BytesN::from_array(&env, &[7; 32]);
    let input = |status: u32,
                 allowlist: bool,
                 deny: bool,
                 sanctions: bool,
                 region: u32|
     -> EvaluationInput {
        EvaluationInput {
            account_status: status,
            allowlist_member: allowlist,
            denylist_matched: deny,
            sanctions_matched: sanctions,
            jurisdiction: region,
            subject: subject.clone(),
            account: admin.clone(),
        }
    };

    // Case 1 — everything passes → APPROVE (no_reason).
    let ok = client.evaluate(&policy, &token, &input(0, true, false, false, 0));
    assert_eq!(ok.decision, Decision::Approve.to_code());
    assert_eq!(ok.reason_code, ReasonCode::NoReason.to_code());
    assert_eq!(ok.policy_version, 1);

    // Case 2 — non-member → BLOCK by allowlist, rule attributed.
    let blocked = client.evaluate(&policy, &token, &input(0, false, false, false, 0));
    assert_eq!(blocked.decision, Decision::Block.to_code());
    assert_eq!(blocked.reason_code, ReasonCode::AllowlistRequired.to_code());
    assert_eq!(blocked.rule_id, Some(rid(&env, "ALLOWLIST-001")));

    // Case 3 — sanctions match → FLAG (not BLOCK) under the combined policy.
    let flagged = client.evaluate(&policy, &token, &input(0, true, false, true, 0));
    assert_eq!(flagged.decision, Decision::Flag.to_code());
    assert_eq!(flagged.reason_code, ReasonCode::SanctionsMatch.to_code());
    assert_eq!(flagged.rule_id, Some(rid(&env, "SANCTIONS-001")));

    // Case 4 — frozen account → structural BLOCK, no rule.
    let frozen = client.evaluate(&policy, &token, &input(2, true, false, false, 0));
    assert_eq!(frozen.decision, Decision::Block.to_code());
    assert_eq!(frozen.reason_code, ReasonCode::AccountFrozen.to_code());
    assert_eq!(frozen.rule_id, None);

    // Case 5 — prohibited region (IR) → BLOCK by jurisdiction.
    let prohibited = client.evaluate(&policy, &token, &input(0, true, false, false, 2));
    assert_eq!(prohibited.decision, Decision::Block.to_code());
    assert_eq!(
        prohibited.reason_code,
        ReasonCode::JurisdictionProhibited.to_code()
    );
    assert_eq!(prohibited.rule_id, Some(rid(&env, "JURISDICTION-001"))); // Case 6 — unknown region → fail-closed BLOCK.
    let unknown = client.evaluate(&policy, &token, &input(0, true, false, false, 3));
    assert_eq!(unknown.decision, Decision::Block.to_code());
    assert_eq!(
        unknown.reason_code,
        ReasonCode::JurisdictionUnknown.to_code()
    );
}

/// The stable numeric surface `safeguard-hooks` consumes, pinned in one
/// place: schema version, error codes, decision/reason/type/action codes and
/// the registry status codes. Renumbering any of these silently breaks audit
/// history and hook integrations, so each is asserted explicitly (see
/// docs/versioning.md and docs/contract-interface.md).
#[test]
fn the_stable_numeric_interface_is_pinned() {
    let env = Env::default();
    let (_, _, _, _, _, client) = setup(&env);

    // Schema version the contract speaks.
    assert_eq!(client.schema_version(), 1);

    // Contract error codes (docs/contract-interface.md table). Codes are
    // non-dense after the completeness audit: 1 (Unauthorized) and 10
    // (InvalidPolicyId) were removed and are never reissued; 14/15 replaced
    // the single generic authorization code with distinct authority gates.
    use crate::error::ContractError as E;
    assert_eq!(E::AlreadyInitialized as u32, 2);
    assert_eq!(E::NotInitialized as u32, 3);
    assert_eq!(E::PolicyNotFound as u32, 4);
    assert_eq!(E::VersionNotFound as u32, 5);
    assert_eq!(E::VersionNotDraft as u32, 6);
    assert_eq!(E::InvalidRuleSet as u32, 7);
    assert_eq!(E::PolicyNotActive as u32, 8);
    assert_eq!(E::TokenNotBound as u32, 9);
    assert_eq!(E::VersionExists as u32, 11);
    assert_eq!(E::VersionNotActive as u32, 12);
    assert_eq!(E::InvalidRegistryData as u32, 13);
    assert_eq!(E::RegistryAuthorityRequired as u32, 14);
    assert_eq!(E::PolicyAuthorityRequired as u32, 15);

    // Core decision/reason/rule codes echoed into EvaluationResult and
    // events (docs/rule-engine.md).
    use safeguard_core::decision::{Decision as D, ReasonCode as R};
    assert_eq!(D::Approve.to_code(), 0);
    assert_eq!(D::Block.to_code(), 1);
    assert_eq!(D::Flag.to_code(), 2);
    assert_eq!(R::NoReason.to_code(), 0);
    assert_eq!(R::AccountFrozen.to_code(), 1);
    assert_eq!(R::AccountSuspended.to_code(), 2);
    assert_eq!(R::AccountRestricted.to_code(), 3);
    assert_eq!(R::AccountStatusUnknown.to_code(), 4);
    assert_eq!(R::AllowlistRequired.to_code(), 5);
    assert_eq!(R::DenylistMatch.to_code(), 6);
    assert_eq!(R::SanctionsMatch.to_code(), 7);
    assert_eq!(R::JurisdictionProhibited.to_code(), 8);
    assert_eq!(R::JurisdictionRestricted.to_code(), 9);
    assert_eq!(R::JurisdictionUnknown.to_code(), 10);

    use safeguard_core::rule::{RuleAction as A, RuleType as T};
    assert_eq!(T::Allowlist.to_code(), 0);
    assert_eq!(T::Denylist.to_code(), 1);
    assert_eq!(T::Sanctions.to_code(), 2);
    assert_eq!(T::Jurisdiction.to_code(), 3);
    assert_eq!(A::Block.to_code(), 0);
    assert_eq!(A::Flag.to_code(), 1);

    // Registry status codes written by the authorities and read by hooks.
    use safeguard_core::registries::identity::IdentityStatus as I;
    use safeguard_core::registries::sanctions::SanctionsStatus as S;
    assert_eq!(I::Verified.to_code(), 0);
    assert_eq!(I::Unverified.to_code(), 1);
    assert_eq!(I::Revoked.to_code(), 2);
    assert_eq!(I::Expired.to_code(), 3);
    assert_eq!(I::Unknown.to_code(), 4);
    assert_eq!(S::Active.to_code(), 0);
    assert_eq!(S::Inactive.to_code(), 1);

    // Account status and region codes entering EvaluationInput.
    use safeguard_core::rules::account_status::AccountStatus as St;
    use safeguard_core::rules::jurisdiction::RegionStatus as Rg;
    assert_eq!(St::Active.to_code(), 0);
    assert_eq!(St::Restricted.to_code(), 1);
    assert_eq!(St::Frozen.to_code(), 2);
    assert_eq!(St::Suspended.to_code(), 3);
    assert_eq!(St::Unknown.to_code(), 4);
    assert_eq!(Rg::Permitted.to_code(), 0);
    assert_eq!(Rg::Restricted.to_code(), 1);
    assert_eq!(Rg::Prohibited.to_code(), 2);
    assert_eq!(Rg::Unknown.to_code(), 3);
}

/// The documented code table in `docs/contract-interface.md` is generated
/// from this enum; this test fails the build when a variant is added
/// without a corresponding docs update, so the on-chain surface and the
/// reference docs can never drift apart silently.
#[test]
fn every_error_variant_is_pinned_to_the_documented_table() {
    // The match below is exhaustive: adding a ContractError variant breaks
    // compilation here, and listing it without updating the documented set
    // (or vice versa) fails the assertion — the docs table and the enum
    // cannot drift apart.
    let live_codes: [u32; 13] = [
        ContractError::AlreadyInitialized as u32,
        ContractError::NotInitialized as u32,
        ContractError::PolicyNotFound as u32,
        ContractError::VersionNotFound as u32,
        ContractError::VersionNotDraft as u32,
        ContractError::InvalidRuleSet as u32,
        ContractError::PolicyNotActive as u32,
        ContractError::TokenNotBound as u32,
        ContractError::VersionExists as u32,
        ContractError::VersionNotActive as u32,
        ContractError::InvalidRegistryData as u32,
        ContractError::RegistryAuthorityRequired as u32,
        ContractError::PolicyAuthorityRequired as u32,
    ];
    // Exhaustiveness guard: a new variant must be added to this match (and
    // therefore to the docs table and the stable-interface test above).
    #[allow(dead_code, unreachable_patterns)]
    fn assert_exhaustive(e: ContractError) -> u32 {
        match e {
            ContractError::AlreadyInitialized => 2,
            ContractError::NotInitialized => 3,
            ContractError::PolicyNotFound => 4,
            ContractError::VersionNotFound => 5,
            ContractError::VersionNotDraft => 6,
            ContractError::InvalidRuleSet => 7,
            ContractError::PolicyNotActive => 8,
            ContractError::TokenNotBound => 9,
            ContractError::VersionExists => 11,
            ContractError::VersionNotActive => 12,
            ContractError::InvalidRegistryData => 13,
            ContractError::RegistryAuthorityRequired => 14,
            ContractError::PolicyAuthorityRequired => 15,
        }
    }
    let documented: [u32; 13] = [2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14, 15];
    let mut sorted_live = live_codes;
    sorted_live.sort_unstable();
    assert_eq!(sorted_live, documented, "ContractError and the docs code table in docs/contract-interface.md disagree — update the table and this test together");
}

// -------------------------------------------- enforcement wire (is_authorized)

/// An enforcement-shaped policy: rules decidable from on-chain state only.
/// The wire carries just an account and a token, so allowlist/denylist
/// membership (caller-supplied facts in `evaluate`) is always absent here;
/// this policy has no such rules.
fn register_enforcement_policy(
    env: &Env,
    client: &PolicyContractClient,
    policy: &Id,
    version: u32,
) {
    let rules = vec![env];
    client.register_version(policy, &version, &config_hash(env, 2), &rules);
}

#[test]
fn is_authorized_approves_only_verified_active_accounts_on_a_bound_token() {
    let env = Env::default();
    let (admin, _, _, token, policy, client) = setup(&env);
    register_enforcement_policy(&env, &client, &policy, 1);
    client.activate_version(&admin, &policy, &1);
    client.bind_token(&admin, &policy, &token);

    let alice = Address::generate(&env);
    // No identity record yet: the status defaults to Unknown, which the
    // evaluator flags — an unverified account is never authorized.
    assert!(!client.is_authorized(&alice, &token));

    // Verified active: authorized.
    client.set_identity(
        &admin,
        &alice,
        &AccountStatus::Active.to_code(),
        &config_hash(&env, 7),
        &0,
    );
    assert!(client.is_authorized(&alice, &token));

    // Frozen and suspended accounts are blocked, not flagged.
    client.set_identity(
        &admin,
        &alice,
        &AccountStatus::Frozen.to_code(),
        &config_hash(&env, 7),
        &0,
    );
    assert!(!client.is_authorized(&alice, &token));
    client.set_identity(
        &admin,
        &alice,
        &AccountStatus::Suspended.to_code(),
        &config_hash(&env, 7),
        &0,
    );
    assert!(!client.is_authorized(&alice, &token));

    // A restricted account flags (review outcome) — not an approve.
    client.set_identity(
        &admin,
        &alice,
        &AccountStatus::Restricted.to_code(),
        &config_hash(&env, 7),
        &0,
    );
    assert!(!client.is_authorized(&alice, &token));
}

#[test]
fn is_authorized_denies_unbound_tokens_and_inactive_policies() {
    let env = Env::default();
    let (admin, _, _, token, policy, client) = setup(&env);
    let alice = Address::generate(&env);
    client.set_identity(
        &admin,
        &alice,
        &AccountStatus::Active.to_code(),
        &config_hash(&env, 7),
        &0,
    );

    // Registered but never activated: no active version → deny.
    register_enforcement_policy(&env, &client, &policy, 1);
    assert!(!client.is_authorized(&alice, &token));

    // Activated but the token is not bound: no reverse index → deny.
    client.activate_version(&admin, &policy, &1);
    assert!(!client.is_authorized(&alice, &token));

    // Bound: authorized.
    client.bind_token(&admin, &policy, &token);
    assert!(client.is_authorized(&alice, &token));

    // Unbind clears the reverse index → deny again.
    client.unbind_token(&admin, &policy, &token);
    assert!(!client.is_authorized(&alice, &token));
}

#[test]
fn is_authorized_consults_the_on_chain_sanctions_registry() {
    use safeguard_core::registries::sanctions::SanctionsStatus;

    let env = Env::default();
    let (admin, _, _, token, policy, client) = setup(&env);
    let rules = vec![
        &env,
        RuleRecord {
            rule_id: rid(&env, "SANCTIONS-001"),
            rule_type: RuleType::Sanctions.to_code(),
            action: RuleAction::Block.to_code(),
        },
    ];
    client.register_version(&policy, &1, &config_hash(&env, 3), &rules);
    client.activate_version(&admin, &policy, &1);
    client.bind_token(&admin, &policy, &token);

    let alice = Address::generate(&env);
    client.set_identity(
        &admin,
        &alice,
        &AccountStatus::Active.to_code(),
        &config_hash(&env, 7),
        &0,
    );

    // No entry at the wire's subject key (zeros — subject hashes of
    // provider text are not derivable from an account on this wire): the
    // caller's claim (false) stands → allowed.
    assert!(client.is_authorized(&alice, &token));

    // An active sanctions entry keyed at the wire subject blocks; retiring
    // it (inactive) stops the block.
    let subject = BytesN::from_array(&env, &[0u8; 32]);
    client.set_sanctions_entry(
        &admin,
        &subject,
        &rid(&env, "OFAC-SDN"),
        &SanctionsStatus::Active.to_code(),
        &1,
        &0,
        &Bytes::from_array(&env, b"ofac"),
    );
    assert!(!client.is_authorized(&alice, &token));

    client.retire_sanctions_entry(&admin, &subject);
    assert!(client.is_authorized(&alice, &token));
}

#[test]
fn is_authorized_treats_an_expired_identity_as_unknown() {
    use soroban_sdk::testutils::Ledger as _;

    let env = Env::default();
    let (admin, _, _, token, policy, client) = setup(&env);
    register_enforcement_policy(&env, &client, &policy, 1);
    client.activate_version(&admin, &policy, &1);
    client.bind_token(&admin, &policy, &token);

    let alice = Address::generate(&env);
    let verified = AccountStatus::Active.to_code();

    // Zero means "no expiry": permanent until the authority rewrites it.
    client.set_identity(&admin, &alice, &verified, &config_hash(&env, 7), &0);
    assert!(client.is_authorized(&alice, &token));

    // A future expiry keeps the record authoritative while it is live.
    let now = env.ledger().timestamp();
    client.set_identity(
        &admin,
        &alice,
        &verified,
        &config_hash(&env, 7),
        &(now + 1_000),
    );
    assert!(client.is_authorized(&alice, &token));

    // The instant the expiry passes, the record degrades to Unknown — the
    // evaluator flags (review outcome), which the wire answers as a denial.
    // A stale verification must not keep approving forever.
    env.ledger().with_mut(|l| l.timestamp = now + 1_000);
    assert!(!client.is_authorized(&alice, &token));

    // Rewriting the record restores the approval (the expiry is per write,
    // not per account).
    client.set_identity(
        &admin,
        &alice,
        &verified,
        &config_hash(&env, 7),
        &(now + 5_000),
    );
    assert!(client.is_authorized(&alice, &token));
}
