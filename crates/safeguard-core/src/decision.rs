//! Decision model: outcomes, reason codes and the policy decision envelope.
//!
//! Every evaluation resolves to exactly one of three [`Decision`]s —
//! [`Approve`](Decision::Approve), [`Block`](Decision::Block) or
//! [`Flag`](Decision::Flag) — wrapped in a [`PolicyDecision`] together with
//! a machine-readable [`ReasonCode`] and, when a policy rule produced the
//! outcome, the identifier of the rule that triggered it.
//!
//! # Determinism and versioning
//!
//! The numeric codes in this module are **stable public API**. They are
//! explicitly assigned rather than derived from declaration order, and they
//! must never be renumbered, because `safeguard-hooks` and `safeguard-audit`
//! serialize them into on-chain events and audit records.

use crate::rule::RuleId;

/// The outcome of a policy evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Decision {
    /// The operation is permitted.
    Approve = 0,
    /// The operation is denied. Enforcement (actually refusing the transfer)
    /// is performed by `safeguard-hooks`, never by this crate.
    Block = 1,
    /// The operation needs review: it is neither clearly permitted nor
    /// clearly denied under the configured rules.
    Flag = 2,
}

impl Decision {
    /// The stable numeric representation, used in on-chain serialization.
    #[must_use]
    pub const fn to_code(self) -> u32 {
        self as u32
    }

    /// The stable ASCII label, used in JSON decision documents.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "APPROVE",
            Self::Block => "BLOCK",
            Self::Flag => "FLAG",
        }
    }

    /// Reconstruct a [`Decision`] from its stable numeric code.
    #[must_use]
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::Approve),
            1 => Some(Self::Block),
            2 => Some(Self::Flag),
            _ => None,
        }
    }
}

/// Machine-readable reason for a policy decision.
///
/// Codes are stable: new reasons are appended, never renumbered. See the
/// module documentation for the versioning contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReasonCode {
    /// No rule triggered. Used with [`Decision::Approve`].
    NoReason = 0,
    /// The account is frozen. Freezing mechanics integrate with SAC-compatible
    /// controls in the contract layer.
    AccountFrozen = 1,
    /// The account is suspended.
    AccountSuspended = 2,
    /// The account is restricted and needs review before operations.
    AccountRestricted = 3,
    /// The account's compliance status is unknown (fail-closed default).
    AccountStatusUnknown = 4,
    /// The policy requires allowlist membership and the subject is not a member.
    AllowlistRequired = 5,
    /// The subject matched the denylist.
    DenylistMatch = 6,
    /// The subject matched a sanctions screening entry.
    SanctionsMatch = 7,
    /// The subject's jurisdiction is prohibited for this token.
    JurisdictionProhibited = 8,
    /// The subject's jurisdiction is restricted for this token.
    JurisdictionRestricted = 9,
    /// The subject's jurisdiction could not be determined (fail-closed default).
    JurisdictionUnknown = 10,
}

impl ReasonCode {
    /// The stable numeric representation, used in on-chain serialization.
    #[must_use]
    pub const fn to_code(self) -> u32 {
        self as u32
    }

    /// The stable lowercase label, used in JSON decision documents.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoReason => "no_reason",
            Self::AccountFrozen => "account_frozen",
            Self::AccountSuspended => "account_suspended",
            Self::AccountRestricted => "account_restricted",
            Self::AccountStatusUnknown => "account_status_unknown",
            Self::AllowlistRequired => "allowlist_required",
            Self::DenylistMatch => "denylist_match",
            Self::SanctionsMatch => "sanctions_match",
            Self::JurisdictionProhibited => "jurisdiction_prohibited",
            Self::JurisdictionRestricted => "jurisdiction_restricted",
            Self::JurisdictionUnknown => "jurisdiction_unknown",
        }
    }

    /// Reconstruct a [`ReasonCode`] from its stable numeric code.
    #[must_use]
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::NoReason),
            1 => Some(Self::AccountFrozen),
            2 => Some(Self::AccountSuspended),
            3 => Some(Self::AccountRestricted),
            4 => Some(Self::AccountStatusUnknown),
            5 => Some(Self::AllowlistRequired),
            6 => Some(Self::DenylistMatch),
            7 => Some(Self::SanctionsMatch),
            8 => Some(Self::JurisdictionProhibited),
            9 => Some(Self::JurisdictionRestricted),
            10 => Some(Self::JurisdictionUnknown),
            _ => None,
        }
    }
}

