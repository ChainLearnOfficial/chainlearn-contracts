//! Full flow integration tests for cross-contract interactions.

mod fixtures;
use fixtures::{setup_chainlearn_env, create_sample_course, complete_full_course};

use learn_token::LearnTokenClient;
use credential_nft::CredentialNftClient;
use progress_tracker::ProgressTrackerClient;
use soroban_sdk::Symbol;

#[test]
fn test_full_learner_journey() {
    let env_context = setup_chainlearn_env();
    let env = env_context.env;
    let learner = env_context.learner;
    let admin = env_context.admin;
    
    let token_client = LearnTokenClient::new(&env, &env_context.token_contract_id);
    let credential_client = CredentialNftClient::new(&env, &env_context.credential_contract_id);
    let progress_client = ProgressTrackerClient::new(&env, &env_context.progress_contract_id);

    env.mock_all_auths();

    // 1. Create a course and enroll the learner
    let course_id = create_sample_course(&env, &progress_client);
    
    // 2. Complete the full course (enrolls, completes modules, submits quizzes)
    complete_full_course(&env, &learner, &course_id, &progress_client);

    // Verify progress
    let progress = progress_client.get_progress(&learner, &course_id);
    assert!(progress.eligible_for_credential);
    // 3/3 modules = 70%, quizzes average 80 -> 30% of 80 = 24. 70+24 = 94.
    assert_eq!(progress.overall_progress, 94);

    // 3. Claim reward from learn-token
    // Midterm quiz reward
    token_client.claim_reward(&learner, &course_id, &Symbol::new(&env, "quiz_midterm"));
    // Final quiz reward
    token_client.claim_reward(&learner, &course_id, &Symbol::new(&env, "quiz_final"));

    // Check balance
    // 85 * 100 = 8500, 75 * 100 = 7500 => Total 16000
    assert_eq!(token_client.balance(&learner), 16000);

    // 4. Mint credential
    let metadata_uri = Symbol::new(&env, "ipfs_hash");
    let cred_id = credential_client.mint_credential(&learner, &course_id, &80, &metadata_uri);

    // Verify credential
    let info = credential_client.verify_credential(&cred_id);
    assert_eq!(info.learner, learner);
    assert_eq!(info.course_id, course_id);
    assert_eq!(info.score, 80);
    assert!(!info.revoked);
}
