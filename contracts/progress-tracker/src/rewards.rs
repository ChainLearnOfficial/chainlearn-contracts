use chainlearn_shared::MIN_CREDENTIAL_SCORE;
use soroban_sdk::{Address, Env, Symbol, Vec};

use crate::types::{Course, ProgressInfo, ProgressTrackerDataKey};

/// Count how many modules a learner has completed in a course.
pub fn count_completed_modules(
    env: &Env,
    learner: &Address,
    course_id: &Symbol,
    modules: &Vec<Symbol>,
) -> u32 {
    let mut count = 0u32;
    for module_id in modules.iter() {
        let key = ProgressTrackerDataKey::ModuleCompleted(
            learner.clone(),
            course_id.clone(),
            module_id.clone(),
        );
        if env.storage().persistent().has(&key) {
            count += 1;
        }
    }
    count
}

/// Calculate the overall progress percentage for a learner in a course.
///
/// Progress is weighted:
/// - 70% from module completion (proportion of modules completed)
/// - 30% from quiz performance (average quiz score / 100)
pub fn calculate_progress(
    env: &Env,
    learner: &Address,
    course_id: &Symbol,
    course: &Course,
    progress: &ProgressInfo,
) -> u32 {
    let module_progress = if course.total_modules > 0 {
        let completed = count_completed_modules(env, learner, course_id, &course.module_ids);
        (completed * 70) / course.total_modules
    } else {
        0
    };

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

/// Calculate the average quiz score for a learner in a course from `ProgressInfo`.
pub fn average_quiz_score(progress: &ProgressInfo) -> u32 {
    if progress.quizzes_submitted == 0 {
        return 0;
    }

    (progress.total_quiz_score / progress.quizzes_submitted as u64) as u32
}

/// Determine if a learner is eligible for a credential.
pub fn is_eligible_for_credential(
    env: &Env,
    learner: &Address,
    course_id: &Symbol,
    course: &Course,
    progress: &ProgressInfo,
) -> bool {
    // Check all modules completed
    let completed = count_completed_modules(env, learner, course_id, &course.module_ids);
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
