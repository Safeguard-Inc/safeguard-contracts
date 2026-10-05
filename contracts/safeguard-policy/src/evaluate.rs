//! On-chain evaluation: assemble a core request from contract state.
//!
//! `evaluate` is the read path `safeguard-hooks` calls. It is deliberately
//! side-effect free (no storage writes, no events) and deterministic:
//!
//! 1. Resolve the policy's **active version** record (fail: no active policy).
//! 2. Check the token is **bound** to the policy (fail: not covered).
//! 3. Resolve authoritative facts from the on-chain registries where entries
//!    exist (sanctions match, jurisdiction), falling back to the caller's
//!    claims otherwise.
//! 4. Translate the resolved facts plus the active version's rules into a
//!    [`safeguard_core::evaluation::EvaluationRequest`].
//! 5. Run the core engine and map its decision back into Soroban values.
//!
//! The contract never interprets rule semantics itself — the core engine
//! decides. The contract only owns state, scope and translation.
//!
//! # Input codes
//!
//! [`EvaluationInput`] carries core codes (status/region) or booleans
//! (membership/matches). Unknown status or region codes map to the core
//! `Unknown` variants, whose fail-closed semantics flag rather than approve,
//! so a misbehaving caller cannot widen the outcome by sending junk codes.

use soroban_sdk::{contracttype, Address, BytesN, Env};

use crate::error::ContractError;
use crate::registry;
use crate::storage::{self, PolicyVersionRecord};

use safeguard_core::decision::PolicyDecision;
use safeguard_core::evaluation::{
    EvaluationRequest, JurisdictionCheck, MatchCheck, MembershipCheck,
};
use safeguard_core::rule::{RuleAction, RuleId, RuleType};
use safeguard_core::rules::account_status::AccountStatus;
use safeguard_core::rules::jurisdiction::RegionStatus;

/// Caller-supplied facts about the subject of an evaluation.
///
/// Codes follow the stable core enums (`AccountStatus`, `RegionStatus`);
/// flags are raw booleans for membership/matches. Rule configuration (which
/// categories are enabled, with which actions) comes from the active policy
/// version, never from the caller.
///
/// # Registry resolution
///
/// Where a deployment maintains the on-chain registries, `evaluate` resolves
/// two facts authoritatively instead of trusting the caller: the sanctions
/// match (from the subject hash's entry) and the jurisdiction classification
/// (from the account's stored region). Caller claims are the fallback when no
/// entry exists, so deployments without registries behave exactly as before.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluationInput {
    /// [`safeguard_core::rules::account_status::AccountStatus`] code.
    pub account_status: u32,
    /// Whether the subject is an allowlist member (only read when the policy
    /// enables an allowlist rule).
    pub allowlist_member: bool,
    /// Whether the subject matched the denylist.
    pub denylist_matched: bool,
    /// Caller claim: whether the subject matched sanctions screening. Overridden
    /// by the on-chain sanctions registry when an entry exists for
    /// [`Self::subject`].
    pub sanctions_matched: bool,
    /// [`safeguard_core::rules::jurisdiction::RegionStatus`] code. Overridden
    /// by the on-chain jurisdiction registry when a classification exists for
    /// [`Self::account`].
    pub jurisdiction: u32,
    /// 32-byte subject reference (hash) used for sanctions-registry lookup.
    pub subject: BytesN<32>,
    /// The transacting account, used for jurisdiction-registry lookup.
    pub account: Address,
}

/// The on-chain result of an evaluation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluationResult {
    /// The active policy version that produced this decision.
    pub policy_version: u32,
    /// [`safeguard_core::decision::Decision`] code.
    pub decision: u32,
    /// [`safeguard_core::decision::ReasonCode`] code.
    pub reason_code: u32,
    /// The rule that triggered the outcome, when a rule produced it.
    pub rule_id: Option<BytesN<32>>,
}

/// Decode an account status code, fail-closed on unknown values.
fn decode_status(code: u32) -> AccountStatus {
    AccountStatus::from_code(code).unwrap_or(AccountStatus::Unknown)
}

/// Decode a region code, fail-closed on unknown values.
fn decode_region(code: u32) -> RegionStatus {
    RegionStatus::from_code(code).unwrap_or(RegionStatus::Unknown)
}

/// Decode a rule action (records are validated at registration).
fn decode_action(code: u32) -> Result<RuleAction, ContractError> {
    RuleAction::from_code(code).ok_or(ContractError::InvalidRuleSet)
}

/// Decode a stored rule record into its core representation.
fn decode_rule(
    record: &storage::RuleRecord,
) -> Result<(RuleId, RuleType, RuleAction), ContractError> {
    let rule_id = RuleId::from_bytes(record.rule_id.to_array());
    let rule_type = RuleType::from_code(record.rule_type).ok_or(ContractError::InvalidRuleSet)?;
    let action = decode_action(record.action)?;
    Ok((rule_id, rule_type, action))
}

