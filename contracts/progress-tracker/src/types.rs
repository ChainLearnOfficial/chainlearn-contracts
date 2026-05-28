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
}

/// Represents a single module within a course.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Module {
    /// Module identifier.
    pub module_id: Symbol,
    /// Parent course identifier.
    pub course_id: Symbol,
    /// Whether this module requires a quiz to complete.
    pub requires_quiz: bool,
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
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressInfo {
    /// When the learner enrolled.
    pub enrolled_at: u64,
    /// IDs of completed modules.
    pub modules_completed: Vec<Symbol>,
    /// Quiz results for this course.
    pub quiz_scores: Vec<QuizResult>,
    /// Overall progress percentage (0-100).
    pub overall_progress: u32,
    /// Whether the learner qualifies for a credential.
    pub eligible_for_credential: bool,
}

/// Storage keys for the progress tracker contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    Course(Symbol),
    CourseModules(Symbol),
    Progress(Address, Symbol),
    ModuleCompleted(Address, Symbol, Symbol),
    QuizResult(Address, Symbol, Symbol),
}
