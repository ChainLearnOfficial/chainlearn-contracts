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

/// Status of a learner's enrollment in a course.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnrollmentStatus {
    NotEnrolled,
    InProgress,
    Completed,
}
