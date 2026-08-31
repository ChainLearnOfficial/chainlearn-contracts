#![cfg(test)]

//! Security tests verifying that double-spending is prevented across all
//! ChainLearn contracts. Double-spending is a critical vulnerability — these
//! tests prove it is impossible through every contract surface.

use credential_nft::{CredentialNft, CredentialNftClient};
use learn_token::LearnTokenClient;
use progress_tracker::{ProgressTracker, ProgressTrackerClient};
use soroban_sdk::{
    testutils::Address as _, Address, Env, String as SorobanString, Symbol, Vec,
};

fn setup_env(env: &Env) -> (Address, LearnTokenClient<'static>, CredentialNftClient<'static>, ProgressTrackerClient<'static>) {
    let admin = Address::generate(env);

    // Register and initialize ProgressTracker
    let progress_contract_id = env.register_contract(None, ProgressTracker);
    let progress_client = ProgressTrackerClient::new(env, &progress_contract_id);
    progress_client.initialize(&admin);

    // Register and initialize LearnToken
    let token_contract_id = env.register_contract(None, learn_token::LearnToken);
    let token_client = LearnTokenClient::new(env, &token_contract_id);
    token_client.initialize(
        &admin,
        &SorobanString::from_str(env, "CLearn"),
        &SorobanString::from_str(env, "CLRN"),
        &7,
        &progress_contract_id,
        &1_000_000_000_000_000,
    );

    // Register and initialize CredentialNft
    let credential_contract_id = env.register_contract(None, CredentialNft);
    let credential_client = CredentialNftClient::new(env, &credential_contract_id);
    credential_client.initialize(&admin, &progress_contract_id);

    (admin, token_client, credential_client, progress_client)
}

fn create_course_and_complete(
    env: &Env,
    progress_client: &ProgressTrackerClient,
    learner: &Address,
    course_id: &Symbol,
    score: u32,
) {
    let mut module_ids = Vec::new(env);
    module_ids.push_back(Symbol::new(env, "mod_1"));
    module_ids.push_back(Symbol::new(env, "mod_2"));
    let mut quiz_ids = Vec::new(env);
    quiz_ids.push_back(Symbol::new(env, "quiz_1"));
    progress_client.create_course(course_id, &2, &1, &module_ids, &quiz_ids);
    progress_client.enroll(learner, course_id);
    progress_client.complete_module(learner, course_id, &Symbol::new(env, "mod_1"));
    progress_client.complete_module(learner, course_id, &Symbol::new(env, "mod_2"));
    progress_client.submit_quiz_score(learner, course_id, &Symbol::new(env, "quiz_1"), &score);
}

// ── Double-claim reward ─────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "reward already claimed")]
fn test_double_claim_reward_is_prevented() {
    let env = Env::default();
    let (_admin, token_client, _credential_client, progress_client) = setup_env(&env);
    let learner = Address::generate(&env);
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "course_1");
    create_course_and_complete(&env, &progress_client, &learner, &course_id, 80);

    // First claim succeeds
    token_client.claim_reward(&learner, &course_id, &Symbol::new(env, "quiz_1"));

    // Second claim for the same quiz must be rejected
    token_client.claim_reward(&learner, &course_id, &Symbol::new(env, "quiz_1"));
}

#[test]
fn test_double_claim_reward_does_not_corrupt_state() {
    let env = Env::default();
    let (_admin, token_client, _credential_client, progress_client) = setup_env(&env);
    let learner = Address::generate(&env);
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "course_1");
    create_course_and_complete(&env, &progress_client, &learner, &course_id, 80);

    // First claim succeeds
    token_client.claim_reward(&learner, &course_id, &Symbol::new(env, "quiz_1"));
    let balance_after_first = token_client.balance(&learner);
    let supply_after_first = token_client.total_supply();

    // Second claim reverts — state must be unchanged
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        token_client.claim_reward(&learner, &course_id, &Symbol::new(env, "quiz_1"));
    }));
    assert!(result.is_err(), "double claim should revert");

    assert_eq!(token_client.balance(&learner), balance_after_first);
    assert_eq!(token_client.total_supply(), supply_after_first);
}

