//! Rule model: identifiers, rule categories and rule definitions.
//!
//! A *rule* is a named, typed check that a policy version can enable. Rules
//! reference external state (allowlist membership, denylist entries, sanctions
//! datasets, jurisdiction status) by category; the engine evaluates enabled
//! rules in a fixed precedence order (see [`crate::evaluator`]).

/// Length of a rule (or policy) identifier in bytes.
///
/// Identifiers are fixed-width so they serialize deterministically to on-chain
/// storage and events without heap allocation. `"SANCTIONS-001"`-style
/// identifiers fit comfortably; longer identifiers are truncated.
pub const ID_LEN: usize = 32;

/// A fixed-width identifier for rules and policies.
///
/// [`RuleId`] compares byte-for-byte and never allocates. Construction from a
/// string pads with `0x00` and truncates at [`ID_LEN`] bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuleId([u8; ID_LEN]);

impl RuleId {
    /// Wrap the raw fixed-width bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; ID_LEN]) -> Self {
        Self(bytes)
    }

    /// Build an identifier from a string, truncating to [`ID_LEN`] bytes.
    ///
    /// Bytes beyond the first [`ID_LEN`] are dropped; shorter inputs are
    /// zero-padded. The name deliberately mirrors [`FromStr::from_str`] for
    /// ergonomics; the behavior differs (truncation instead of failure) so
    /// the trait itself is not implemented.
    #[allow(clippy::should_implement_trait)]
    #[must_use]
    pub fn from_str(input: &str) -> Self {
        let mut bytes = [0u8; ID_LEN];
        for (slot, byte) in bytes.iter_mut().zip(input.as_bytes()) {
            *slot = *byte;
        }
        Self(bytes)
    }

    /// Borrow the raw fixed-width bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; ID_LEN] {
        &self.0
    }

    /// The identifier as a byte slice trimmed of trailing zero padding.
    #[must_use]
    pub fn as_trimmed_bytes(&self) -> &[u8] {
        let mut end = self.0.len();
        while end > 0 && self.0[end - 1] == 0 {
            end -= 1;
        }
        &self.0[..end]
    }

    /// Whether the identifier is all zeros (unset).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        let mut i = 0;
        while i < ID_LEN {
            if self.0[i] != 0 {
                return false;
            }
            i += 1;
        }
        true
    }
}

impl From<&str> for RuleId {
    fn from(value: &str) -> Self {
        Self::from_str(value)
    }
}

/// The category of a compliance rule.
///
/// The engine evaluates enabled rules in fixed [`PRECEDENCE`] order: a rule
/// of a later category only runs if every earlier category passed. Every
/// category maps to a distinct registry/state source, which is what keeps the
/// precedence unambiguous and testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleType {
    /// Membership in the allowlist is required (or sufficient) for operations.
    Allowlist = 0,
    /// Presence in the denylist prohibits operations.
    Denylist = 1,
    /// A sanctions-screening match triggers the rule action.
    Sanctions = 2,
    /// The subject's jurisdiction must be permitted for this token.
    Jurisdiction = 3,
}

impl RuleType {
    /// The stable numeric representation, used in on-chain serialization.
    #[must_use]
    pub const fn to_code(self) -> u32 {
        self as u32
    }

    /// The stable lowercase label, used in JSON policy documents.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allowlist => "allowlist",
            Self::Denylist => "denylist",
            Self::Sanctions => "sanctions",
            Self::Jurisdiction => "jurisdiction",
        }
    }

    /// Reconstruct a [`RuleType`] from its stable numeric code.
    #[must_use]
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::Allowlist),
            1 => Some(Self::Denylist),
            2 => Some(Self::Sanctions),
            3 => Some(Self::Jurisdiction),
            _ => None,
        }
    }
}

/// What a rule does when its condition is met.
///
/// Policies choose per-rule severity: a sanctions match might `Block` under
/// one policy and merely `Flag` for review under another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleAction {
    /// Deny the operation: the evaluation resolves to [`crate::decision::Decision::Block`].
    Block = 0,
    /// Require review: the evaluation resolves to [`crate::decision::Decision::Flag`].
    Flag = 1,
}

impl RuleAction {
    /// The stable numeric representation, used in on-chain serialization.
    #[must_use]
    pub const fn to_code(self) -> u32 {
        self as u32
    }

    /// The stable lowercase label, used in JSON policy documents.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Flag => "flag",
        }
    }

    /// Reconstruct a [`RuleAction`] from its stable numeric code.
    #[must_use]
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::Block),
            1 => Some(Self::Flag),
            _ => None,
        }
    }
}

/// A named rule of a specific category with a configured action.
///
/// One policy version may enable at most one rule per [`RuleType`]; the
/// contract rejects rule sets that duplicate a category so evaluation always
/// maps unambiguously from category to rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    /// Identifier, unique within a policy version, e.g. `"SANCTIONS-001"`.
    pub id: RuleId,
    /// The rule category.
    pub rule_type: RuleType,
    /// The action taken when the rule's condition is met.
    pub action: RuleAction,
}

