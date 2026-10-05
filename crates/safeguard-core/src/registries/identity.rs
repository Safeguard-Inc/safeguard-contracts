//! Identity verification status.
//!
//! The normalized result of an identity/KYC/attestation provider (see
//! `docs/adapters.md`): the policy layer consumes these statuses, never the
//! provider-specific shapes behind them. The on-chain identity registry
//! stores one status per account reference.
//!
//! Fail-closed posture: anything other than `Verified` is not treated as
//! verified. The engine's allowlist/jurisdiction rules and the structural
//! account-status check carry the actual severity decisions; this enum only
//! says what the provider result means.

/// Verification status of an identity attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdentityStatus {
    /// The identity is verified and the attestation is current.
    Verified = 0,
    /// The identity has not been verified.
    Unverified = 1,
    /// A previously verified identity had its attestation revoked.
    Revoked = 2,
    /// A previously verified identity's attestation has expired.
    Expired = 3,
    /// The verification status could not be determined.
    Unknown = 4,
}

impl IdentityStatus {
    /// The stable numeric representation, used in on-chain serialization.
    #[must_use]
    pub const fn to_code(self) -> u32 {
        self as u32
    }

    /// The stable lowercase label, used in JSON documents.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Unverified => "unverified",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
            Self::Unknown => "unknown",
        }
    }

    /// Reconstruct an [`IdentityStatus`] from its stable numeric code.
    #[must_use]
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::Verified),
            1 => Some(Self::Unverified),
            2 => Some(Self::Revoked),
            3 => Some(Self::Expired),
            4 => Some(Self::Unknown),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::IdentityStatus;

    #[test]
    fn statuses_round_trip() {
        for status in [
            IdentityStatus::Verified,
            IdentityStatus::Unverified,
            IdentityStatus::Revoked,
            IdentityStatus::Expired,
            IdentityStatus::Unknown,
        ] {
            assert_eq!(IdentityStatus::from_code(status.to_code()), Some(status));
        }
        assert_eq!(IdentityStatus::from_code(99), None);
    }

    #[test]
    fn labels_are_stable() {
        assert_eq!(IdentityStatus::Verified.as_str(), "verified");
        assert_eq!(IdentityStatus::Unverified.as_str(), "unverified");
        assert_eq!(IdentityStatus::Revoked.as_str(), "revoked");
        assert_eq!(IdentityStatus::Expired.as_str(), "expired");
        assert_eq!(IdentityStatus::Unknown.as_str(), "unknown");
    }
}
