//! Property tests for the evaluation engine.
//!
//! The unit tests enumerate the closed request space exhaustively; these
//! properties sample the **unbounded** space — arbitrary rule subsets with
//! arbitrary per-rule actions and arbitrary rule ids — and assert the
//! invariants that make the engine safe to run on-chain:
//!
//! * evaluation is total and deterministic;
//! * blocked subjects never evaluate to APPROVE;
//! * an APPROVE requires every enabled rule to have passed.
//!
//! proptest shrinks failing inputs to a minimal counterexample, which makes
//! a violated invariant far easier to debug than an exhaustive sweep would.

use proptest::prelude::*;

use safeguard_core::decision::Decision;
use safeguard_core::evaluation::{
    EvaluationRequest, JurisdictionCheck, MatchCheck, MembershipCheck,
};
use safeguard_core::rule::{RuleAction, RuleId};
use safeguard_core::rules::account_status::AccountStatus;
use safeguard_core::rules::jurisdiction::RegionStatus;

fn any_id() -> impl Strategy<Value = RuleId> {
    prop::array::uniform32(any::<u8>()).prop_map(RuleId::from_bytes)
}

fn any_action() -> impl Strategy<Value = RuleAction> {
    prop_oneof![Just(RuleAction::Block), Just(RuleAction::Flag)]
}

fn any_status() -> impl Strategy<Value = AccountStatus> {
    prop_oneof![
        Just(AccountStatus::Active),
        Just(AccountStatus::Restricted),
        Just(AccountStatus::Frozen),
        Just(AccountStatus::Suspended),
        Just(AccountStatus::Unknown),
    ]
}

fn any_region() -> impl Strategy<Value = RegionStatus> {
    prop_oneof![
        Just(RegionStatus::Permitted),
        Just(RegionStatus::Restricted),
        Just(RegionStatus::Prohibited),
        Just(RegionStatus::Unknown),
    ]
}

/// An arbitrary evaluation request: every rule category may be enabled or
/// absent, with independent actions, ids and conditions.
fn any_request() -> impl Strategy<Value = EvaluationRequest> {
    (
        any_status(),
        prop::option::of((any_id(), any_action(), any::<bool>())),
        prop::option::of((any_id(), any_action(), any::<bool>())),
        prop::option::of((any_id(), any_action(), any::<bool>())),
        prop::option::of((any_id(), any_action(), any_region())),
    )
        .prop_map(
            |(account_status, allowlist, denylist, sanctions, jurisdiction)| EvaluationRequest {
                account_status,
                allowlist: allowlist.map(|(rule_id, action, member)| MembershipCheck {
                    rule_id,
                    action,
                    member,
                }),
                denylist: denylist.map(|(rule_id, action, matched)| MatchCheck {
                    rule_id,
                    action,
                    matched,
                }),
                sanctions: sanctions.map(|(rule_id, action, matched)| MatchCheck {
                    rule_id,
                    action,
                    matched,
                }),
                jurisdiction: jurisdiction.map(|(rule_id, action, region)| JurisdictionCheck {
                    rule_id,
                    action,
                    region,
                }),
            },
        )
}

proptest! {
    /// The engine is total and deterministic: any request yields a decision,
    /// and the same request always yields the same decision.
    #[test]
    fn evaluation_is_total_and_deterministic(request in any_request()) {
        let first = safeguard_core::evaluator::evaluate(&request);
        for _ in 0..8 {
            prop_assert_eq!(
                safeguard_core::evaluator::evaluate(&request),
                first,
                "evaluation must be a pure function of its request"
            );
        }
    }

    /// Frozen and suspended accounts block regardless of any rule.
    #[test]
    fn frozen_and_suspended_accounts_always_block(
        request in any_request(),
        blocked in prop_oneof![Just(AccountStatus::Frozen), Just(AccountStatus::Suspended)],
    ) {
        let request = EvaluationRequest {
            account_status: blocked,
            ..request
        };
        let decision = safeguard_core::evaluator::evaluate(&request);
        prop_assert_eq!(decision.decision, Decision::Block);
    }

    /// Unknown account status never approves (fail-closed to review).
    #[test]
    fn unknown_account_status_never_approves(request in any_request()) {
        let request = EvaluationRequest {
            account_status: AccountStatus::Unknown,
            ..request
        };
        let decision = safeguard_core::evaluator::evaluate(&request);
        prop_assert_ne!(decision.decision, Decision::Approve);
    }

    /// A denylist match under a blocking rule never approves — either the
    /// denylist fires (Block) or an earlier structural/rule check already
    /// produced a non-approve outcome; APPROVE is impossible either way.
    #[test]
    fn blocking_denylist_matches_never_approve(
        request in any_request(),
        matched: bool,
        id in any_id(),
    ) {
        let request = EvaluationRequest {
            denylist: Some(MatchCheck {
                rule_id: id,
                action: RuleAction::Block,
                matched,
            }),
            ..request
        };
        let decision = safeguard_core::evaluator::evaluate(&request);
        if matched {
            prop_assert_ne!(decision.decision, Decision::Approve);
        }
    }

    /// An unknown region under an enabled jurisdiction rule never approves:
    /// the rule action fires (fail-closed), so the result is Block or Flag.
    #[test]
    fn unknown_region_never_approves(request in any_request(), action in any_action(), id in any_id()) {
        let request = EvaluationRequest {
            jurisdiction: Some(JurisdictionCheck {
                rule_id: id,
                action,
                region: RegionStatus::Unknown,
            }),
            ..request
        };
        let decision = safeguard_core::evaluator::evaluate(&request);
        prop_assert_ne!(decision.decision, Decision::Approve);
    }

    /// A non-member under a blocking allowlist never approves.
    #[test]
    fn blocking_allowlist_non_members_never_approve(request in any_request(), id in any_id()) {
        let request = EvaluationRequest {
            allowlist: Some(MembershipCheck {
                rule_id: id,
                action: RuleAction::Block,
                member: false,
            }),
            ..request
        };
        let decision = safeguard_core::evaluator::evaluate(&request);
        prop_assert_ne!(decision.decision, Decision::Approve);
    }

    /// APPROVE is only reachable when every enabled rule passed: an active
    /// account and no triggering rule. If the engine approves while some
    /// rule should have fired, that is a fail-open leak this test catches.
    #[test]
    fn approve_requires_every_rule_to_pass(request in any_request()) {
        let decision = safeguard_core::evaluator::evaluate(&request);
        if decision.decision == Decision::Approve {
            prop_assert_eq!(request.account_status, AccountStatus::Active);
            if let Some(allowlist) = request.allowlist {
                prop_assert!(allowlist.member, "approve despite allowlist non-member");
            }
            if let Some(denylist) = request.denylist {
                prop_assert!(!denylist.matched, "approve despite denylist match");
            }
            if let Some(sanctions) = request.sanctions {
                prop_assert!(!sanctions.matched, "approve despite sanctions match");
            }
            if let Some(jurisdiction) = request.jurisdiction {
                prop_assert_eq!(jurisdiction.region, RegionStatus::Permitted);
            }
        }
    }
}
