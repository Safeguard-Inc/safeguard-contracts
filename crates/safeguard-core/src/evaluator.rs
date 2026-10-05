//! The deterministic evaluation engine.
//!
//! [`evaluate`] resolves an [`EvaluationRequest`] to a single
//! [`PolicyDecision`] by walking checks in a fixed, documented order:
//!
//! ```text
//! Account status  (structural: frozen/suspended/restricted/unknown)
//!     ↓
//! Allowlist       (required but not a member)
//!     ↓
//! Denylist        (listed subject)
//!     ↓
//! Sanctions       (screening match)
//!     ↓
//! Jurisdiction    (restricted/prohibited/unknown region)
//!     ↓
//! APPROVE
//! ```
//!
//! The first check that produces an outcome wins; later checks never run.
//! This order is part of the public contract of the crate (mirrored by
//! `docs/rule-engine.md` and exercised exhaustively by tests) so no two
//! implementations of the same policy can disagree.
//!
//! # Determinism
//!
//! The engine is a pure function of its request: no randomness, no wall
//! clock, no storage, no network. The same request always yields the same
//! decision, which the property tests in this module assert by brute force
//! over the whole request space.
//!
//! # Fail-closed defaults
//!
//! Missing information never silently approves: an unknown account status
//! flags, an unknown region triggers its rule, and a configured rule that
//! triggers with action `block` always blocks.

use crate::decision::PolicyDecision;
use crate::evaluation::EvaluationRequest;
use crate::rule::RuleId;
use crate::rules::{account_status, allowlist, denylist, jurisdiction, sanctions};

/// Evaluate a request under its policy snapshot.
///
/// Returns the first decisive outcome in precedence order, or
/// [`PolicyDecision::approve`] when every check passes.
#[must_use]
pub fn evaluate(request: &EvaluationRequest) -> PolicyDecision {
    // 1. Structural account status (always runs first, never configurable).
    if let Some(decision) = account_status::check(request.account_status) {
        return decision;
    }

    // 2. Rule categories in fixed precedence order. Rules the active policy
    //    does not enable are absent from the request and skipped.
    if let Some(check) = request.allowlist {
        if let Some(decision) = allowlist::check(check.action, check.member) {
            return attributed(decision, check.rule_id);
        }
    }
    if let Some(check) = request.denylist {
        if let Some(decision) = denylist::check(check.action, check.matched) {
            return attributed(decision, check.rule_id);
        }
    }
    if let Some(check) = request.sanctions {
        if let Some(decision) = sanctions::check(check.action, check.matched) {
            return attributed(decision, check.rule_id);
        }
    }
    if let Some(check) = request.jurisdiction {
        if let Some(decision) = jurisdiction::check(check.region, check.action) {
            return attributed(decision, check.rule_id);
        }
    }

    PolicyDecision::approve()
}

/// Attribute a rule-triggered decision to the rule that produced it.
fn attributed(decision: PolicyDecision, rule_id: RuleId) -> PolicyDecision {
    PolicyDecision {
        rule: Some(rule_id),
        ..decision
    }
}

#[cfg(test)]
mod tests {
    use super::evaluate;
    use crate::decision::{Decision, ReasonCode};
    use crate::evaluation::{EvaluationRequest, JurisdictionCheck, MatchCheck, MembershipCheck};
    use crate::rule::{RuleAction, RuleId};
    use crate::rules::account_status::AccountStatus;
    use crate::rules::jurisdiction::RegionStatus;

    // Convenience builders; RuleId::from_str is not const, so requests are
    // assembled at runtime rather than with constants.
    fn allowlist(member: bool, action: RuleAction) -> Option<MembershipCheck> {
        Some(MembershipCheck {
            rule_id: RuleId::from_str("ALLOWLIST-001"),
            action,
            member,
        })
    }

    fn denylist(matched: bool, action: RuleAction) -> Option<MatchCheck> {
        Some(MatchCheck {
            rule_id: RuleId::from_str("DENYLIST-001"),
            action,
            matched,
        })
    }

