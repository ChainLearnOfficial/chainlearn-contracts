use chainlearn_shared::MIN_CREDENTIAL_SCORE;
use soroban_sdk::{Address, Env, Symbol, Vec};

use crate::types::{Course, DataKey, ProgressInfo};

/// Calculate the overall progress percentage for a learner in a course.
///
/// Progress is weighted:
/// - 70% from module completion (proportion of modules completed)
/// - 30% from quiz performance (average quiz score / 100)
///
/// # Arguments
/// * `env` - Soroban environment
/// * `learner` - The learner address
/// * `course_id` - The course identifier
/// * `course` - The course configuration
/// * `progress` - The learner's current progress record
///
/// # Returns
/// Progress percentage (0-100).
pub fn calculate_progress(
    env: &Env,
    learner: &Address,
    course_id: &Symbol,
    course: &Course,
    progress: &ProgressInfo,
) -> u32 {
    // Module completion component (70% weight)
    let module_progress = if course.total_modules > 0 {
        let completed = count_completed_modules(env, learner, course_id);
        (completed * 70) / course.total_modules
    } else {
        0
    };

    // Quiz performance component (30% weight)
    let quiz_progress = if course.total_quizzes > 0 {
        (average_quiz_score(progress) * 30) / 100
    } else {
        0
    };

    let total = module_progress + quiz_progress;
    if total > 100 {
        100
    } else {
        total
    }
}

/// Count how many modules a learner has completed in a course.
pub fn count_completed_modules(env: &Env, learner: &Address, course_id: &Symbol) -> u32 {
    let modules: Vec<Symbol> = env
        .storage()
        .persistent()
        .get(&DataKey::CourseModules(course_id.clone()))
        .unwrap_or(Vec::new(env));

    let mut count = 0u32;
    for module_id in modules.iter() {
        let key = DataKey::ModuleCompleted(learner.clone(), course_id.clone(), module_id.clone());
        if env.storage().persistent().has(&key) {
            count += 1;
        }
    }
    count
}

/// Calculate the average quiz score from the aggregates on `ProgressInfo`.
///
/// Derived from the running total kept at submission time, so no quiz result
/// has to be re-read from storage.
pub fn average_quiz_score(progress: &ProgressInfo) -> u32 {
    if progress.quizzes_submitted == 0 {
        return 0;
    }

    (progress.total_quiz_score / progress.quizzes_submitted as u64) as u32
}

/// Determine if a learner is eligible for a credential.
///
/// Eligibility requires:
/// - All modules completed
/// - Average quiz score >= MIN_CREDENTIAL_SCORE
/// - All quizzes submitted
///
/// # Arguments
/// * `env` - Soroban environment
/// * `learner` - The learner address
/// * `course_id` - The course identifier
/// * `course` - The course configuration
/// * `progress` - The learner's current progress record
///
/// # Returns
/// `true` if the learner qualifies for a credential.
pub fn is_eligible_for_credential(
    env: &Env,
    learner: &Address,
    course_id: &Symbol,
    course: &Course,
    progress: &ProgressInfo,
) -> bool {
    // Check all modules completed
    let completed = count_completed_modules(env, learner, course_id);
    if completed < course.total_modules {
        return false;
    }

    // Check all quizzes submitted
    if progress.quizzes_submitted < course.total_quizzes {
        return false;
    }

    // Check average score meets minimum
    average_quiz_score(progress) >= MIN_CREDENTIAL_SCORE
}
