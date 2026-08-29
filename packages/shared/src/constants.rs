use soroban_sdk::{contracttype, Env, String as SorobanString};

/// Minimum score required to mint a credential (out of 100).
pub const MIN_CREDENTIAL_SCORE: u32 = 50;

/// Maximum score for any quiz.
pub const MAX_QUIZ_SCORE: u32 = 100;

/// Base reward per quiz point (in token base units).
pub const BASE_REWARD_PER_POINT: i128 = 100;

/// Maximum modules per course.
pub const MAX_MODULES_PER_COURSE: u32 = 64;

/// Maximum number of credential IDs a single paginated read may return.
pub const MAX_CREDENTIALS_PAGE_SIZE: u32 = 50;

/// TTL threshold (in ledgers) below which a persistent entry's lifetime is
/// extended. Assumes a ~5s average ledger close time, so this is roughly 30
/// days out.
pub const PERSISTENT_TTL_THRESHOLD: u32 = 518_400;

/// TTL (in ledgers) a persistent entry is extended to once it drops below
/// [`PERSISTENT_TTL_THRESHOLD`]. Roughly 90 days, comfortably under Soroban's
/// network-wide max entry TTL.
pub const PERSISTENT_TTL_EXTEND_TO: u32 = 1_555_200;

/// Version stamped into every contract's metadata on `initialize()` (#107),
/// so external tools can identify which release of the contracts is deployed
/// without guessing from behavior. Bump this alongside `CHANGELOG.md`.
pub const CONTRACT_VERSION: &str = "1.0.0";

/// On-chain identity of a deployed contract, written once during
/// `initialize()` and read back via a `contract_metadata()` getter (#107).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractMetadata {
    /// The contract's crate name, e.g. `"credential-nft"`.
    pub name: SorobanString,
    /// The contract's semantic version, e.g. `"1.0.0"`.
    pub version: SorobanString,
}

impl ContractMetadata {
    /// Build the metadata for a contract named `name`, stamped with the
    /// current [`CONTRACT_VERSION`].
    pub fn new(env: &Env, name: &str) -> Self {
        Self {
            name: SorobanString::from_str(env, name),
            version: SorobanString::from_str(env, CONTRACT_VERSION),
        }
    }
}
