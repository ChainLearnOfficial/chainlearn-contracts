use chainlearn_shared::ContractMetadata;
use soroban_sdk::{contracttype, Address, Env, IntoVal, Symbol, Val, Vec};

/// Represents a course with its modules and total module count.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Course {
    /// Unique course identifier.
    pub course_id: Symbol,
    /// Total number of modules in the course.
    pub total_modules: u32,
    /// Number of quizzes in the course.
    pub total_quizzes: u32,
    /// Ordered list of module IDs for the course.
    pub module_ids: Vec<Symbol>,
    /// List of valid quiz IDs for the course.
    pub quiz_ids: Vec<Symbol>,
    /// Whether the course is archived and cannot accept new enrollments (#210).
    pub archived: bool,
    /// Hash of the course's off-chain content, used to verify integrity (#235).
    ///
    /// Verification is optional: an empty symbol means no hash is set and
    /// enrollment skips the check.
    pub content_hash: Symbol,
    /// Courses that must be completed before a learner can enroll (#231).
    ///
    /// Empty means the course has no prerequisites and enrolls freely.
    pub prerequisites: Vec<Symbol>,
    /// Version of the course content (#245).
    ///
    /// Incremented when off-chain content changes; learners track which
    /// version they completed.
    pub version: u32,
}

/// Represents a quiz submission.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuizResult {
    /// Quiz identifier.
    pub quiz_id: Symbol,
    /// Course this quiz belongs to.
    pub course_id: Symbol,
    /// Score achieved (0-100).
    pub score: u32,
    /// Timestamp of submission.
    pub submitted_at: u64,
}

/// A learner's progress in a specific course.
///
/// Individual quiz submissions are stored under [`ProgressTrackerDataKey::QuizResult`]; this
/// struct only keeps the aggregates needed to derive progress and eligibility,
/// so a quiz result is never written twice.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressInfo {
    /// When the learner enrolled.
    pub enrolled_at: u64,
    /// Bitmap tracking completed module indices.
    pub modules_completed_bitmap: u64,
    /// Number of quizzes submitted for this course.
    pub quizzes_submitted: u32,
    /// Sum of every submitted quiz score, used to derive the average.
    pub total_quiz_score: u64,
    /// Overall progress percentage (0-100).
    pub overall_progress: u32,
    /// Whether the learner qualifies for a credential.
    pub eligible_for_credential: bool,
    /// The course version when the learner became eligible for a credential (#245).
    ///
    /// `None` until eligibility is reached, then set to the course's version
    /// at that moment so later version bumps do not erase the learner's
    /// record.
    pub completed_version: Option<u32>,
}

/// Complete progress snapshot for a learner in a course, aggregated from
/// several storage keys into a single value (#196).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressExport {
    /// Whether the learner is enrolled in the course.
    pub enrolled: bool,
    /// When the learner enrolled.
    pub enrolled_at: u64,
    /// Bitmap tracking completed module indices.
    pub modules_completed_bitmap: u64,
    /// Total number of modules in the course.
    pub total_modules: u32,
    /// Every quiz the learner has submitted for this course, in course-defined
    /// quiz order. Quizzes not yet submitted are omitted.
    pub quiz_scores: Vec<QuizResult>,
    /// Number of quizzes submitted for this course.
    pub quizzes_submitted: u32,
    /// Sum of every submitted quiz score, used to derive the average.
    pub total_quiz_score: u64,
    /// Overall progress percentage (0-100).
    pub overall_progress: u32,
    /// Whether the learner qualifies for a credential.
    pub eligible_for_credential: bool,
    /// The course version when the learner became eligible (#245).
    pub completed_version: Option<u32>,
}

/// Aggregate statistics for a learner across every course they enrolled in (#232).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LearnerStats {
    /// Number of courses the learner has enrolled in.
    pub courses_enrolled: u32,
    /// Number of enrolled courses the learner has completed, i.e. courses
    /// where they qualify for a credential.
    pub courses_completed: u32,
    /// Total quizzes submitted across every enrolled course.
    pub total_quizzes_submitted: u32,
    /// Sum of every submitted quiz score across every enrolled course.
    pub total_quiz_score: u64,
    /// Average quiz score across every enrolled course, floored to a whole
    /// number. Zero when no quiz has been submitted.
    pub average_score: u32,
    /// Reward tokens the learner's submitted quiz scores are worth, at
    /// `BASE_REWARD_PER_POINT` per score point -- the same rate the token
    /// contract mints at in `claim_reward`.
    pub total_rewards_earned: i128,
}

/// Contract metadata (#107) plus the on-chain upgrade counter (#219),
/// returned together from `contract_metadata()` so callers get a single,
/// complete identity snapshot for the deployed contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionedContractMetadata {
    /// The contract's crate name and semantic version (#107).
    pub metadata: ContractMetadata,
    /// Number of times this contract has been upgraded in place (#219).
    /// Starts at `0` for a freshly initialized contract and is bumped by
    /// whatever upgrade mechanism the contract adopts; progress-tracker has
    /// none yet, so this only ever reads back the initialized value today.
    pub version: u32,
}

/// Storage keys for the progress tracker contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgressTrackerDataKey {
    Admin,
    Course(Symbol),
    Progress(Address, Symbol),
    ModuleCompleted(Address, Symbol, Symbol),
    QuizResult(Address, Symbol, Symbol),
    /// On-chain contract name/version, set on `initialize()` (#107).
    Metadata,
    /// Emergency pause state (#189).
    Paused,
    /// Every course a learner has enrolled in, in enrollment order (#232).
    LearnerCourses(Address),
    /// On-chain upgrade counter, set to `0` on `initialize()` and bumped by
    /// whatever upgrade mechanism the contract adopts (#219).
    Version,
    /// The address a learner has delegated progress-tracking to, if any
    /// (#222). Absent when the learner has no active delegation.
    DelegatedTo(Address),
    /// Running count of persistent storage entries this contract has
    /// written, excluding this counter entry itself (#239).
    StorageSize,
}

// ── Storage Size Tracking (#239) ─────────────────────────────────────────────
//
// Soroban has no API to enumerate or count a contract's storage entries at
// runtime, so the count is maintained as an ordinary persistent counter,
// kept in sync by routing every persistent write through `write_entry`
// below instead of calling `env.storage().persistent().set` directly. It
// checks whether the key already exists before writing, so overwriting an
// existing key does not double-count it.

/// Get the current persistent-entry count (#239).
///
/// O(1): reads a single counter entry, never scans storage.
pub fn get_storage_size(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&ProgressTrackerDataKey::StorageSize)
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
        .set(&ProgressTrackerDataKey::StorageSize, &next);
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
