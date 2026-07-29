use soroban_sdk::{contracttype, Address, Symbol, Vec};

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
}
