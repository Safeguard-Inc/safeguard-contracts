//! Denylist rule: listed subjects are excluded from operations.
//!
//! When a policy enables a `denylist` rule, subjects present in the policy's
//! denylist trigger the rule's action. Lookup happens against the contract's
//! identity registry; this module only decides what a match means.

use crate::decision::{Decision, PolicyDecision, ReasonCode};
use crate::rule::RuleAction;

/// The check performed by an enabled `denylist` rule.
///
/// Returns `None` when the subject is not listed. A listed subject triggers
/// the rule's action with reason [`ReasonCode::DenylistMatch`]; the evaluator
/// attributes the decision to the rule.
#[must_use]
pub fn check(action: RuleAction, is_listed: bool) -> Option<PolicyDecision> {
    if !is_listed {
        return None;
    }
    let decision = match action {
        RuleAction::Block => Decision::Block,
        RuleAction::Flag => Decision::Flag,
    };
    Some(PolicyDecision::structural(
        decision,
        ReasonCode::DenylistMatch,
    ))
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::decision::{Decision, ReasonCode};
    use crate::rule::RuleAction;

    #[test]
    fn unlisted_subjects_pass() {
        assert_eq!(check(RuleAction::Block, false), None);
        assert_eq!(check(RuleAction::Flag, false), None);
    }

    #[test]
    fn listed_subjects_trigger_the_configured_action() {
        let blocked = check(RuleAction::Block, true).unwrap();
        assert_eq!(blocked.decision, Decision::Block);
        assert_eq!(blocked.reason_code, ReasonCode::DenylistMatch);

        let flagged = check(RuleAction::Flag, true).unwrap();
        assert_eq!(flagged.decision, Decision::Flag);
        assert_eq!(flagged.reason_code, ReasonCode::DenylistMatch);
    }
}
