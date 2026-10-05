//! Evaluation request: the snapshot an evaluator needs to decide.
//!
//! The engine is storage-agnostic. Callers (the Soroban contract,
//! `safeguard-hooks`, SDKs) resolve every piece of external state — account
//! status, allowlist membership, denylist presence, sanctions matches,
//! jurisdiction classification — against registries and attestations, then
//! hand the engine a fully materialized [`EvaluationRequest`].
//!
//! An optional per-category check carries the id and action of the policy
//! rule that enabled it; `None` means the category is not part of the active
//! policy and must be skipped. A request therefore fully determines its
//! decision, which is what makes evaluation deterministic and reproducible.

use crate::rule::{RuleAction, RuleId};
use crate::rules::account_status::AccountStatus;
use crate::rules::jurisdiction::RegionStatus;

/// Configured state of a rule whose condition is a membership test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MembershipCheck {
    /// Id of the policy rule this check belongs to (echoed into decisions).
    pub rule_id: RuleId,
    /// Action the rule takes when its condition is met.
    pub action: RuleAction,
    /// Whether the subject satisfies the membership condition.
    pub member: bool,
}

/// Configured state of a rule whose condition is a dataset match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchCheck {
    /// Id of the policy rule this check belongs to (echoed into decisions).
    pub rule_id: RuleId,
    /// Action the rule takes when its condition is met.
    pub action: RuleAction,
    /// Whether the subject matched the dataset (denylist, sanctions, …).
    pub matched: bool,
}

/// Configured state of the jurisdiction rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JurisdictionCheck {
    /// Id of the policy rule this check belongs to (echoed into decisions).
    pub rule_id: RuleId,
    /// Action the rule takes for a non-permitted region.
    pub action: RuleAction,
    /// The subject's resolved region classification.
    pub region: RegionStatus,
}

/// Everything the engine needs to produce one decision.
///
/// Structural checks (account status) are always present; rule categories are
/// present exactly when the active policy enables a rule of that category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationRequest {
    /// Structural account state; always evaluated first.
    pub account_status: AccountStatus,
    /// Present when the active policy enables an `allowlist` rule.
    pub allowlist: Option<MembershipCheck>,
    /// Present when the active policy enables a `denylist` rule.
    pub denylist: Option<MatchCheck>,
    /// Present when the active policy enables a `sanctions` rule.
    pub sanctions: Option<MatchCheck>,
    /// Present when the active policy enables a `jurisdiction` rule.
    pub jurisdiction: Option<JurisdictionCheck>,
}

impl Default for EvaluationRequest {
    fn default() -> Self {
        Self {
            account_status: AccountStatus::Active,
            allowlist: None,
            denylist: None,
            sanctions: None,
            jurisdiction: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EvaluationRequest;
    use crate::rules::account_status::AccountStatus;

    #[test]
    fn default_request_has_no_rules_and_an_active_account() {
        let request = EvaluationRequest::default();
        assert_eq!(request.account_status, AccountStatus::Active);
        assert_eq!(request.allowlist, None);
        assert_eq!(request.denylist, None);
        assert_eq!(request.sanctions, None);
        assert_eq!(request.jurisdiction, None);
    }
}
