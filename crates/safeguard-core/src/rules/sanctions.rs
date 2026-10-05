//! Sanctions screening rule: matched subjects are excluded or flagged.
//!
//! Safeguard is **not** an official sanctions-data provider. Normalized
//! sanctions datasets enter the system through adapters (off-chain sources
//! converted to deterministic registry/attestation state) and the contract's
//! sanctions registry performs the lookup. This module only decides what a
//! match means under the rule's configured action.
//!
//! The important property is that a sanctions match under a blocking policy
//! must **never** evaluate as APPROVE — enforced here by construction and by
//! the evaluator property tests.

use crate::decision::{Decision, PolicyDecision, ReasonCode};
use crate::rule::RuleAction;

/// The check performed by an enabled `sanctions` rule.
///
/// Returns `None` when the subject did not match the screened dataset. A
/// match triggers the rule's action with reason
/// [`ReasonCode::SanctionsMatch`]; the evaluator attributes the decision to
/// the rule.
#[must_use]
pub fn check(action: RuleAction, matched: bool) -> Option<PolicyDecision> {
    if !matched {
        return None;
    }
    let decision = match action {
        RuleAction::Block => Decision::Block,
        RuleAction::Flag => Decision::Flag,
    };
    Some(PolicyDecision::structural(
        decision,
        ReasonCode::SanctionsMatch,
    ))
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::decision::{Decision, ReasonCode};
    use crate::rule::RuleAction;

    #[test]
    fn unmatched_subjects_pass() {
        assert_eq!(check(RuleAction::Block, false), None);
        assert_eq!(check(RuleAction::Flag, false), None);
    }

    #[test]
    fn matched_subjects_trigger_the_configured_action() {
        let blocked = check(RuleAction::Block, true).unwrap();
        assert_eq!(blocked.decision, Decision::Block);
        assert_eq!(blocked.reason_code, ReasonCode::SanctionsMatch);

        let flagged = check(RuleAction::Flag, true).unwrap();
        assert_eq!(flagged.decision, Decision::Flag);
        assert_eq!(flagged.reason_code, ReasonCode::SanctionsMatch);
    }

    #[test]
    fn a_match_never_approves() {
        // Property: a sanctions match must never evaluate as APPROVE under a
        // policy configured to screen sanctions, regardless of action.
        for action in [RuleAction::Block, RuleAction::Flag] {
            let verdict = check(action, true).expect("a match always produces a decision");
            assert_ne!(verdict.decision, Decision::Approve);
        }
    }
}
