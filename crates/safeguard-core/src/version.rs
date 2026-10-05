//! Policy versioning: a policy never mutates silently.
//!
//! Every change to a policy is a new immutable version:
//!
//! ```text
//! Policy v1 ─▶ Policy v2 ─▶ Policy v3
//! ```
//!
//! A version records its policy, version number, lifecycle status and the
//! hash of its configuration, so configuration changes are auditable and the
//! exact rule set that produced a decision can be proven later by
//! `safeguard-audit`.
//!
//! Lifecycle transitions are pure functions here; the contract enforces that
//! only an authorized administrator can drive them.

use crate::rule::RuleId;

/// Length of a configuration hash in bytes (matches SHA-256).
pub const CONFIG_HASH_LEN: usize = 32;

/// Hash of a policy version's configuration.
///
/// The caller computes the digest (the contract hashes the serialized rule
/// set with the host's crypto); this crate treats it as opaque bytes that
/// uniquely identify configuration content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConfigHash([u8; CONFIG_HASH_LEN]);

impl ConfigHash {
    /// Wrap the raw digest bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; CONFIG_HASH_LEN]) -> Self {
        Self(bytes)
    }

    /// Borrow the raw digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; CONFIG_HASH_LEN] {
        &self.0
    }
}

/// Lifecycle status of a policy version.
///
/// ```text
/// Draft ──activate──▶ Active ──supersede──▶ Superseded
///    │                    │
///    └───disable──────────┴──────▶ Disabled
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VersionStatus {
    /// Registered but not yet in force. Only drafts may be activated.
    Draft = 0,
    /// The version currently in force for its policy.
    Active = 1,
    /// Was active, replaced by a newer activated version.
    Superseded = 2,
    /// Deactivated by an administrator.
    Disabled = 3,
}

impl VersionStatus {
    /// The stable numeric representation, used in on-chain serialization.
    #[must_use]
    pub const fn to_code(self) -> u32 {
        self as u32
    }

    /// The stable lowercase label, used in JSON documents.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Superseded => "superseded",
            Self::Disabled => "disabled",
        }
    }

    /// Reconstruct a [`VersionStatus`] from its stable numeric code.
    #[must_use]
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::Draft),
            1 => Some(Self::Active),
            2 => Some(Self::Superseded),
            3 => Some(Self::Disabled),
            _ => None,
        }
    }
}

/// Immutable metadata of one policy version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyVersionInfo {
    /// The policy this version belongs to.
    pub policy_id: RuleId,
    /// Monotonic version number within the policy.
    pub version: u32,
    /// Lifecycle status.
    pub status: VersionStatus,
    /// Hash of the version's configuration (rule set).
    pub config_hash: ConfigHash,
}

impl PolicyVersionInfo {
    /// Create a new draft version of a policy.
    #[must_use]
    pub const fn new(policy_id: RuleId, version: u32, config_hash: ConfigHash) -> Self {
        Self {
            policy_id,
            version,
            status: VersionStatus::Draft,
            config_hash,
        }
    }

    /// Whether this version is currently in force.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self.status, VersionStatus::Active)
    }
}

/// Whether a version in this status may be activated.
///
/// Only drafts can be activated. Active, superseded and disabled versions
/// require a new version rather than a status flip, which is what keeps the
/// version history append-only and auditable.
#[must_use]
pub const fn can_activate(status: VersionStatus) -> bool {
    matches!(status, VersionStatus::Draft)
}

/// The status a draft moves to when activated.
///
/// Returns `None` when the version is not activatable; callers must reject
/// invalid activations rather than silently ignoring them.
#[must_use]
pub const fn activated(status: VersionStatus) -> Option<VersionStatus> {
    match status {
        VersionStatus::Draft => Some(VersionStatus::Active),
        _ => None,
    }
}

/// The status any version moves to when disabled by an administrator.
#[must_use]
pub const fn disabled(_status: VersionStatus) -> VersionStatus {
    VersionStatus::Disabled
}

#[cfg(test)]
mod tests {
    use super::{activated, can_activate, disabled, ConfigHash, PolicyVersionInfo, VersionStatus};
    use crate::rule::RuleId;

    const HASH: ConfigHash = ConfigHash::from_bytes([7u8; super::CONFIG_HASH_LEN]);

    #[test]
    fn statuses_round_trip() {
        for status in [
            VersionStatus::Draft,
            VersionStatus::Active,
            VersionStatus::Superseded,
            VersionStatus::Disabled,
        ] {
            assert_eq!(VersionStatus::from_code(status.to_code()), Some(status));
        }
        assert_eq!(VersionStatus::from_code(99), None);
    }

    #[test]
    fn new_versions_are_drafts() {
        let version = PolicyVersionInfo::new(RuleId::from_str("institutional-default"), 3, HASH);
        assert_eq!(version.status, VersionStatus::Draft);
        assert!(!version.is_active());
        assert_eq!(version.version, 3);
        assert_eq!(version.config_hash, HASH);
    }

    #[test]
    fn only_drafts_can_be_activated() {
        assert!(can_activate(VersionStatus::Draft));
        assert!(!can_activate(VersionStatus::Active));
        assert!(!can_activate(VersionStatus::Superseded));
        assert!(!can_activate(VersionStatus::Disabled));

        assert_eq!(activated(VersionStatus::Draft), Some(VersionStatus::Active));
        assert_eq!(activated(VersionStatus::Active), None);
        assert_eq!(activated(VersionStatus::Superseded), None);
        assert_eq!(activated(VersionStatus::Disabled), None);
    }

    #[test]
    fn disabling_is_available_from_any_status() {
        assert_eq!(disabled(VersionStatus::Draft), VersionStatus::Disabled);
        assert_eq!(disabled(VersionStatus::Active), VersionStatus::Disabled);
        assert_eq!(disabled(VersionStatus::Superseded), VersionStatus::Disabled);
    }
}
