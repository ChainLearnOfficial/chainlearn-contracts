//! Direct cross-contract calls into the progress-tracker (#217).
//!
//! These wrappers call `env.invoke_contract` directly instead of constructing a
//! `ProgressTrackerClient` per hop (#133): the client wrapper only ever holds an
//! `Env` + `Address` clone and then does the same `invoke_contract` underneath,
//! so skipping it removes that per-call setup. Callers resolve the tracker
//! address once (a storage read) and pass it to each wrapper they need rather
//! than re-reading it for every check.

use soroban_sdk::{Address, Env, IntoVal, Symbol};

/// `ProgressTracker::course_exists(course_id) -> bool`
pub fn course_exists(env: &Env, tracker: &Address, course_id: &Symbol) -> bool {
    env.invoke_contract(
        tracker,
        &Symbol::new(env, "course_exists"),
        (course_id,).into_val(env),
    )
}

/// `ProgressTracker::is_eligible_for_credential(learner, course_id) -> bool`
pub fn is_eligible_for_credential(
    env: &Env,
    tracker: &Address,
    learner: &Address,
    course_id: &Symbol,
) -> bool {
    env.invoke_contract(
        tracker,
        &Symbol::new(env, "is_eligible_for_credential"),
        (learner, course_id).into_val(env),
    )
}

/// `ProgressTracker::get_course_score(learner, course_id) -> u32`
pub fn get_course_score(
    env: &Env,
    tracker: &Address,
    learner: &Address,
    course_id: &Symbol,
) -> u32 {
    env.invoke_contract(
        tracker,
        &Symbol::new(env, "get_course_score"),
        (learner, course_id).into_val(env),
    )
}
