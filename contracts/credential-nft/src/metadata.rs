use soroban_sdk::{contracttype, Address, Env, IntoVal, Symbol, Val};

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
    /// Display properties for a credential (#244).
    Display(u64),
    /// Generated certificate URI for learner and course (#223).
    CertificateURI(Address, Symbol),
    /// Emergency pause state (#189).
    Paused,
    /// Running count of persistent storage entries this contract has
    /// written, excluding this counter entry itself (#239).
    StorageSize,
}

// ── Storage Size Tracking (#239) ─────────────────────────────────────────────
//
// Soroban has no API to enumerate or count a contract's storage entries at
// runtime, so the count is maintained as an ordinary persistent counter,
// kept in sync by routing every persistent write and removal through
// `write_entry`/`remove_entry` below instead of calling
// `env.storage().persistent().set/remove` directly. Both check whether the
// key already exists before mutating, so overwriting an existing key does
// not double-count it, and removing a key that was never set does not
// underflow the counter.

/// Get the current persistent-entry count (#239).
///
/// O(1): reads a single counter entry, never scans storage.
pub fn get_storage_size(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&CredentialDataKey::StorageSize)
        .unwrap_or(0)
}

fn bump_storage_size(env: &Env, delta: i64) {
    let current = get_storage_size(env);
    let next = if delta >= 0 {
        current.saturating_add(delta as u64)
    } else {
        current.saturating_sub((-delta) as u64)
    };
    env.storage()
        .persistent()
        .set(&CredentialDataKey::StorageSize, &next);
}

/// Write `value` to persistent storage at `key`, incrementing
/// [`get_storage_size`] iff `key` did not already exist. Use this (instead
/// of `env.storage().persistent().set` directly) for every persistent write
/// so the counter stays accurate.
pub fn write_entry<K, V>(env: &Env, key: &K, value: &V)
where
    K: IntoVal<Env, Val>,
    V: IntoVal<Env, Val>,
{
    let is_new = !env.storage().persistent().has(key);
    env.storage().persistent().set(key, value);
    if is_new {
        bump_storage_size(env, 1);
    }
}

/// Remove `key` from persistent storage, decrementing [`get_storage_size`]
/// iff `key` existed. Use this (instead of
/// `env.storage().persistent().remove` directly) for every persistent
/// removal so the counter stays accurate.
#[allow(dead_code)]
pub fn remove_entry<K>(env: &Env, key: &K)
where
    K: IntoVal<Env, Val>,
{
    let existed = env.storage().persistent().has(key);
    env.storage().persistent().remove(key);
    if existed {
        bump_storage_size(env, -1);
    }
}

/// Display properties for a credential NFT (#244).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialDisplay {
    /// URL of the credential image (e.g., IPFS hash).
    pub image_url: Option<Symbol>,
    /// Human-readable description of the credential.
    pub description: Option<Symbol>,
    /// Name of the issuer organization.
    pub issuer_name: Option<Symbol>,
}

/// Combined verification response for a credential (#244).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialVerification {
    /// The core credential info.
    pub info: CredentialInfo,
    /// Optional display properties.
    pub display: Option<CredentialDisplay>,
}