#[test]
fn test_double_claim_different_quizzes_succeeds() {
    let env = Env::default();
    let (_admin, token_client, _credential_client, progress_client) = setup_env(&env);
    let learner = Address::generate(&env);
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "course_1");
    let mut module_ids = Vec::new(&env);
    module_ids.push_back(Symbol::new(&env, "mod_1"));
    let mut quiz_ids = Vec::new(&env);
    quiz_ids.push_back(Symbol::new(&env, "quiz_1"));
    quiz_ids.push_back(Symbol::new(&env, "quiz_2"));
    progress_client.create_course(&course_id, &1, &2, &module_ids, &quiz_ids);
    progress_client.enroll(&learner, &course_id);
    progress_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
    progress_client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);
    progress_client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &90);

    // Claiming rewards for two different quizzes is valid — not double-spending
    token_client.claim_reward(&learner, &course_id, &Symbol::new(&env, "quiz_1"));
    token_client.claim_reward(&learner, &course_id, &Symbol::new(&env, "quiz_2"));

    // 80*100 + 90*100 = 8000 + 9000 = 17000
    assert_eq!(token_client.balance(&learner), 17000);
}

#[test]
#[should_panic(expected = "reward already claimed")]
fn test_double_claim_in_batch_is_skipped() {
    let env = Env::default();
    let (_admin, token_client, _credential_client, progress_client) = setup_env(&env);
    let learner = Address::generate(&env);
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "course_1");
    create_course_and_complete(&env, &progress_client, &learner, &course_id, 80);

    // Claim once via single call
    token_client.claim_reward(&learner, &course_id, &Symbol::new(&env, "quiz_1"));

    // Attempting to claim again via batch — batch skips already-claimed quizzes,
    // so if it accidentally re-mints, the supply cap will be the only guard.
    // In this case the batch should simply skip the quiz. Verify with a second
    // quiz that hasn't been claimed yet.
    let mut quiz_ids = Vec::new(&env);
    quiz_ids.push_back(Symbol::new(&env, "quiz_1"));
    let successful = token_client.batch_claim_reward(&learner, &course_id, &quiz_ids);
    // The already-claimed quiz is skipped, not re-claimed
    assert_eq!(successful.len(), 0);
    assert_eq!(token_client.balance(&learner), 8000);
}

// ── Double-mint credential ──────────────────────────────────────────────────

#[test]
#[should_panic(expected = "credential already exists for this learner and course")]
fn test_double_mint_credential_is_prevented() {
    let env = Env::default();
    let (_admin, _token_client, credential_client, progress_client) = setup_env(&env);
    let learner = Address::generate(&env);
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "course_1");
    let metadata_uri = Symbol::new(&env, "ipfs_Qm123");
    create_course_and_complete(&env, &progress_client, &learner, &course_id, 85);

    // First mint succeeds
    credential_client.mint_credential(&learner, &course_id, &85, &metadata_uri);

    // Second mint for the same learner+course must be rejected
    credential_client.mint_credential(&learner, &course_id, &85, &metadata_uri);
}

#[test]
fn test_double_mint_credential_does_not_corrupt_state() {
    let env = Env::default();
    let (_admin, _token_client, credential_client, progress_client) = setup_env(&env);
    let learner = Address::generate(&env);
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "course_1");
    let metadata_uri = Symbol::new(&env, "ipfs_Qm123");
    create_course_and_complete(&env, &progress_client, &learner, &course_id, 85);

    let cred_id = credential_client.mint_credential(&learner, &course_id, &85, &metadata_uri);
    let total_before = credential_client.get_total_credentials_count();

    // Second mint reverts — state must be unchanged
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        credential_client.mint_credential(&learner, &course_id, &85, &metadata_uri);
    }));
    assert!(result.is_err(), "double mint should revert");

    assert_eq!(credential_client.get_total_credentials_count(), total_before);
    let info = credential_client.verify_credential(&cred_id);
    assert_eq!(info.learner, learner);
    assert_eq!(info.score, 85);
}