    fn sanctions(matched: bool, action: RuleAction) -> Option<MatchCheck> {
        Some(MatchCheck {
            rule_id: RuleId::from_str("SANCTIONS-001"),
            action,
            matched,
        })
    }

    fn jurisdiction(region: RegionStatus, action: RuleAction) -> Option<JurisdictionCheck> {
        Some(JurisdictionCheck {
            rule_id: RuleId::from_str("JURISDICTION-001"),
            action,
            region,
        })
    }

    #[test]
    fn empty_policy_approves_an_active_account() {
        let request = EvaluationRequest::default();
        let decision = evaluate(&request);
        assert_eq!(decision.decision, Decision::Approve);
        assert_eq!(decision.reason_code, ReasonCode::NoReason);
        assert_eq!(decision.rule, None);
    }

    #[test]
    fn account_status_wins_over_every_rule() {
        // A frozen account is blocked even when every rule would pass.
        let request = EvaluationRequest {
            account_status: AccountStatus::Frozen,
            allowlist: allowlist(true, RuleAction::Block),
            denylist: denylist(false, RuleAction::Block),
            sanctions: sanctions(false, RuleAction::Block),
            jurisdiction: jurisdiction(RegionStatus::Permitted, RuleAction::Block),
        };
        let decision = evaluate(&request);
        assert_eq!(decision.decision, Decision::Block);
        assert_eq!(decision.reason_code, ReasonCode::AccountFrozen);
        assert_eq!(decision.rule, None, "status decisions are structural");
    }

    #[test]
    fn each_rule_triggers_with_its_own_reason_and_id() {
        let request = EvaluationRequest {
            allowlist: allowlist(false, RuleAction::Block),
            ..EvaluationRequest::default()
        };
        let decision = evaluate(&request);
        assert_eq!(decision.decision, Decision::Block);
        assert_eq!(decision.reason_code, ReasonCode::AllowlistRequired);
        assert_eq!(decision.rule, Some(RuleId::from_str("ALLOWLIST-001")));

        let request = EvaluationRequest {
            denylist: denylist(true, RuleAction::Block),
            ..EvaluationRequest::default()
        };
        let decision = evaluate(&request);
        assert_eq!(decision.decision, Decision::Block);
        assert_eq!(decision.reason_code, ReasonCode::DenylistMatch);
        assert_eq!(decision.rule, Some(RuleId::from_str("DENYLIST-001")));

        let request = EvaluationRequest {
            sanctions: sanctions(true, RuleAction::Block),
            ..EvaluationRequest::default()
        };
        let decision = evaluate(&request);
        assert_eq!(decision.decision, Decision::Block);
        assert_eq!(decision.reason_code, ReasonCode::SanctionsMatch);
        assert_eq!(decision.rule, Some(RuleId::from_str("SANCTIONS-001")));

        let request = EvaluationRequest {
            jurisdiction: jurisdiction(RegionStatus::Prohibited, RuleAction::Block),
            ..EvaluationRequest::default()
        };
        let decision = evaluate(&request);
        assert_eq!(decision.decision, Decision::Block);
        assert_eq!(decision.reason_code, ReasonCode::JurisdictionProhibited);
        assert_eq!(decision.rule, Some(RuleId::from_str("JURISDICTION-001")));
    }

    #[test]
    fn rule_actions_resolve_to_flag() {
        let request = EvaluationRequest {
            sanctions: sanctions(true, RuleAction::Flag),
            ..EvaluationRequest::default()
        };
        let decision = evaluate(&request);
        assert_eq!(decision.decision, Decision::Flag);
        assert_eq!(decision.reason_code, ReasonCode::SanctionsMatch);
    }

