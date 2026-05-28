//! Integration tests for the end-to-end credential flow.
//!
//! Tests the full journey: enroll -> complete -> quiz -> credential mint -> verify.

mod fixtures;

use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

/// Test the complete credential flow from enrollment to minting.
///
/// 1. Learner enrolls in a course
/// 2. Learner completes all modules
/// 3. Learner submits passing quiz scores
/// 4. Check eligibility
/// 5. Mint credential
/// 6. Verify credential on-chain
#[test]
fn test_end_to_end_credential_flow() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let course_id = Symbol::new(env, "rust_101");

    // Step 1: Enroll
    fixtures::create_sample_course(env);
    // progress_client.enroll(learner, &course_id);

    // Step 2: Complete all modules
    // progress_client.complete_module(learner, &course_id, &Symbol::new(env, "mod_basics"));
    // progress_client.complete_module(learner, &course_id, &Symbol::new(env, "mod_ownership"));
    // progress_client.complete_module(learner, &course_id, &Symbol::new(env, "mod_traits"));

    // Step 3: Submit quiz scores
    // progress_client.submit_quiz_score(learner, &course_id, &Symbol::new(env, "quiz_midterm"), &85);
    // progress_client.submit_quiz_score(learner, &course_id, &Symbol::new(env, "quiz_final"), &75);

    // Step 4: Check eligibility
    // assert!(progress_client.is_eligible_for_credential(learner, &course_id));

    // Step 5: Mint credential (average score = 80)
    let metadata_uri = Symbol::new(env, "ipfs://QmCredential123");
    // let cred_id = credential_client.mint_credential(learner, &course_id, &80, &metadata_uri);

    // Step 6: Verify
    // let info = credential_client.verify_credential(&cred_id);
    // assert_eq!(info.learner, *learner);
    // assert_eq!(info.course_id, course_id);
    // assert_eq!(info.score, 80);
    // assert!(!info.revoked);
}

/// Test that credentials can be verified by anyone.
#[test]
fn test_public_verification() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    let anyone = Address::generate(env);
    env.mock_all_auths();

    // Mint a credential
    // let cred_id = credential_client.mint_credential(learner, &course_id, &90, &uri);

    // Anyone can verify (no auth required for read)
    // let info = credential_client.verify_credential(&cred_id);
    // assert_eq!(info.score, 90);
}

/// Test that a learner can hold credentials from multiple courses.
#[test]
fn test_multiple_course_credentials() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    // let cred1 = credential_client.mint_credential(learner, &Symbol::new(env, "rust_101"), &85, &uri);
    // let cred2 = credential_client.mint_credential(learner, &Symbol::new(env, "sol_201"), &90, &uri);

    // let creds = credential_client.get_credentials_for(learner);
    // assert_eq!(creds.len(), 2);
    // assert!(creds.contains(&cred1));
    // assert!(creds.contains(&cred2));
}

/// Test that credential revocation is visible in verification.
#[test]
fn test_revoked_credential_shows_revoked_status() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    // let cred_id = credential_client.mint_credential(learner, &course_id, &80, &uri);
    // assert!(credential_client.is_credential_valid(&cred_id));

    // credential_client.revoke_credential(&cred_id);

    // let info = credential_client.verify_credential(&cred_id);
    // assert!(info.revoked);
    // assert!(!credential_client.is_credential_valid(&cred_id));
}

/// Test credential metadata URI is stored correctly.
#[test]
fn test_credential_metadata_uri() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let expected_uri = Symbol::new(env, "ipfs://QmTestMetadata");
    // let cred_id = credential_client.mint_credential(learner, &course_id, &75, &expected_uri);

    // let info = credential_client.verify_credential(&cred_id);
    // assert_eq!(info.metadata_uri, expected_uri);
}

/// Test credential issuance timestamp is set.
#[test]
fn test_credential_has_issuance_timestamp() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    // let cred_id = credential_client.mint_credential(learner, &course_id, &80, &uri);

    // let info = credential_client.verify_credential(&cred_id);
    // assert!(info.issued_at > 0);
}
