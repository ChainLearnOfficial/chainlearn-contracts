use soroban_sdk::contracttype;

/// Minimum score required to mint a credential (out of 100).
pub const MIN_CREDENTIAL_SCORE: u32 = 50;

/// Maximum score for any quiz.
pub const MAX_QUIZ_SCORE: u32 = 100;

/// Token decimals for the learn token.
pub const TOKEN_DECIMALS: u32 = 7;

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

/// Status of a learner's enrollment in a course.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnrollmentStatus {
    NotEnrolled,
    InProgress,
    Completed,
}
