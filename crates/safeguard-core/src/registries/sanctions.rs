//! Sanctions dataset entry status.
//!
//! Mirrors the `status` enum of `policy-schema/sanctions.schema.json`
//! (`active`/`inactive`). An entry is *active* while it screens subjects and
//! *inactive* once it has been reviewed and removed from screening without
//! being deleted (audit-friendly: the record's history stays readable).
//!
//! Matching semantics live in the policy engine (the sanctions rule checks a
//! caller-resolved match flag); this enum only classifies the dataset entry.

/// Screening status of a normalized sanctions entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SanctionsStatus {
    /// The entry actively screens subjects.
    Active = 0,
    /// The entry no longer screens subjects (retired, reviewed).
    Inactive = 1,
}

impl SanctionsStatus {
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
            Self::Inactive => "inactive",
        }
    }

    /// Reconstruct a [`SanctionsStatus`] from its stable numeric code.
    #[must_use]
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::Active),
            1 => Some(Self::Inactive),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SanctionsStatus;

    #[test]
    fn statuses_round_trip() {
        for status in [SanctionsStatus::Active, SanctionsStatus::Inactive] {
            assert_eq!(SanctionsStatus::from_code(status.to_code()), Some(status));
        }
        assert_eq!(SanctionsStatus::from_code(99), None);
    }

    #[test]
    fn labels_are_stable() {
        assert_eq!(SanctionsStatus::Active.as_str(), "active");
        assert_eq!(SanctionsStatus::Inactive.as_str(), "inactive");
    }
}