/// Fixed evaluation order across rule categories, documented in
/// `docs/rule-engine.md` and enforced by the evaluator and its tests.
///
/// ```text
/// Allowlist → Denylist → Sanctions → Jurisdiction
/// ```
///
/// Account-status checks are structural and always run first (a frozen
/// account is blocked regardless of what any rule says); see
/// [`crate::evaluator`].
pub const PRECEDENCE: [RuleType; 4] = [
    RuleType::Allowlist,
    RuleType::Denylist,
    RuleType::Sanctions,
    RuleType::Jurisdiction,
];

/// Rank of a rule category within [`PRECEDENCE`]: lower runs earlier.
#[must_use]
pub const fn precedence_rank(rule_type: RuleType) -> usize {
    match rule_type {
        RuleType::Allowlist => 0,
        RuleType::Denylist => 1,
        RuleType::Sanctions => 2,
        RuleType::Jurisdiction => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::{RuleId, ID_LEN};

    #[test]
    fn ids_are_truncated_and_padded_deterministically() {
        let short = RuleId::from_str("SANCTIONS-001");
        assert_eq!(
            short.as_trimmed_bytes(),
            b"SANCTIONS-001",
            "short ids keep their bytes"
        );
        assert_eq!(short.as_bytes().len(), ID_LEN);

        let exact = RuleId::from_str("12345678901234567890123456789012");
        assert_eq!(
            exact.as_trimmed_bytes(),
            b"12345678901234567890123456789012"
        );

        let long = RuleId::from_str("this-identifier-is-much-longer-than-thirty-two-bytes");
        assert_eq!(long.as_trimmed_bytes().len(), ID_LEN);
    }

    #[test]
    fn ids_compare_byte_for_byte() {
        assert_eq!(RuleId::from_str("a"), RuleId::from_str("a"));
        assert_ne!(RuleId::from_str("a"), RuleId::from_str("b"));
        // An all-zero id is the unset id and compares unequal to everything else.
        assert!(RuleId::from_str("").is_empty());
        assert!(RuleId::from_bytes([0u8; ID_LEN]).is_empty());
        assert!(!RuleId::from_str("a").is_empty());
    }

    #[test]
    fn from_str_impl_matches_from_str_helper() {
        assert_eq!(
            RuleId::from("ALLOWLIST-01"),
            RuleId::from_str("ALLOWLIST-01")
        );
    }
}

#[cfg(test)]
mod rule_type_tests {
    use super::{precedence_rank, PRECEDENCE};
    use crate::rule::{Rule, RuleAction, RuleId, RuleType};

    #[test]
    fn rule_types_round_trip() {
        for rule_type in [
            RuleType::Allowlist,
            RuleType::Denylist,
            RuleType::Sanctions,
            RuleType::Jurisdiction,
        ] {
            assert_eq!(RuleType::from_code(rule_type.to_code()), Some(rule_type));
        }
        assert_eq!(RuleType::from_code(99), None);
    }

    #[test]
    fn rule_type_labels_are_stable() {
        assert_eq!(RuleType::Allowlist.as_str(), "allowlist");
        assert_eq!(RuleType::Denylist.as_str(), "denylist");
        assert_eq!(RuleType::Sanctions.as_str(), "sanctions");
        assert_eq!(RuleType::Jurisdiction.as_str(), "jurisdiction");
    }

    #[test]
    fn rule_actions_round_trip() {
        assert_eq!(RuleAction::from_code(0), Some(RuleAction::Block));
        assert_eq!(RuleAction::from_code(1), Some(RuleAction::Flag));
        assert_eq!(RuleAction::from_code(2), None);
        assert_eq!(RuleAction::Block.as_str(), "block");
        assert_eq!(RuleAction::Flag.as_str(), "flag");
    }

    #[test]
    fn precedence_is_fixed_and_total() {
        assert_eq!(
            PRECEDENCE,
            [
                RuleType::Allowlist,
                RuleType::Denylist,
                RuleType::Sanctions,
                RuleType::Jurisdiction,
            ]
        );
        for (index, rule_type) in PRECEDENCE.iter().enumerate() {
            assert_eq!(precedence_rank(*rule_type), index);
        }
    }

    #[test]
    fn rule_equality_compares_id_type_and_action() {
        let rule = Rule {
            id: RuleId::from_str("SANCTIONS-001"),
            rule_type: RuleType::Sanctions,
            action: RuleAction::Block,
        };
        let same = rule;
        assert_eq!(rule, same);
        assert_ne!(
            rule,
            Rule {
                id: RuleId::from_str("SANCTIONS-002"),
                ..rule
            }
        );
        assert_ne!(
            rule,
            Rule {
                action: RuleAction::Flag,
                ..rule
            }
        );
    }
}