/// Resolve the sanctions match flag for a subject.
///
/// The on-chain registry is authoritative when an entry exists: an active
/// entry means the subject matches; an inactive (retired) entry means it
/// does not. Without an entry the caller's claim is used, so deployments
/// without a registry behave exactly as before.
fn resolve_sanctions(env: &Env, input: &EvaluationInput) -> bool {
    match storage::sanctions_entry(env, &input.subject) {
        Some(record) => {
            record.status
                == safeguard_core::registries::sanctions::SanctionsStatus::Active.to_code()
        }
        None => input.sanctions_matched,
    }
}

/// Resolve the jurisdiction classification for an account.
///
/// The on-chain registry is authoritative when a classification exists;
/// otherwise the caller's region code is used (decoded fail-closed to
/// `Unknown` when malformed).
fn resolve_jurisdiction(env: &Env, input: &EvaluationInput) -> RegionStatus {
    match storage::jurisdiction(env, &input.account) {
        Some(code) => decode_region(code),
        None => decode_region(input.jurisdiction),
    }
}

/// Assemble the core evaluation request from the active version's rules,
/// the caller's facts, and the on-chain registries where authoritative.
fn assemble(
    env: &Env,
    record: &PolicyVersionRecord,
    input: &EvaluationInput,
) -> Result<EvaluationRequest, ContractError> {
    // Registry reads are metered, so they are only paid when the policy
    // actually enables the rule that consumes them: a policy without a
    // sanctions or jurisdiction rule must not pay a sanctions/jurisdiction
    // storage read on every evaluation. This is the common ENFORCE shape
    // (a rule set decidable from on-chain state), so the saving lands on
    // the hot is_authorized path.
    let sanctions_rule = record
        .rules
        .iter()
        .any(|r| r.rule_type == RuleType::Sanctions.to_code());
    let jurisdiction_rule = record
        .rules
        .iter()
        .any(|r| r.rule_type == RuleType::Jurisdiction.to_code());
    let sanctions_matched = if sanctions_rule {
        resolve_sanctions(env, input)
    } else {
        input.sanctions_matched
    };
    let jurisdiction = if jurisdiction_rule {
        resolve_jurisdiction(env, input)
    } else {
        decode_region(input.jurisdiction)
    };

    let mut request = EvaluationRequest {
        account_status: decode_status(input.account_status),
        ..EvaluationRequest::default()
    };

    for stored in record.rules.iter() {
        let (rule_id, rule_type, action) = decode_rule(&stored)?;
        match rule_type {
            RuleType::Allowlist => {
                request.allowlist = Some(MembershipCheck {
                    rule_id,
                    action,
                    member: input.allowlist_member,
                });
            }
            RuleType::Denylist => {
                request.denylist = Some(MatchCheck {
                    rule_id,
                    action,
                    matched: input.denylist_matched,
                });
            }
            RuleType::Sanctions => {
                request.sanctions = Some(MatchCheck {
                    rule_id,
                    action,
                    matched: sanctions_matched,
                });
            }
            RuleType::Jurisdiction => {
                request.jurisdiction = Some(JurisdictionCheck {
                    rule_id,
                    action,
                    region: jurisdiction,
                });
            }
        }
    }
    Ok(request)
}

/// Map a core decision into its on-chain representation.
fn to_result(env: &Env, policy_version: u32, decision: PolicyDecision) -> EvaluationResult {
    EvaluationResult {
        policy_version,
        decision: decision.decision.to_code(),
        reason_code: decision.reason_code.to_code(),
        rule_id: decision
            .rule
            .map(|id| BytesN::from_array(env, id.as_bytes())),
    }
}

/// Evaluate a subject against the active version of a policy for a token.
///
/// Public read; deterministic; never writes state. Errors only on scope or
/// configuration problems (`PolicyNotActive`, `TokenNotBound`,
/// `InvalidRuleSet`), never on the subject's compliance state — that is what
/// the returned decision expresses.
pub fn evaluate(
    env: &Env,
    policy_id: &BytesN<32>,
    token: &Address,
    input: &EvaluationInput,
) -> Result<EvaluationResult, ContractError> {
    // Scope checks first: no active policy, or an unbound token, means we
    // cannot (and must not) evaluate.
    let active = storage::active_version(env, policy_id).ok_or(ContractError::PolicyNotActive)?;
    if !registry::is_bound(env, policy_id, token) {
        return Err(ContractError::TokenNotBound);
    }
    evaluate_active(env, policy_id, active, input)
}

/// Evaluate against an active version whose token coverage the caller has
/// already established.
///
/// Kept private to the crate: `evaluate` uses it after its own scope checks,
/// and the enforcement entry point `is_authorized` uses it when the
/// registry's reverse index already proves the token is bound to the policy
/// — re-reading the full token-binding list per authorization call would be
/// pure waste on the hot enforcement path. The registry maintains the index
/// atomically with the bindings (set on bind, cleared on unbind), so the
/// index's presence is the coverage proof.
pub(crate) fn evaluate_active(
    env: &Env,
    policy_id: &BytesN<32>,
    active: u32,
    input: &EvaluationInput,
) -> Result<EvaluationResult, ContractError> {
    let record =
        storage::version_record(env, policy_id, active).ok_or(ContractError::VersionNotFound)?;

    let request = assemble(env, &record, input)?;
    let decision = safeguard_core::evaluator::evaluate(&request);
    Ok(to_result(env, active, decision))
}
