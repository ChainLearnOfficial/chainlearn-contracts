use chainlearn_shared::MIN_CREDENTIAL_SCORE;
use soroban_sdk::{Address, Env, Symbol};

use crate::metadata::{CredentialInfo, DataKey};
use crate::ProgressTrackerClient;

/// Mint a new credential NFT for a learner.
///
/// The credential is only minted if the learner's score meets the minimum
/// threshold, matches the score the progress-tracker recorded, and the
/// progress-tracker confirms the learner actually completed the course. Each
/// learner can only receive one credential per course.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `to` - Learner address receiving the credential
/// * `course_id` - Identifier of the completed course
/// * `score` - Final score (must be >= 50 and match the tracker's record)
/// * `metadata_uri` - URI to off-chain metadata
///
/// # Returns
/// The unique credential ID.
///
/// # Panics
/// * If score is below the minimum threshold
/// * If the learner already has a credential for this course
/// * If the progress-tracker reports the learner is not eligible
/// * If score does not match the progress-tracker's verified score
/// * If the credential ID counter would overflow `u64`
pub fn mint_credential(
    env: &Env,
    to: &Address,
    course_id: &Symbol,
    score: u32,
    metadata_uri: &Symbol,
) -> u64 {
    // Score gate: only mint if score >= 50
    if score < MIN_CREDENTIAL_SCORE {
        panic!(
            "score {} below minimum threshold {}",
            score, MIN_CREDENTIAL_SCORE
        );
    }

    // Check for duplicate: one credential per learner per course
    let dup_key = DataKey::CourseCredential(to.clone(), course_id.clone());
    if env.storage().persistent().has(&dup_key) {
        panic!("credential already exists for this learner and course");
    }

    // Completion gate: the progress-tracker is the source of truth for whether
    // the learner finished every module and quiz in the course.
    let progress_tracker: Address = env
        .storage()
        .persistent()
        .get(&DataKey::ProgressTracker)
        .expect("not initialized");
    let tracker = ProgressTrackerClient::new(env, &progress_tracker);
    if !tracker.is_eligible_for_credential(to, course_id) {
        panic!("learner has not completed the course requirements");
    }

    // Score gate: the caller supplies a score, but the progress-tracker is the
    // only authority on what the learner actually earned. Without this check a
    // caller could mint a credential reading 100 for a learner who scored 50
    // (#34). The tracker's value is the average across the learner's submitted
    // quizzes for this course.
    let verified_score = tracker.get_course_score(to, course_id);
    if score != verified_score {
        panic!(
            "score {} does not match verified score {}",
            score, verified_score
        );
    }

    // Generate unique credential ID. The counter is checked so it can never wrap
    // around to an already-issued ID and overwrite an existing credential (#99).
    let counter: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::CredentialCounter)
        .unwrap_or(0);
    let credential_id = match counter.checked_add(1) {
        Some(id) => id,
        None => panic!("credential ID counter overflow"),
    };
    env.storage()
        .persistent()
        .set(&DataKey::CredentialCounter, &credential_id);

    // Build credential info
    let info = CredentialInfo {
        learner: to.clone(),
        course_id: course_id.clone(),
        score,
        issued_at: env.ledger().timestamp(),
        revoked: false,
        metadata_uri: metadata_uri.clone(),
    };

    // Store credential data. The owner is available as `info.learner`, so no
    // separate owner key is kept (#116).
    env.storage()
        .persistent()
        .set(&DataKey::Credential(credential_id), &info);

    // Track credentials per learner
    let mut learner_creds: soroban_sdk::Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::LearnerCredentials(to.clone()))
        .unwrap_or(soroban_sdk::Vec::new(env));
    learner_creds.push_back(credential_id);
    env.storage()
        .persistent()
        .set(&DataKey::LearnerCredentials(to.clone()), &learner_creds);

    // Store the course-credential mapping to prevent duplicates
    env.storage().persistent().set(&dup_key, &credential_id);

    // Index credentials by course for reverse lookup (#105)
    let mut course_creds: soroban_sdk::Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::CourseCredentials(course_id.clone()))
        .unwrap_or(soroban_sdk::Vec::new(env));
    course_creds.push_back(credential_id);
    env.storage()
        .persistent()
        .set(&DataKey::CourseCredentials(course_id.clone()), &course_creds);

    // Emit mint event. `metadata_uri` is included so indexers can reconstruct the
    // full credential metadata from the event stream alone (#101).
    env.events().publish(
        (Symbol::new(env, "credential_minted"),),
        (to, course_id, credential_id, score, metadata_uri),
    );

    credential_id
}
