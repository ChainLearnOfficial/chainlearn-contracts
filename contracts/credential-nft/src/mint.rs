use chainlearn_shared::MIN_CREDENTIAL_SCORE;
use soroban_sdk::{Address, Env, Symbol};

use crate::metadata::{CredentialInfo, DataKey};

/// Mint a new credential NFT for a learner.
///
/// The credential is only minted if the learner's score meets the minimum
/// threshold. Each learner can only receive one credential per course.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `to` - Learner address receiving the credential
/// * `course_id` - Identifier of the completed course
/// * `score` - Final score (must be >= 50)
/// * `metadata_uri` - URI to off-chain metadata
///
/// # Returns
/// The unique credential ID.
///
/// # Panics
/// * If score is below the minimum threshold
/// * If the learner already has a credential for this course
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

    // Generate unique credential ID
    let counter: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::CredentialCounter)
        .unwrap_or(0);
    let credential_id = counter + 1;
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

    // Store credential data
    env.storage()
        .persistent()
        .set(&DataKey::Credential(credential_id), &info);
    env.storage()
        .persistent()
        .set(&DataKey::CredentialOwner(credential_id), to);

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

    // Emit mint event
    env.events().publish(
        (Symbol::new(env, "credential_minted"),),
        (to, course_id, credential_id, score),
    );

    credential_id
}
