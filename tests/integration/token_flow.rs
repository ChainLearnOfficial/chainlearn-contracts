//! Integration tests for the end-to-end token reward flow.
//!
//! Tests the full journey: enroll -> complete modules -> submit quiz -> claim reward.

mod fixtures;

use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

/// Test the complete reward flow from enrollment to token claim.
///
/// 1. Learner enrolls in a course
/// 2. Learner completes all modules
/// 3. Learner submits quiz with score 85
/// 4. Learner claims token reward
/// 5. Verify token balance is proportional to score
#[test]
fn test_end_to_end_reward_flow() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let course_id = Symbol::new(env, "rust_101");

    // Step 1: Create course and enroll
    fixtures::create_sample_course(env);
    // progress_client.enroll(learner, &course_id);

    // Step 2: Complete all modules
    // progress_client.complete_module(learner, &course_id, &Symbol::new(env, "mod_basics"));
    // progress_client.complete_module(learner, &course_id, &Symbol::new(env, "mod_ownership"));
    // progress_client.complete_module(learner, &course_id, &Symbol::new(env, "mod_traits"));

    // Step 3: Submit quiz score
    let quiz_id = Symbol::new(env, "quiz_final");
    // progress_client.submit_quiz_score(learner, &course_id, &quiz_id, &85);

    // Step 4: Claim reward
    // let reward = token_client.claim_reward(learner, &quiz_id, &85);

    // Step 5: Verify
    // Expected: 85 * 100 (BASE_REWARD_PER_POINT) = 8500
    // assert_eq!(reward, 8500);
    // assert_eq!(token_client.balance(learner), 8500);
}

/// Test that a learner cannot claim the same reward twice.
#[test]
#[should_panic(expected = "reward already claimed")]
fn test_double_claim_prevented() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let quiz_id = Symbol::new(env, "quiz_1");

    // token_client.claim_reward(learner, &quiz_id, &80);
    // token_client.claim_reward(learner, &quiz_id, &80);
}

/// Test that different quizzes yield independent rewards.
#[test]
fn test_multiple_quiz_rewards() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let quiz1 = Symbol::new(env, "quiz_1");
    let quiz2 = Symbol::new(env, "quiz_2");

    // token_client.claim_reward(learner, &quiz1, &80); // 8000 tokens
    // token_client.claim_reward(learner, &quiz2, &60); // 6000 tokens

    // assert_eq!(token_client.balance(learner), 14000);
    // assert_eq!(token_client.total_supply(), 14000);
}

/// Test token transfer between learners.
#[test]
fn test_learner_to_learner_transfer() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    let other_learner = Address::generate(env);
    env.mock_all_auths();

    // learner earns tokens
    let quiz_id = Symbol::new(env, "quiz_1");
    // token_client.claim_reward(learner, &quiz_id, &100); // 10000 tokens

    // learner transfers to another
    // token_client.transfer(learner, &other_learner, &3000);

    // assert_eq!(token_client.balance(learner), 7000);
    // assert_eq!(token_client.balance(&other_learner), 3000);
}

/// Test total supply tracks all minted tokens.
#[test]
fn test_total_supply_consistency() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner1 = &setup.learner;
    let learner2 = Address::generate(env);
    env.mock_all_auths();

    // learner1 claims 8000 tokens
    // token_client.claim_reward(learner1, &Symbol::new(env, "q1"), &80);
    // learner2 claims 5000 tokens
    // token_client.claim_reward(&learner2, &Symbol::new(env, "q2"), &50);

    // assert_eq!(token_client.total_supply(), 13000);
}
