use soroban_sdk::{Env, Symbol, Vec};
use chainlearn_shared::MIN_CREDENTIAL_SCORE;

use crate::types::{Course, DataKey, ProgressInfo, QuizResult};

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
        let avg_score = average_quiz_score(env, learner, course_id);
        (avg_score * 30) / 100
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
fn count_completed_modules(
    env: &Env,
    learner: &soroban_sdk::Address,
    course_id: &Symbol,
) -> u32 {
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
fn average_quiz_score(
    env: &Env,
    learner: &soroban_sdk::Address,
    course_id: &Symbol,
) -> u32 {
    let progress: ProgressInfo = env
        .storage()
        .persistent()
        .get(&DataKey::Progress(learner.clone(), course_id.clone()))
        .expect("not enrolled");

    if progress.quiz_scores.is_empty() {
        return 0;
    }

    let mut total_score: u64 = 0;
    let count = progress.quiz_scores.len() as u64;

    for quiz in progress.quiz_scores.iter() {
        total_score += quiz.score as u64;
    }

    (total_score / count) as u32
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
///
/// # Returns
/// `true` if the learner qualifies for a credential.
pub fn is_eligible_for_credential(
    env: &Env,
    learner: &soroban_sdk::Address,
    course_id: &Symbol,
    course: &Course,
) -> bool {
    // Check all modules completed
    let completed = count_completed_modules(env, learner, course_id);
    if completed < course.total_modules {
        return false;
    }

    // Check quiz scores
    let progress: Option<ProgressInfo> = env
        .storage()
        .persistent()
        .get(&DataKey::Progress(learner.clone(), course_id.clone()));

    match progress {
        Some(p) => {
            if p.quiz_scores.len() < course.total_quizzes {
                return false;
            }
            let avg = average_quiz_score(env, learner, course_id);
            avg >= MIN_CREDENTIAL_SCORE
        }
        None => false,
    }
}

/// Calculate the token reward for a quiz submission.
///
/// # Arguments
/// * `score` - The quiz score (0-100)
///
/// # Returns
/// The reward amount in token base units.
pub fn calculate_quiz_reward(score: u32) -> i128 {
    (score as i128) * (chainlearn_shared::BASE_REWARD_PER_POINT as i128)
}
