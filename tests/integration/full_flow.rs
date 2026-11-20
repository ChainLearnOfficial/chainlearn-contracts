//! Full flow integration tests for cross-contract interactions.

mod fixtures;
use fixtures::{setup_chainlearn_env, create_sample_course, complete_full_course};

use learn_token::LearnTokenClient;
use credential_nft::CredentialNftClient;
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
    let cred_id = credential_client.mint_credential(&_admin, &learner, &course_id, &80, &metadata_uri);

    // Verify credential
    let info = credential_client.verify_credential(&cred_id);
    assert_eq!(info.learner, learner);
    assert_eq!(info.course_id, course_id);
    assert_eq!(info.score, 80);
    assert!(!info.revoked);
}

#[test]
fn test_claim_reward_before_quiz_submission_fails() {
    let env_context = setup_chainlearn_env();
    let env = &env_context.env;
    let learner = &env_context.learner;

    let token_client = LearnTokenClient::new(env, &env_context.token_contract_id);
    let progress_client = ProgressTrackerClient::new(env, &env_context.progress_contract_id);

    env.mock_all_auths();

    let course_id = create_sample_course(env, &progress_client);
    progress_client.enroll(learner, &course_id);

    // No quiz score has been submitted yet, so the tracker cannot verify a
    // score and the reward claim must fail without minting anything.
    let result = token_client.try_claim_reward(
        learner,
        &course_id,
        &Symbol::new(env, "quiz_midterm"),
    );
    assert!(result.is_err());
    assert_eq!(token_client.balance(learner), 0);
    assert_eq!(token_client.total_supply(), 0);
}

#[test]
fn test_mint_credential_before_completing_all_modules_fails() {
    let env_context = setup_chainlearn_env();
    let env = &env_context.env;
    let learner = &env_context.learner;

    let credential_client = CredentialNftClient::new(env, &env_context.credential_contract_id);
    let progress_client = ProgressTrackerClient::new(env, &env_context.progress_contract_id);

    env.mock_all_auths();

    let course_id = create_sample_course(env, &progress_client);
    progress_client.enroll(learner, &course_id);

    // Finish both quizzes but leave every module incomplete.
    progress_client.submit_quiz_score(learner, &course_id, &Symbol::new(env, "quiz_midterm"), &85);
    progress_client.submit_quiz_score(learner, &course_id, &Symbol::new(env, "quiz_final"), &75);

    assert!(!progress_client.is_eligible_for_credential(learner, &course_id));

    let result = credential_client.try_mint_credential(
        learner,
        &course_id,
        &80,
        &Symbol::new(env, "ipfs_QmCredential123"),
    );
    assert!(result.is_err());
    assert_eq!(credential_client.get_credential_count(learner), 0);
}

#[test]
fn test_mint_credential_with_score_below_minimum_fails() {
    let env_context = setup_chainlearn_env();
    let env = &env_context.env;
    let learner = &env_context.learner;

    let credential_client = CredentialNftClient::new(env, &env_context.credential_contract_id);
    let progress_client = ProgressTrackerClient::new(env, &env_context.progress_contract_id);

    env.mock_all_auths();

    let course_id = create_sample_course(env, &progress_client);
    progress_client.enroll(learner, &course_id);
    progress_client.complete_module(learner, &course_id, &Symbol::new(env, "mod_basics"));
    progress_client.complete_module(learner, &course_id, &Symbol::new(env, "mod_ownership"));
    progress_client.complete_module(learner, &course_id, &Symbol::new(env, "mod_traits"));

    // Average of 40 is below the 50-point credential threshold, so the
    // learner never becomes eligible.
    progress_client.submit_quiz_score(learner, &course_id, &Symbol::new(env, "quiz_midterm"), &40);
    progress_client.submit_quiz_score(learner, &course_id, &Symbol::new(env, "quiz_final"), &40);
    assert!(!progress_client.is_eligible_for_credential(learner, &course_id));

    let result = credential_client.try_mint_credential(
        learner,
        &course_id,
        &40,
        &Symbol::new(env, "ipfs_QmCredential123"),
    );
    assert!(result.is_err());
    assert_eq!(credential_client.get_credential_count(learner), 0);
}