#[test]
fn test_mint_different_courses_succeeds() {
    let env = Env::default();
    let (_admin, _token_client, credential_client, progress_client) = setup_env(&env);
    let learner = Address::generate(&env);
    env.mock_all_auths();

    let course_a = Symbol::new(&env, "course_a");
    let course_b = Symbol::new(&env, "course_b");
    let metadata_uri = Symbol::new(&env, "ipfs_Qm123");

    create_course_and_complete(&env, &progress_client, &learner, &course_a, 85);

    // Create and complete course_b
    let mut module_ids = Vec::new(&env);
    module_ids.push_back(Symbol::new(&env, "mod_1"));
    let mut quiz_ids = Vec::new(&env);
    quiz_ids.push_back(Symbol::new(&env, "quiz_1"));
    progress_client.create_course(&course_b, &1, &1, &module_ids, &quiz_ids);
    progress_client.enroll(&learner, &course_b);
    progress_client.complete_module(&learner, &course_b, &Symbol::new(&env, "mod_1"));
    progress_client.submit_quiz_score(&learner, &course_b, &Symbol::new(&env, "quiz_1"), &90);

    // Two different courses — not double-spending
    credential_client.mint_credential(&learner, &course_a, &85, &metadata_uri);
    credential_client.mint_credential(&learner, &course_b, &90, &metadata_uri);

    assert_eq!(credential_client.get_credential_count(&learner), 2);
}

// ── Double-enroll ───────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "already enrolled")]
fn test_double_enroll_is_prevented() {
    let env = Env::default();
    let (_admin, _token_client, _credential_client, progress_client) = setup_env(&env);
    let learner = Address::generate(&env);
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "course_1");
    let mut module_ids = Vec::new(&env);
    module_ids.push_back(Symbol::new(&env, "mod_1"));
    let mut quiz_ids = Vec::new(&env);
    quiz_ids.push_back(Symbol::new(&env, "quiz_1"));
    progress_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);

    // First enrollment succeeds
    progress_client.enroll(&learner, &course_id);

    // Second enrollment for the same course must be rejected
    progress_client.enroll(&learner, &course_id);
}

#[test]
fn test_double_enroll_does_not_corrupt_state() {
    let env = Env::default();
    let (_admin, _token_client, _credential_client, progress_client) = setup_env(&env);
    let learner = Address::generate(&env);
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "course_1");
    let mut module_ids = Vec::new(&env);
    module_ids.push_back(Symbol::new(&env, "mod_1"));
    let mut quiz_ids = Vec::new(&env);
    quiz_ids.push_back(Symbol::new(&env, "quiz_1"));
    progress_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);

    progress_client.enroll(&learner, &course_id);
    let progress_before = progress_client.get_progress(&learner, &course_id);

    // Second enrollment reverts — state must be unchanged
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        progress_client.enroll(&learner, &course_id);
    }));
    assert!(result.is_err(), "double enroll should revert");

    let progress_after = progress_client.get_progress(&learner, &course_id);
    assert_eq!(
        progress_before.overall_progress, progress_after.overall_progress
    );
    assert_eq!(
        progress_before.modules_completed_bitmap,
        progress_after.modules_completed_bitmap
    );
}

