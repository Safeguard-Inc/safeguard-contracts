//! Policy configuration: the rule set a policy version enforces.
//!
//! A policy version's configuration is its rule set. Because the engine
//! allows at most one rule per category (see [`crate::evaluator`] precedence),
//! a valid rule set has at most [`crate::rule::PRECEDENCE`]`::len()` rules,
//! one per category, with unique ids. The contract stores and the schema
//! describes rule sets in this normalized shape; [`RuleSet`] validates and
//! normalizes on insertion so malformed configurations cannot be registered.

use crate::rule::{precedence_rank, Rule, RuleId, RuleType, PRECEDENCE};

/// A configuration rule-set error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSetError {
    /// A rule of this category already exists in the set.
    DuplicateCategory,
    /// A rule with this id already exists in the set.
    DuplicateId,
}

/// Normalized rule set: at most one rule per category, indexed by precedence.
///
/// Index 0 is the highest-precedence category; iterating the set yields rules
/// in evaluation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleSet([Option<Rule>; PRECEDENCE.len()]);

impl RuleSet {
    /// An empty rule set (no rules enabled).
    #[must_use]
    pub const fn empty() -> Self {
        Self([None; PRECEDENCE.len()])
    }

    /// The rule of a category, if enabled.
    #[must_use]
    pub fn get(&self, rule_type: RuleType) -> Option<&Rule> {
        self.0[precedence_rank(rule_type)].as_ref()
    }

    /// Iterate enabled rules in evaluation (precedence) order.
    pub fn iter(&self) -> impl Iterator<Item = &Rule> {
        self.0.iter().flatten()
    }

    /// Number of enabled rules.
    #[must_use]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Whether no rules are enabled.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    /// Insert a rule, rejecting duplicates of a category or an id.
    ///
    /// Returns the previous rule of the category (if any) on success, so
    /// callers can treat insertion as replacement when they want to.
    pub fn insert(&mut self, rule: Rule) -> Result<Option<Rule>, RuleSetError> {
        for existing in self.iter() {
            if existing.id == rule.id {
                return Err(RuleSetError::DuplicateId);
            }
        }
        let slot = &mut self.0[precedence_rank(rule.rule_type)];
        if slot.is_some() {
            return Err(RuleSetError::DuplicateCategory);
        }
        Ok(slot.replace(rule))
    }
}

impl Default for RuleSet {
    fn default() -> Self {
        Self::empty()
    }
}

/// The configuration of one policy version: identity plus its rule set.
///
/// The rule set's serialized digest is the version's [`crate::version::ConfigHash`];
/// see [`crate::version::PolicyVersionInfo`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyConfig {
    /// The policy this configuration belongs to.
    pub policy_id: RuleId,
    /// The version number within the policy.
    pub version: u32,
    /// The enabled rules, normalized one-per-category.
    pub rule_set: RuleSet,
}

impl PolicyConfig {
    /// Create an (initially empty) configuration for a policy version.
    #[must_use]
    pub const fn new(policy_id: RuleId, version: u32) -> Self {
        Self {
            policy_id,
            version,
            rule_set: RuleSet::empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PolicyConfig, RuleSet, RuleSetError};
    use crate::rule::{Rule, RuleAction, RuleId, RuleType};

    fn rule(id: &str, rule_type: RuleType, action: RuleAction) -> Rule {
        Rule {
            id: RuleId::from_str(id),
            rule_type,
            action,
        }
    }

    #[test]
    fn empty_rule_set_has_no_rules() {
        let set = RuleSet::empty();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        assert_eq!(set.get(RuleType::Sanctions), None);
    }

    #[test]
    fn insert_normalizes_by_category_and_rejects_duplicates() {
        let mut set = RuleSet::empty();
        set.insert(rule("ALLOW-1", RuleType::Allowlist, RuleAction::Block))
            .unwrap();
        assert_eq!(set.len(), 1);

        // Same category rejected regardless of id.
        assert_eq!(
            set.insert(rule("ALLOW-2", RuleType::Allowlist, RuleAction::Flag)),
            Err(RuleSetError::DuplicateCategory)
        );

        // Same id rejected across categories.
        assert_eq!(
            set.insert(rule("ALLOW-1", RuleType::Sanctions, RuleAction::Block)),
            Err(RuleSetError::DuplicateId)
        );
    }

    #[test]
    fn full_rule_set_is_iterated_in_precedence_order() {
        let mut set = RuleSet::empty();
        for (id, rule_type) in [
            ("JUR-1", RuleType::Jurisdiction),
            ("SANCT-1", RuleType::Sanctions),
            ("DENY-1", RuleType::Denylist),
            ("ALLOW-1", RuleType::Allowlist),
        ] {
            set.insert(rule(id, rule_type, RuleAction::Block)).unwrap();
        }
        assert_eq!(set.len(), 4);
        // no_std: compare against the expected precedence directly.
        let expected = [
            RuleType::Allowlist,
            RuleType::Denylist,
            RuleType::Sanctions,
            RuleType::Jurisdiction,
        ];
        for (index, r) in set.iter().enumerate() {
            assert_eq!(r.rule_type, expected[index]);
        }
    }

    #[test]
    fn policy_config_tracks_identity_and_version() {
        let config = PolicyConfig::new(RuleId::from_str("institutional-default"), 2);
        assert_eq!(config.policy_id, RuleId::from_str("institutional-default"));
        assert_eq!(config.version, 2);
        assert!(config.rule_set.is_empty());
    }
}
