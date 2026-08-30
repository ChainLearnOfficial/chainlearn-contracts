use chainlearn_shared::MIN_CREDENTIAL_SCORE;
use soroban_env_common::SymbolStr;
use soroban_sdk::{Address, Env, Symbol, TryFromVal};

use crate::metadata::{CredentialDataKey, CredentialInfo};
use crate::xcall;

/// Validate that `metadata_uri` is non-empty, meets the minimum length (>= 8 characters),
/// and starts with a recognized URI scheme (http, https, ipfs, or cert).
pub fn validate_metadata_uri(env: &Env, metadata_uri: &Symbol) {
    let sstr = match SymbolStr::try_from_val(env, &metadata_uri.to_symbol_val()) {
        Ok(s) => s,
        Err(_) => panic!("metadata_uri is malformed"),
    };
    let uri: &str = sstr.as_ref();
    if uri.is_empty() {
        panic!("metadata_uri cannot be empty");
    }
    if uri.len() < 8 {
        panic!("metadata_uri too short: minimum length is 8");
    }
    let has_valid_scheme = uri.starts_with("ipfs_")
        || uri.starts_with("ipfs://")
        || uri.starts_with("http_")
        || uri.starts_with("http://")
        || uri.starts_with("https_")
        || uri.starts_with("https://")
        || uri.starts_with("cert_")
        || uri.starts_with("cert://");
    if !has_valid_scheme {
        panic!("metadata_uri is malformed: must start with a valid URI scheme");
    }
}

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
/// * If `metadata_uri` is empty, too short (< 8 chars), or malformed
/// * If `course_id` does not correspond to a known course
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
    // Metadata URI gate: must be non-empty, >= 8 chars, with a valid scheme
    validate_metadata_uri(env, metadata_uri);

    // Score gate: only mint if score >= 50
    if score < MIN_CREDENTIAL_SCORE {
        panic!(
            "score {} below minimum threshold {}",
            score, MIN_CREDENTIAL_SCORE
        );
    }

    // Check for duplicate: one credential per learner per course
    let dup_key = CredentialDataKey::CourseCredential(to.clone(), course_id.clone());
    if env.storage().persistent().has(&dup_key) {
        panic!("credential already exists for this learner and course");
    }

    // Completion gate: the progress-tracker is the source of truth for whether
    // the learner finished every module and quiz in the course.
    let progress_tracker: Address = env
        .storage()
        .persistent()
        .get(&CredentialDataKey::ProgressTracker)
        .expect("not initialized");

    // Course gate: reject unknown course_ids explicitly instead of relying on
    // it transitively failing eligibility, so a bad course_id fails with a
    // clear reason (#108). The tracker address is resolved once above and
    // reused for each check via direct cross-contract calls (#217, #133).
    if !xcall::course_exists(env, &progress_tracker, course_id) {
        panic!("course does not exist");
    }

    if !xcall::is_eligible_for_credential(env, &progress_tracker, to, course_id) {
        panic!("learner has not completed the course requirements");
    }

    // Score gate: the caller supplies a score, but the progress-tracker is the
    // only authority on what the learner actually earned. Without this check a
    // caller could mint a credential reading 100 for a learner who scored 50
    // (#34). The tracker's value is the average across the learner's submitted
    // quizzes for this course.
    let verified_score = xcall::get_course_score(env, &progress_tracker, to, course_id);
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
        .get(&CredentialDataKey::CredentialCounter)
        .unwrap_or(0);
    let credential_id = match counter.checked_add(1) {
        Some(id) => id,
        None => panic!("credential ID counter overflow"),
    };
    crate::metadata::write_entry(env, &CredentialDataKey::CredentialCounter, &credential_id);

    // Build credential info
    let info = CredentialInfo {
        learner: to.clone(),
        course_id: course_id.clone(),
        score,
        issued_at: env.ledger().timestamp(),
        revoked: false,
        metadata_uri: metadata_uri.clone(),
        expires_at: 0, // No expiration by default (#193)
    };

    // Store credential data. The owner is available as `info.learner`, so no
    // separate owner key is kept (#116).
    crate::metadata::write_entry(env, &CredentialDataKey::Credential(credential_id), &info);

    // Track credentials per learner
    let mut learner_creds: soroban_sdk::Vec<u64> = env
        .storage()
        .persistent()
        .get(&CredentialDataKey::LearnerCredentials(to.clone()))
        .unwrap_or(soroban_sdk::Vec::new(env));
    learner_creds.push_back(credential_id);
    crate::metadata::write_entry(
        env,
        &CredentialDataKey::LearnerCredentials(to.clone()),
        &learner_creds,
    );

    // Store the course-credential mapping to prevent duplicates
    crate::metadata::write_entry(env, &dup_key, &credential_id);

    // Index credentials by course for reverse lookup (#105)
    let mut course_creds: soroban_sdk::Vec<u64> = env
        .storage()
        .persistent()
        .get(&CredentialDataKey::CourseCredentials(course_id.clone()))
        .unwrap_or(soroban_sdk::Vec::new(env));
    course_creds.push_back(credential_id);
    crate::metadata::write_entry(
        env,
        &CredentialDataKey::CourseCredentials(course_id.clone()),
        &course_creds,
    );

    // Emit mint event. `metadata_uri` is included so indexers can reconstruct the
    // full credential metadata from the event stream alone (#101).
    env.events().publish(
        (Symbol::new(env, "credential_minted"),),
        (to, course_id, credential_id, score, metadata_uri),
    );

    credential_id
}