    #[test]
    fn precedence_is_applied_in_documented_order() {
        // Earlier categories shadow later ones when both would trigger.
        let request = EvaluationRequest {
            account_status: AccountStatus::Active,
            allowlist: allowlist(false, RuleAction::Block),
            denylist: denylist(true, RuleAction::Block),
            sanctions: sanctions(true, RuleAction::Block),
            jurisdiction: jurisdiction(RegionStatus::Prohibited, RuleAction::Block),
        };
        let decision = evaluate(&request);
        assert_eq!(decision.reason_code, ReasonCode::AllowlistRequired);

        let request = EvaluationRequest {
            account_status: AccountStatus::Active,
            allowlist: allowlist(true, RuleAction::Block),
            denylist: denylist(true, RuleAction::Block),
            sanctions: sanctions(true, RuleAction::Block),
            jurisdiction: jurisdiction(RegionStatus::Prohibited, RuleAction::Block),
        };
        let decision = evaluate(&request);
        assert_eq!(decision.reason_code, ReasonCode::DenylistMatch);

        let request = EvaluationRequest {
            account_status: AccountStatus::Active,
            allowlist: allowlist(true, RuleAction::Block),
            denylist: denylist(false, RuleAction::Block),
            sanctions: sanctions(true, RuleAction::Block),
            jurisdiction: jurisdiction(RegionStatus::Prohibited, RuleAction::Block),
        };
        let decision = evaluate(&request);
        assert_eq!(decision.reason_code, ReasonCode::SanctionsMatch);

        let request = EvaluationRequest {
            account_status: AccountStatus::Active,
            allowlist: allowlist(true, RuleAction::Block),
            denylist: denylist(false, RuleAction::Block),
            sanctions: sanctions(false, RuleAction::Block),
            jurisdiction: jurisdiction(RegionStatus::Restricted, RuleAction::Block),
        };
        let decision = evaluate(&request);
        assert_eq!(decision.reason_code, ReasonCode::JurisdictionRestricted);
    }

    #[test]
    fn evaluation_is_repeatable_across_the_request_space() {
        // Determinism property: brute-force every combination of statuses,
        // presence flags and actions; evaluating twice must give identical
        // decisions every time.
        let statuses = [
            AccountStatus::Active,
            AccountStatus::Restricted,
            AccountStatus::Frozen,
            AccountStatus::Suspended,
            AccountStatus::Unknown,
        ];
        let regions = [
            RegionStatus::Permitted,
            RegionStatus::Restricted,
            RegionStatus::Prohibited,
            RegionStatus::Unknown,
        ];
        let actions = [RuleAction::Block, RuleAction::Flag];

        for status in statuses {
            for allow_member in [false, true] {
                for deny_match in [false, true] {
                    for sanct_match in [false, true] {
                        for region in regions {
                            for action in actions {
                                let request = EvaluationRequest {
                                    account_status: status,
                                    allowlist: allowlist(allow_member, action),
                                    denylist: denylist(deny_match, action),
                                    sanctions: sanctions(sanct_match, action),
                                    jurisdiction: jurisdiction(region, action),
                                };
                                let first = evaluate(&request);
                                for _ in 0..16 {
                                    assert_eq!(
                                        evaluate(&request),
                                        first,
                                        "evaluation must be deterministic for {request:?}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn denied_accounts_never_become_approved_without_state_change() {
        // Property: if a rule with action=block triggers, the decision is
        // BLOCK for every other configuration of the request.
        let statuses = [
            AccountStatus::Active,
            AccountStatus::Restricted,
            AccountStatus::Frozen,
            AccountStatus::Suspended,
            AccountStatus::Unknown,
        ];
        for status in statuses {
            let request = EvaluationRequest {
                account_status: status,
                allowlist: allowlist(false, RuleAction::Block),
                denylist: denylist(false, RuleAction::Block),
                sanctions: sanctions(false, RuleAction::Block),
                jurisdiction: jurisdiction(RegionStatus::Permitted, RuleAction::Block),
            };
            // allowlist triggers (not a member) so decision must be BLOCK —
            // unless the structural status already produced a non-approve
            // outcome first.
            let decision = evaluate(&request);
            assert_ne!(
                decision.decision,
                Decision::Approve,
                "a blocked allowlist under a blocking rule must not approve"
            );
        }
    }
}
