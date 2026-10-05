//! Allowlist rule: membership required for operations.
//!
//! When a policy enables an `allowlist` rule, the account must be a member of
//! the policy's allowlist to proceed. Membership state is resolved by the
//! contract's identity registry and passed in as a boolean; this module only
//! decides what non-membership means under the rule's configured action.

use crate::decision::{Decision, PolicyDecision, ReasonCode};
use crate::rule::RuleAction;

/// The check performed by an enabled `allowlist` rule.
///
/// Returns `None` when the account is a member. A non-member triggers the
/// rule's action with reason [`ReasonCode::AllowlistRequired`]; the evaluator
/// attributes the decision to the rule.
#[must_use]
pub fn check(action: RuleAction, is_member: bool) -> Option<PolicyDecision> {
    if is_member {
        return None;
    }
    let decision = match action {
        RuleAction::Block => Decision::Block,
        RuleAction::Flag => Decision::Flag,
    };
    Some(PolicyDecision::structural(
        decision,
        ReasonCode::AllowlistRequired,
    ))
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::decision::{Decision, ReasonCode};
    use crate::rule::RuleAction;

    #[test]
    fn members_pass_regardless_of_action() {
        assert_eq!(check(RuleAction::Block, true), None);
        assert_eq!(check(RuleAction::Flag, true), None);
    }

    #[test]
    fn non_members_trigger_the_configured_action() {
        let blocked = check(RuleAction::Block, false).unwrap();
        assert_eq!(blocked.decision, Decision::Block);
        assert_eq!(blocked.reason_code, ReasonCode::AllowlistRequired);

        let flagged = check(RuleAction::Flag, false).unwrap();
        assert_eq!(flagged.decision, Decision::Flag);
        assert_eq!(flagged.reason_code, ReasonCode::AllowlistRequired);
    }
}
