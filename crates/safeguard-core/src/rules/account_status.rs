//! Compliance-relevant account state.
//!
//! Account status is the **structural** layer of evaluation: it is not a
//! configurable rule because the semantics are (and must stay) uniform —
//! a frozen account is blocked, full stop. The actual freezing mechanics
//! integrate with Stellar/SAC-compatible controls in the contract layer; this
//! module only decides what a given status means for a policy decision.

use crate::decision::{Decision, PolicyDecision, ReasonCode};

/// Compliance-relevant state of an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccountStatus {
    /// The account may operate.
    Active = 0,
    /// The account is restricted: operations need review.
    Restricted = 1,
    /// The account is frozen: operations are denied.
    Frozen = 2,
    /// The account is suspended: operations are denied.
    Suspended = 3,
    /// The compliance status could not be determined.
    Unknown = 4,
}

impl AccountStatus {
    /// The stable numeric representation, used in on-chain serialization.
    #[must_use]
    pub const fn to_code(self) -> u32 {
        self as u32
    }

    /// The stable lowercase label, used in JSON documents.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Restricted => "restricted",
            Self::Frozen => "frozen",
            Self::Suspended => "suspended",
            Self::Unknown => "unknown",
        }
    }

    /// Reconstruct an [`AccountStatus`] from its stable numeric code.
    #[must_use]
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::Active),
            1 => Some(Self::Restricted),
            2 => Some(Self::Frozen),
            3 => Some(Self::Suspended),
            4 => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// The structural decision for an account status.
///
/// Returns `None` when the account may proceed ([`AccountStatus::Active`]).
///
/// Fail-closed semantics: anything other than a confirmed active status
/// either blocks (frozen/suspended) or forces human review (restricted,
/// unknown). An unknown status must never silently approve.
///
/// | Status      | Outcome | Reason                |
/// | ----------- | ------- | --------------------- |
/// | Active      | pass    | —                     |
/// | Restricted  | FLAG    | `account_restricted`  |
/// | Frozen      | BLOCK   | `account_frozen`      |
/// | Suspended   | BLOCK   | `account_suspended`   |
/// | Unknown     | FLAG    | `account_status_unknown` |
#[must_use]
pub fn check(status: AccountStatus) -> Option<PolicyDecision> {
    match status {
        AccountStatus::Active => None,
        AccountStatus::Restricted => Some(PolicyDecision::structural(
            Decision::Flag,
            ReasonCode::AccountRestricted,
        )),
        AccountStatus::Frozen => Some(PolicyDecision::structural(
            Decision::Block,
            ReasonCode::AccountFrozen,
        )),
        AccountStatus::Suspended => Some(PolicyDecision::structural(
            Decision::Block,
            ReasonCode::AccountSuspended,
        )),
        AccountStatus::Unknown => Some(PolicyDecision::structural(
            Decision::Flag,
            ReasonCode::AccountStatusUnknown,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{check, AccountStatus};
    use crate::decision::{Decision, ReasonCode};

    #[test]
    fn statuses_round_trip() {
        for status in [
            AccountStatus::Active,
            AccountStatus::Restricted,
            AccountStatus::Frozen,
            AccountStatus::Suspended,
            AccountStatus::Unknown,
        ] {
            assert_eq!(AccountStatus::from_code(status.to_code()), Some(status));
        }
        assert_eq!(AccountStatus::from_code(99), None);
    }

    #[test]
    fn status_labels_are_stable() {
        assert_eq!(AccountStatus::Active.as_str(), "active");
        assert_eq!(AccountStatus::Frozen.as_str(), "frozen");
        assert_eq!(AccountStatus::Unknown.as_str(), "unknown");
    }

    #[test]
    fn active_accounts_pass() {
        assert_eq!(check(AccountStatus::Active), None);
    }

    #[test]
    fn frozen_and_suspended_accounts_block() {
        let frozen = check(AccountStatus::Frozen).expect("frozen must produce a decision");
        assert_eq!(frozen.decision, Decision::Block);
        assert_eq!(frozen.reason_code, ReasonCode::AccountFrozen);
        assert_eq!(
            frozen.rule, None,
            "account status is structural, not rule-based"
        );

        let suspended = check(AccountStatus::Suspended).expect("suspended must produce a decision");
        assert_eq!(suspended.decision, Decision::Block);
        assert_eq!(suspended.reason_code, ReasonCode::AccountSuspended);
    }

    #[test]
    fn restricted_and_unknown_accounts_flag() {
        let restricted = check(AccountStatus::Restricted).expect("restricted must flag");
        assert_eq!(restricted.decision, Decision::Flag);
        assert_eq!(restricted.reason_code, ReasonCode::AccountRestricted);

        // Unknown must never approve silently.
        let unknown = check(AccountStatus::Unknown).expect("unknown must flag");
        assert_eq!(unknown.decision, Decision::Flag);
        assert_eq!(unknown.reason_code, ReasonCode::AccountStatusUnknown);
    }
}