#[test]
fn test_operations_blocked_during_pause() {
    let env_context = setup_chainlearn_env();
    let env = &env_context.env;
    let admin = &env_context.admin;
    let learner = &env_context.learner;

    let token_client = LearnTokenClient::new(env, &env_context.token_contract_id);
    let credential_client = CredentialNftClient::new(env, &env_context.credential_contract_id);
    let progress_client = ProgressTrackerClient::new(env, &env_context.progress_contract_id);

    env.mock_all_auths();

    let course_id = create_sample_course(env, &progress_client);
    complete_full_course(env, learner, &course_id, &progress_client);

    // Claim one reward before pausing so transfer and burn have a balance to
    // operate on; leave the final quiz unclaimed for the paused claim check.
    token_client.claim_reward(learner, &course_id, &Symbol::new(env, "quiz_midterm"));
    assert_eq!(token_client.balance(learner), 8500);

    token_client.pause(admin);
    progress_client.emergency_pause();
    credential_client.emergency_pause();

    // Token operations are rejected while paused.
    let recipient = Address::generate(env);
    assert!(token_client.try_transfer(learner, &recipient, &1_000).is_err());
    assert!(token_client.try_burn(learner, &1_000).is_err());
    assert!(token_client
        .try_claim_reward(learner, &course_id, &Symbol::new(env, "quiz_final"))
        .is_err());

    // Progress-tracker writes are rejected while paused.
    assert!(progress_client
        .try_retake_quiz(learner, &course_id, &Symbol::new(env, "quiz_midterm"), &90)
        .is_err());

    // Credential minting is rejected while paused even though the learner is
    // otherwise fully eligible.
    assert!(credential_client
        .try_mint_credential(
            learner,
            &course_id,
            &80,
            &Symbol::new(env, "ipfs_QmCredential123"),
        )
        .is_err());

    // No paused call changed state.
    assert_eq!(token_client.balance(learner), 8500);
    assert_eq!(token_client.total_supply(), 8500);
    assert_eq!(token_client.balance(&recipient), 0);
    assert_eq!(credential_client.get_credential_count(learner), 0);
}

#[test]
fn test_operations_restored_after_unpause() {
    let env_context = setup_chainlearn_env();
    let env = &env_context.env;
    let admin = &env_context.admin;
    let learner = &env_context.learner;

    let token_client = LearnTokenClient::new(env, &env_context.token_contract_id);
    let credential_client = CredentialNftClient::new(env, &env_context.credential_contract_id);
    let progress_client = ProgressTrackerClient::new(env, &env_context.progress_contract_id);

    env.mock_all_auths();

    let course_id = create_sample_course(env, &progress_client);

    token_client.pause(admin);
    progress_client.emergency_pause();
    credential_client.emergency_pause();

    token_client.unpause(admin);
    progress_client.unpause();
    credential_client.unpause();

    // The full learner journey works again once every contract is unpaused.
    complete_full_course(env, learner, &course_id, &progress_client);

    token_client.claim_reward(learner, &course_id, &Symbol::new(env, "quiz_midterm"));
    token_client.claim_reward(learner, &course_id, &Symbol::new(env, "quiz_final"));
    assert_eq!(token_client.balance(learner), 16000);

    let cred_id = credential_client.mint_credential(
        learner,
        &course_id,
        &80,
        &Symbol::new(env, "ipfs_QmCredential123"),
    );
    let info = credential_client.verify_credential(&cred_id);
    assert_eq!(info.learner, *learner);
    assert_eq!(info.score, 80);
    assert_eq!(credential_client.get_credential_count(learner), 1);
}
