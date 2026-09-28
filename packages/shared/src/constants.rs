use soroban_sdk::{contracttype, Address, Env, String as SorobanString};

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

/// Strkey of the all-zero account (public key of 32 zero bytes). Contracts
/// reject it as a mint recipient or new admin, since nobody holds its key (#427).
pub const ZERO_ADDRESS_STR: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

/// Build the zero [`Address`] from [`ZERO_ADDRESS_STR`]. Defined once here so
/// the three contracts share a single, tested spelling of it (#427).
pub fn zero_address(env: &Env) -> Address {
    Address::from_string(&SorobanString::from_str(env, ZERO_ADDRESS_STR))
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::Address as _,
        xdr::{AccountId, PublicKey, ScAddress, Uint256},
        TryIntoVal,
    };

    #[test]
    fn zero_address_matches_all_zero_ed25519_key() {
        let env = Env::default();
        let expected: Address = ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(
            Uint256([0; 32]),
        )))
        .try_into_val(&env)
        .unwrap();
        assert_eq!(zero_address(&env), expected);
    }

    #[test]
    fn zero_address_differs_from_generated_addresses() {
        let env = Env::default();
        assert_ne!(zero_address(&env), Address::generate(&env));
    }
}
