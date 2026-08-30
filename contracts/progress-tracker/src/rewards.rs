use chainlearn_shared::MIN_CREDENTIAL_SCORE;

use crate::types::{Course, ProgressInfo};

/// Count how many modules a learner has completed in a course.
///
/// Uses the `modules_completed_bitmap` on [`ProgressInfo`] so the call is O(1)
/// instead of iterating every module's storage key.
pub fn count_completed_modules(progress: &ProgressInfo) -> u32 {
    progress.modules_completed_bitmap.count_ones()
}

/// Calculate the overall progress percentage for a learner in a course.
///
/// Progress is weighted:
/// - 70% from module completion (proportion of modules completed)
/// - 30% from quiz performance (average quiz score / 100)
pub fn calculate_progress(course: &Course, progress: &ProgressInfo) -> u32 {
    let module_progress = if course.total_modules > 0 {
        let completed = count_completed_modules(progress);
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
///
/// Uses the running sum (`total_quiz_score`) and count (`quizzes_submitted`)
/// maintained in [`ProgressInfo`], so this is O(1) — no Vec iteration.
pub fn average_quiz_score(progress: &ProgressInfo) -> u32 {
    if progress.quizzes_submitted == 0 {
        return 0;
    }

    (progress.total_quiz_score / progress.quizzes_submitted as u64) as u32
}

/// Determine if a learner is eligible for a credential.
pub fn is_eligible_for_credential(course: &Course, progress: &ProgressInfo) -> bool {
    // Check all modules completed
    let completed = count_completed_modules(progress);
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
