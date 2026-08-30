use soroban_sdk::{contracttype, Address, Symbol};

/// On-chain metadata for a minted credential NFT.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialInfo {
    /// The learner who earned this credential.
    pub learner: Address,
    /// The course identifier.
    pub course_id: Symbol,
    /// The learner's final score (0-100).
    pub score: u32,
    /// Ledger timestamp when the credential was issued.
    pub issued_at: u64,
    /// Whether the credential has been revoked.
    pub revoked: bool,
    /// URI pointing to off-chain metadata (e.g., IPFS).
    pub metadata_uri: Symbol,
    /// Optional expiration ledger height (0 = no expiration) (#193).
    pub expires_at: u32,
}

/// Counter key for generating unique credential IDs.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CredentialDataKey {
    Admin,
    ProgressTracker,
    CredentialCounter,
    Credential(u64),
    LearnerCredentials(Address),
    CourseCredential(Address, Symbol),
    CourseCredentials(Symbol),
    /// Mirrors `CredentialInfo.revoked` in its own single-bool entry so
    /// `is_credential_valid` can check it without deserializing the full
    /// `CredentialInfo` struct (#109).
    Revoked(u64),
    /// On-chain contract name/version, set on `initialize()` (#107).
    Metadata,
    /// Stores the reason for credential revocation (#194).
    RevocationReason(u64),
    /// Generated certificate URI for learner and course (#223).
    CertificateURI(Address, Symbol),
}
