//! Full flow integration tests for cross-contract interactions.

mod fixtures;
use fixtures::{complete_full_course, create_sample_course, setup_chainlearn_env};

use credential_nft::CredentialNftClient;
use learn_token::LearnTokenClient;
use progress_tracker::ProgressTrackerClient;
use soroban_sdk::{testutils::Address as _, Address, Symbol};

#[test]
fn test_full_learner_journey() {
    let env_context = setup_chainlearn_env();
    let env = env_context.env;
    let learner = env_context.learner;
    let _admin = env_context.admin;

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

// ── Concurrent Credential and Reward Claim Integration Tests (#640) ────

/// Scope 1 & 4: Test claim_reward then mint_credential in sequence,
/// verifying that token reward claiming does not disrupt subsequent credential minting
/// and that token total_supply and user balance remain strictly consistent.
#[test]
fn test_claim_reward_then_mint_credential_sequence() {
    let env_context = setup_chainlearn_env();
    let env = env_context.env;
    let learner = env_context.learner;

    let token_client = LearnTokenClient::new(&env, &env_context.token_contract_id);
    let credential_client = CredentialNftClient::new(&env, &env_context.credential_contract_id);
    let progress_client = ProgressTrackerClient::new(&env, &env_context.progress_contract_id);

    env.mock_all_auths();

    let course_id = create_sample_course(&env, &progress_client);
    complete_full_course(&env, &learner, &course_id, &progress_client);

    // Initial state: 0 balance and 0 supply
    assert_eq!(token_client.balance(&learner), 0);
    assert_eq!(token_client.total_supply(), 0);

    // Step 1: Claim rewards first
    token_client.claim_reward(&learner, &course_id, &Symbol::new(&env, "quiz_midterm"));
    token_client.claim_reward(&learner, &course_id, &Symbol::new(&env, "quiz_final"));

    let expected_rewards = 85 * 100 + 75 * 100; // 16,000
    assert_eq!(token_client.balance(&learner), expected_rewards);
    assert_eq!(token_client.total_supply(), expected_rewards);

    // Step 2: Mint credential after rewards are already claimed
    let metadata_uri = Symbol::new(&env, "ipfs_hash_seq_1");
    let cred_id = credential_client.mint_credential(&learner, &course_id, &80, &metadata_uri);

    // Verify credential was successfully issued
    let cred_info = credential_client.verify_credential(&cred_id);
    assert_eq!(cred_info.learner, learner);
    assert_eq!(cred_info.course_id, course_id);
    assert_eq!(cred_info.score, 80);
    assert!(!cred_info.revoked);

    // Step 3: Verify supply and balance consistency post-credential minting
    assert_eq!(token_client.balance(&learner), expected_rewards);
    assert_eq!(token_client.total_supply(), expected_rewards);
}

/// Scope 2 & 4: Test mint_credential then claim_reward in reverse sequence,
/// verifying that minting a credential does not invalidate course progress
/// or block subsequent reward claiming.
#[test]
fn test_mint_credential_then_claim_reward_reverse_sequence() {
    let env_context = setup_chainlearn_env();
    let env = env_context.env;
    let learner = env_context.learner;

    let token_client = LearnTokenClient::new(&env, &env_context.token_contract_id);
    let credential_client = CredentialNftClient::new(&env, &env_context.credential_contract_id);
    let progress_client = ProgressTrackerClient::new(&env, &env_context.progress_contract_id);

    env.mock_all_auths();

    let course_id = create_sample_course(&env, &progress_client);
    complete_full_course(&env, &learner, &course_id, &progress_client);

    // Initial state check
    assert_eq!(token_client.balance(&learner), 0);
    assert_eq!(token_client.total_supply(), 0);

    // Step 1: Mint credential first
    let metadata_uri = Symbol::new(&env, "ipfs_hash_rev_1");
    let cred_id = credential_client.mint_credential(&learner, &course_id, &80, &metadata_uri);

    let cred_info = credential_client.verify_credential(&cred_id);
    assert_eq!(cred_info.learner, learner);
    assert_eq!(cred_info.course_id, course_id);
    assert_eq!(cred_info.score, 80);

    // Token balance is still 0 before claiming
    assert_eq!(token_client.balance(&learner), 0);
    assert_eq!(token_client.total_supply(), 0);

    // Step 2: Claim rewards after credential has already been minted
    token_client.claim_reward(&learner, &course_id, &Symbol::new(&env, "quiz_midterm"));
    token_client.claim_reward(&learner, &course_id, &Symbol::new(&env, "quiz_final"));

    let expected_rewards = 85 * 100 + 75 * 100; // 16,000
    assert_eq!(token_client.balance(&learner), expected_rewards);
    assert_eq!(token_client.total_supply(), expected_rewards);

    // Step 3: Verify credential remains valid and untouched
    let cred_info_after = credential_client.verify_credential(&cred_id);
    assert_eq!(cred_info_after.score, 80);
    assert!(!cred_info_after.revoked);
}

/// Scope 3: Test interleaved / concurrent operations in the same session without state reset,
/// simulating multiple learners performing claims and mints concurrently.
#[test]
fn test_concurrent_multi_learner_claim_and_mint_same_session() {
    let env_context = setup_chainlearn_env();
    let env = env_context.env;
    let learner1 = env_context.learner;
    let learner2 = Address::generate(&env);

    let token_client = LearnTokenClient::new(&env, &env_context.token_contract_id);
    let credential_client = CredentialNftClient::new(&env, &env_context.credential_contract_id);
    let progress_client = ProgressTrackerClient::new(&env, &env_context.progress_contract_id);

    env.mock_all_auths();

    let course_id = create_sample_course(&env, &progress_client);
    complete_full_course(&env, &learner1, &course_id, &progress_client);
    complete_full_course(&env, &learner2, &course_id, &progress_client);

    // Interleaved execution in same session:
    // 1. Learner 1 claims midterm reward
    token_client.claim_reward(&learner1, &course_id, &Symbol::new(&env, "quiz_midterm"));
    assert_eq!(token_client.balance(&learner1), 8500);

    // 2. Learner 2 mints credential before claiming any rewards
    let uri2 = Symbol::new(&env, "ipfs_hash_learner2");
    let cred2_id = credential_client.mint_credential(&learner2, &course_id, &80, &uri2);

    // 3. Learner 1 mints credential
    let uri1 = Symbol::new(&env, "ipfs_hash_learner1");
    let cred1_id = credential_client.mint_credential(&learner1, &course_id, &80, &uri1);

    // 4. Learner 2 claims both midterm and final rewards
    token_client.claim_reward(&learner2, &course_id, &Symbol::new(&env, "quiz_midterm"));
    token_client.claim_reward(&learner2, &course_id, &Symbol::new(&env, "quiz_final"));

    // 5. Learner 1 claims final reward
    token_client.claim_reward(&learner1, &course_id, &Symbol::new(&env, "quiz_final"));

    // Verify final state consistency:
    assert_eq!(token_client.balance(&learner1), 16_000);
    assert_eq!(token_client.balance(&learner2), 16_000);
    assert_eq!(token_client.total_supply(), 32_000);

    // Both credentials must exist, be distinct, and belong to the respective learners
    assert_ne!(cred1_id, cred2_id);
    let info1 = credential_client.verify_credential(&cred1_id);
    let info2 = credential_client.verify_credential(&cred2_id);
    assert_eq!(info1.learner, learner1);
    assert_eq!(info2.learner, learner2);
    assert!(!info1.revoked);
    assert!(!info2.revoked);
}
