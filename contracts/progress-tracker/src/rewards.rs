use chainlearn_shared::MIN_CREDENTIAL_SCORE;
use soroban_sdk::{Env, Symbol, Vec};

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
///
/// # Returns
/// Progress percentage (0-100).
pub fn calculate_progress(
    env: &Env,
    learner: &soroban_sdk::Address,
    course_id: &Symbol,
    course: &Course,
    progress: &ProgressInfo,
) -> u32 {
    let scale = 10000u64;

    // Module completion component (70% weight)
    let module_progress_scaled = if course.total_modules > 0 {
        let completed = count_completed_modules(env, learner, course_id);
        (completed as u64 * 70 * scale) / course.total_modules as u64
    } else {
        0
    };

    // Quiz performance component (30% weight)
    let quiz_progress_scaled = if course.total_quizzes > 0 {
        let avg_score = average_quiz_score(progress);
        (avg_score as u64 * 30 * scale) / 100
    } else {
        0
    };

    let total_scaled = module_progress_scaled + quiz_progress_scaled;
    let total = (total_scaled / scale) as u32;
    if total > 100 {
        100
    } else {
        total
    }
}

/// Count how many modules a learner has completed in a course.
fn count_completed_modules(env: &Env, learner: &soroban_sdk::Address, course_id: &Symbol) -> u32 {
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

/// Calculate the average quiz score for a learner in a course.
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
    learner: &soroban_sdk::Address,
    course_id: &Symbol,
    course: &Course,
    progress: &ProgressInfo,
) -> bool {
    // Check all modules completed
    let completed = count_completed_modules(env, learner, course_id);
    if completed < course.total_modules {
        return false;
    }

    // Check quiz scores
    if progress.quizzes_submitted < course.total_quizzes {
        return false;
    }
    let avg = average_quiz_score(progress);
    avg >= MIN_CREDENTIAL_SCORE
}