#[test]
fn test_enroll_different_courses_succeeds() {
    let env = Env::default();
    let (_admin, _token_client, _credential_client, progress_client) = setup_env(&env);
    let learner = Address::generate(&env);
    env.mock_all_auths();

    let course_a = Symbol::new(&env, "course_a");
    let course_b = Symbol::new(&env, "course_b");

    let mut module_ids = Vec::new(&env);
    module_ids.push_back(Symbol::new(&env, "mod_1"));
    let mut quiz_ids = Vec::new(&env);
    quiz_ids.push_back(Symbol::new(&env, "quiz_1"));

    progress_client.create_course(&course_a, &1, &1, &module_ids, &quiz_ids);
    progress_client.create_course(&course_b, &1, &1, &module_ids, &quiz_ids);

    // Enrolling in two different courses is valid — not double-spending
    progress_client.enroll(&learner, &course_a);
    progress_client.enroll(&learner, &course_b);

    let courses = progress_client.get_learner_courses(&learner);
    assert_eq!(courses.len(), 2);
}

// ── Double-complete module ──────────────────────────────────────────────────

#[test]
#[should_panic(expected = "module already completed")]
fn test_double_complete_module_is_prevented() {
    let env = Env::default();
    let (_admin, _token_client, _credential_client, progress_client) = setup_env(&env);
    let learner = Address::generate(&env);
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "course_1");
    let mut module_ids = Vec::new(&env);
    module_ids.push_back(Symbol::new(&env, "mod_1"));
    let mut quiz_ids = Vec::new(&env);
    quiz_ids.push_back(Symbol::new(&env, "quiz_1"));
    progress_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
    progress_client.enroll(&learner, &course_id);

    // First completion succeeds
    progress_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));

    // Second completion for the same module must be rejected
    progress_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
}

// ── Double-submit quiz ──────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "quiz already submitted")]
fn test_double_submit_quiz_is_prevented() {
    let env = Env::default();
    let (_admin, _token_client, _credential_client, progress_client) = setup_env(&env);
    let learner = Address::generate(&env);
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "course_1");
    let mut module_ids = Vec::new(&env);
    module_ids.push_back(Symbol::new(&env, "mod_1"));
    let mut quiz_ids = Vec::new(&env);
    quiz_ids.push_back(Symbol::new(&env, "quiz_1"));
    progress_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
    progress_client.enroll(&learner, &course_id);

    // First submission succeeds
    progress_client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);

    // Second submission for the same quiz must be rejected
    progress_client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &90);
}

// ── Double-revoke credential ────────────────────────────────────────────────

#[test]
#[should_panic(expected = "credential already revoked")]
fn test_double_revoke_credential_is_prevented() {
    let env = Env::default();
    let (_admin, _token_client, credential_client, progress_client) = setup_env(&env);
    let learner = Address::generate(&env);
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "course_1");
    let metadata_uri = Symbol::new(&env, "ipfs_Qm123");
    create_course_and_complete(&env, &progress_client, &learner, &course_id, 85);

    let cred_id = credential_client.mint_credential(&learner, &course_id, &85, &metadata_uri);

    // First revocation succeeds
    credential_client.revoke_credential(&cred_id);

    // Second revocation must be rejected
    credential_client.revoke_credential(&cred_id);
}

// ── Cross-contract double-spend: claim reward then mint credential ──────────

#[test]
fn test_claim_reward_then_mint_credential_are_independent() {
    let env = Env::default();
    let (_admin, token_client, credential_client, progress_client) = setup_env(&env);
    let learner = Address::generate(&env);
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "course_1");
    let metadata_uri = Symbol::new(&env, "ipfs_Qm123");
    create_course_and_complete(&env, &progress_client, &learner, &course_id, 85);

    // Claim reward (mints tokens)
    token_client.claim_reward(&learner, &course_id, &Symbol::new(&env, "quiz_1"));
    assert_eq!(token_client.balance(&learner), 8500);

    // Mint credential (mints NFT) — independent operation, not double-spending
    credential_client.mint_credential(&learner, &course_id, &85, &metadata_uri);
    assert!(credential_client.is_credential_valid(&1));

    // Both succeeded — they are different contract surfaces
    assert_eq!(token_client.total_supply(), 8500);
    assert_eq!(credential_client.get_credential_count(&learner), 1);
}
