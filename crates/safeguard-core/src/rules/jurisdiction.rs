//! Jurisdiction rules: permitted, restricted and prohibited regions.
//!
//! A policy may enable a single `jurisdiction` rule. The rule's action
//! (`block` or `flag`) decides how a non-permitted region is treated. Because
//! the subject's region is resolved off-chain (identity attestation,
//! geo-location provider, registry entry) it is passed in as a snapshot value;
//! this module never touches external services.
//!
//! Fail-closed posture: an **unknown** region triggers the rule action rather
//! than passing, so missing jurisdiction information can never silently
//! approve a restricted-flow token operation.

use crate::decision::{Decision, PolicyDecision, ReasonCode};
use crate::rule::RuleAction;

/// Compliance classification of a region for a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegionStatus {
    /// The region may hold and transact the token.
    Permitted = 0,
    /// The region is restricted for this token.
    Restricted = 1,
    /// The region is prohibited for this token.
    Prohibited = 2,
    /// The region could not be determined.
    Unknown = 3,
}

impl RegionStatus {
    /// The stable numeric representation, used in on-chain serialization.
    #[must_use]
    pub const fn to_code(self) -> u32 {
        self as u32
    }

    /// The stable lowercase label, used in JSON documents.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Permitted => "permitted",
            Self::Restricted => "restricted",
            Self::Prohibited => "prohibited",
            Self::Unknown => "unknown",
        }
    }

    /// Reconstruct a [`RegionStatus`] from its stable numeric code.
    #[must_use]
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::Permitted),
            1 => Some(Self::Restricted),
            2 => Some(Self::Prohibited),
            3 => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// The check performed by an enabled `jurisdiction` rule.
///
/// Returns `None` when the region is permitted. Otherwise returns a decision
/// whose severity follows the rule's action, attributed to the rule by the
/// evaluator.
///
/// | Region      | action `block` | action `flag`  |
/// | ----------- | -------------- | -------------- |
/// | Permitted   | pass           | pass           |
/// | Restricted  | BLOCK          | FLAG           |
/// | Prohibited  | BLOCK          | FLAG           |
/// | Unknown     | BLOCK          | FLAG           |
#[must_use]
pub fn check(region: RegionStatus, action: RuleAction) -> Option<PolicyDecision> {
    let reason = match region {
        RegionStatus::Permitted => return None,
        RegionStatus::Restricted => ReasonCode::JurisdictionRestricted,
        RegionStatus::Prohibited => ReasonCode::JurisdictionProhibited,
        RegionStatus::Unknown => ReasonCode::JurisdictionUnknown,
    };
    let decision = match action {
        RuleAction::Block => Decision::Block,
        RuleAction::Flag => Decision::Flag,
    };
    Some(PolicyDecision::structural(decision, reason))
}

#[cfg(test)]
mod tests {
    use super::{check, RegionStatus};
    use crate::decision::{Decision, ReasonCode};
    use crate::rule::RuleAction;

    #[test]
    fn regions_round_trip() {
        for region in [
            RegionStatus::Permitted,
            RegionStatus::Restricted,
            RegionStatus::Prohibited,
            RegionStatus::Unknown,
        ] {
            assert_eq!(RegionStatus::from_code(region.to_code()), Some(region));
        }
        assert_eq!(RegionStatus::from_code(99), None);
    }

    #[test]
    fn region_labels_are_stable() {
        assert_eq!(RegionStatus::Permitted.as_str(), "permitted");
        assert_eq!(RegionStatus::Restricted.as_str(), "restricted");
        assert_eq!(RegionStatus::Prohibited.as_str(), "prohibited");
        assert_eq!(RegionStatus::Unknown.as_str(), "unknown");
    }

    #[test]
    fn permitted_regions_always_pass() {
        assert_eq!(check(RegionStatus::Permitted, RuleAction::Block), None);
        assert_eq!(check(RegionStatus::Permitted, RuleAction::Flag), None);
    }

    #[test]
    fn block_action_blocks_every_non_permitted_region() {
        let restricted = check(RegionStatus::Restricted, RuleAction::Block).unwrap();
        assert_eq!(restricted.decision, Decision::Block);
        assert_eq!(restricted.reason_code, ReasonCode::JurisdictionRestricted);

        let prohibited = check(RegionStatus::Prohibited, RuleAction::Block).unwrap();
        assert_eq!(prohibited.decision, Decision::Block);
        assert_eq!(prohibited.reason_code, ReasonCode::JurisdictionProhibited);

        // Unknown must never pass: under a blocking rule it blocks.
        let unknown = check(RegionStatus::Unknown, RuleAction::Block).unwrap();
        assert_eq!(unknown.decision, Decision::Block);
        assert_eq!(unknown.reason_code, ReasonCode::JurisdictionUnknown);
    }

    #[test]
    fn flag_action_flags_non_permitted_regions() {
        for region in [
            RegionStatus::Restricted,
            RegionStatus::Prohibited,
            RegionStatus::Unknown,
        ] {
            let verdict = check(region, RuleAction::Flag).unwrap();
            assert_eq!(verdict.decision, Decision::Flag);
        }
    }
}
