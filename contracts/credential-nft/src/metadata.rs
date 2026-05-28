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
}

/// Counter key for generating unique credential IDs.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    CredentialCounter,
    Credential(u64),
    CredentialOwner(u64),
    LearnerCredentials(Address),
    CourseCredential(Address, Symbol),
}