/// The standardized result of a policy evaluation.
///
/// Carries everything `safeguard-hooks` and `safeguard-audit` need to act on
/// and to prove what happened:
///
/// ```text
/// PolicyDecision
/// ├── decision        APPROVE | BLOCK | FLAG
/// ├── reason_code     stable machine-readable cause
/// └── rule            rule id that triggered the outcome (if any)
/// ```
///
/// Policy id, policy version and timestamp are added by the caller (contract
/// or hook), because they are deployment context rather than engine output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyDecision {
    /// The outcome of the evaluation.
    pub decision: Decision,
    /// Stable machine-readable cause of the outcome.
    pub reason_code: ReasonCode,
    /// The rule that triggered the outcome, when a policy rule produced it.
    /// Account-status outcomes are structural and carry no rule reference.
    pub rule: Option<RuleId>,
}

impl PolicyDecision {
    /// An unconditional [`Decision::Approve`] with no triggering rule.
    #[must_use]
    pub const fn approve() -> Self {
        Self {
            decision: Decision::Approve,
            reason_code: ReasonCode::NoReason,
            rule: None,
        }
    }

    /// Build a decision from a triggering rule.
    #[must_use]
    pub const fn from_rule(decision: Decision, reason_code: ReasonCode, rule: RuleId) -> Self {
        Self {
            decision,
            reason_code,
            rule: Some(rule),
        }
    }

    /// Build a structural decision (for example account status) that is not
    /// attributed to a policy rule.
    #[must_use]
    pub const fn structural(decision: Decision, reason_code: ReasonCode) -> Self {
        Self {
            decision,
            reason_code,
            rule: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Decision, PolicyDecision, ReasonCode};

    #[test]
    fn decision_codes_round_trip() {
        for decision in [Decision::Approve, Decision::Block, Decision::Flag] {
            assert_eq!(Decision::from_code(decision.to_code()), Some(decision));
        }
        assert_eq!(Decision::from_code(42), None);
    }

    #[test]
    fn decision_labels_are_stable() {
        assert_eq!(Decision::Approve.as_str(), "APPROVE");
        assert_eq!(Decision::Block.as_str(), "BLOCK");
        assert_eq!(Decision::Flag.as_str(), "FLAG");
    }

    #[test]
    fn reason_codes_round_trip_and_have_unique_labels() {
        let mut seen = std::collections::BTreeSet::new();
        let all = [
            ReasonCode::NoReason,
            ReasonCode::AccountFrozen,
            ReasonCode::AccountSuspended,
            ReasonCode::AccountRestricted,
            ReasonCode::AccountStatusUnknown,
            ReasonCode::AllowlistRequired,
            ReasonCode::DenylistMatch,
            ReasonCode::SanctionsMatch,
            ReasonCode::JurisdictionProhibited,
            ReasonCode::JurisdictionRestricted,
            ReasonCode::JurisdictionUnknown,
        ];
        for code in all {
            assert_eq!(ReasonCode::from_code(code.to_code()), Some(code));
            assert!(
                seen.insert(code.as_str()),
                "duplicate label {}",
                code.as_str()
            );
        }
        assert_eq!(ReasonCode::from_code(99), None);
    }

    #[test]
    fn approve_decision_carries_no_reason_or_rule() {
        let d = PolicyDecision::approve();
        assert_eq!(d.decision, Decision::Approve);
        assert_eq!(d.reason_code, ReasonCode::NoReason);
        assert_eq!(d.rule, None);
    }
}
