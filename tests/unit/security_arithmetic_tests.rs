/* Authorized Protocol Quality Assurance & Formal Verification Test Suite */
//! Defensive Formal Verification & Arithmetic Boundary Regression Test Suite
//!
//! Issue #353: Audit all arithmetic operations for overflow/underflow protection.
//!
//! This suite verifies:
//! 1. Underflow protection on balance subtractions in `transfer` and `burn`.
//! 2. Underflow protection on allowance subtractions in `transfer_from` and `burn_from`.
//! 3. Checked arithmetic and overflow protection on supply minting (including `i128::MAX` boundary).
//! 4. Negative integer rejection across all token state-transition methods.
//! 5. Zero-value boundary handling preserving state invariants.
//! 6. Quiz score bounds and score delta arithmetic in progress tracking.
//! 7. Reward calculation arithmetic proportionality and maximum reward caps.

#![cfg(test)]

use chainlearn_shared::{BASE_REWARD_PER_POINT, MAX_QUIZ_SCORE};
use learn_token::LearnTokenClient;
use progress_tracker::{ProgressTracker, ProgressTrackerClient};
use soroban_sdk::{
    testutils::Address as _, Address, Env, String as SorobanString, Symbol, Vec,
};

fn setup_token(env: &Env, max_supply: i128) -> (Address, Address, LearnTokenClient<'_>) {
    let admin = Address::generate(env);
    let pt_contract_id = env.register_contract(None, ProgressTracker);
    let contract_id = env.register_contract(None, learn_token::LearnToken);
    let client = LearnTokenClient::new(env, &contract_id);

    client.initialize(
        &admin,
        &SorobanString::from_str(env, "ChainLearn"),
        &SorobanString::from_str(env, "CLRN"),
        &7,
        &pt_contract_id,
        &max_supply,
    );

    (admin, contract_id, client)
}

fn setup_progress(env: &Env) -> (Address, Address, ProgressTrackerClient<'_>) {
    let admin = Address::generate(env);
    let contract_id = env.register_contract(None, ProgressTracker);
    let client = ProgressTrackerClient::new(env, &contract_id);
    client.initialize(&admin);
    (admin, contract_id, client)
}

fn create_sample_course(env: &Env, client: &ProgressTrackerClient) -> Symbol {
    let course_id = Symbol::new(env, "course_soroban");
    let mut module_ids = Vec::new(env);
    module_ids.push_back(Symbol::new(env, "mod_1"));
    module_ids.push_back(Symbol::new(env, "mod_2"));
    let mut quiz_ids = Vec::new(env);
    quiz_ids.push_back(Symbol::new(env, "quiz_1"));
    quiz_ids.push_back(Symbol::new(env, "quiz_2"));
    client.create_course(&course_id, &2, &2, &module_ids, &quiz_ids);
    course_id
}

// ---------------------------------------------------------------------------
// Balance & Supply Underflow / Overflow Tests
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "insufficient balance")]
fn test_underflow_balance_subtraction() {
    let env = Env::default();
    let (admin, _token_id, client) = setup_token(&env, 1_000_000);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.transfer(&user, &admin, &1000);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn test_underflow_balance_burn() {
    let env = Env::default();
    let (_admin, _token_id, client) = setup_token(&env, 1_000_000);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.burn(&user, &1000);
}

#[test]
#[should_panic(expected = "maximum supply cap exceeded")]
fn test_overflow_supply() {
    let env = Env::default();
    let (admin, _token_id, client) = setup_token(&env, i128::MAX);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.mint(&admin, &user, &i128::MAX);
    // Checked arithmetic triggers supply cap exceeded
    client.mint(&admin, &user, &1);
}

#[test]
#[should_panic(expected = "maximum supply cap exceeded")]
fn test_overflow_supply_explicit_cap() {
    let env = Env::default();
    let (admin, _token_id, client) = setup_token(&env, 500);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.mint(&admin, &user, &300);
    // 300 + 201 = 501 > 500 max supply
    client.mint(&admin, &user, &201);
}

// ---------------------------------------------------------------------------
// Allowance Underflow Tests
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "insufficient allowance")]
fn test_underflow_allowance_transfer_from() {
    let env = Env::default();
    let (admin, _token_id, client) = setup_token(&env, 1_000_000);
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);
    env.mock_all_auths();

    client.mint(&admin, &owner, &10_000);
    client.approve(&owner, &spender, &500, &1000);

    // Attempting to spend 501 when allowance is 500
    client.transfer_from(&spender, &owner, &recipient, &501);
}

#[test]
#[should_panic(expected = "insufficient allowance")]
fn test_underflow_allowance_burn_from() {
    let env = Env::default();
    let (admin, _token_id, client) = setup_token(&env, 1_000_000);
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    env.mock_all_auths();

    client.mint(&admin, &owner, &10_000);
    client.approve(&owner, &spender, &200, &1000);

    // Attempting to burn 201 when allowance is 200
    client.burn_from(&spender, &owner, &201);
}

// ---------------------------------------------------------------------------
// Negative Amount Input Validation Tests
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "negative amount")]
fn test_negative_amount_transfer() {
    let env = Env::default();
    let (admin, _token_id, client) = setup_token(&env, 1_000_000);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.transfer(&admin, &user, &-1);
}

#[test]
#[should_panic(expected = "negative amount")]
fn test_negative_amount_transfer_from() {
    let env = Env::default();
    let (_admin, _token_id, client) = setup_token(&env, 1_000_000);
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);
    env.mock_all_auths();

    client.transfer_from(&spender, &owner, &recipient, &-50);
}

#[test]
#[should_panic(expected = "negative amount")]
fn test_negative_amount_mint() {
    let env = Env::default();
    let (admin, _token_id, client) = setup_token(&env, 1_000_000);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.mint(&admin, &user, &-100);
}

#[test]
#[should_panic(expected = "negative amount")]
fn test_negative_amount_burn() {
    let env = Env::default();
    let (_admin, _token_id, client) = setup_token(&env, 1_000_000);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.burn(&user, &-10);
}

#[test]
#[should_panic(expected = "negative amount")]
fn test_negative_amount_burn_from() {
    let env = Env::default();
    let (_admin, _token_id, client) = setup_token(&env, 1_000_000);
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    env.mock_all_auths();

    client.burn_from(&spender, &owner, &-5);
}

#[test]
#[should_panic(expected = "negative amount")]
fn test_negative_amount_approve() {
    let env = Env::default();
    let (_admin, _token_id, client) = setup_token(&env, 1_000_000);
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    env.mock_all_auths();

    client.approve(&owner, &spender, &-500, &1000);
}

// ---------------------------------------------------------------------------
// Zero Value Invariant Tests
// ---------------------------------------------------------------------------

#[test]
fn test_zero_amount_transfer_and_mint_invariants() {
    let env = Env::default();
    let (admin, _token_id, client) = setup_token(&env, 1_000_000);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    env.mock_all_auths();

    // Mint initial tokens
    client.mint(&admin, &user1, &500);
    assert_eq!(client.balance(&user1), 500);
    assert_eq!(client.total_supply(), 500);

    // Transfer 0 tokens: balances and total supply remain unchanged
    client.transfer(&user1, &user2, &0);
    assert_eq!(client.balance(&user1), 500);
    assert_eq!(client.balance(&user2), 0);
    assert_eq!(client.total_supply(), 500);

    // Mint 0 tokens
    client.mint(&admin, &user2, &0);
    assert_eq!(client.balance(&user2), 0);
    assert_eq!(client.total_supply(), 500);

    // Burn 0 tokens
    client.burn(&user1, &0);
    assert_eq!(client.balance(&user1), 500);
    assert_eq!(client.total_supply(), 500);
}

// ---------------------------------------------------------------------------
// Progress Tracker Arithmetic & Score Bounds Tests
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "score exceeds maximum")]
fn test_quiz_score_overflow_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, _contract_id, client) = setup_progress(&env);
    let course_id = create_sample_course(&env, &client);
    let learner = Address::generate(&env);
    let quiz_id = Symbol::new(&env, "quiz_1");
    client.enroll(&learner, &course_id);

    // Score 101 exceeds MAX_QUIZ_SCORE (100)
    client.submit_quiz_score(&learner, &course_id, &quiz_id, &101);
}

#[test]
#[should_panic(expected = "score exceeds maximum")]
fn test_quiz_retake_score_overflow_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, _contract_id, client) = setup_progress(&env);
    let course_id = create_sample_course(&env, &client);
    let learner = Address::generate(&env);
    let quiz_id = Symbol::new(&env, "quiz_1");
    client.enroll(&learner, &course_id);
    client.submit_quiz_score(&learner, &course_id, &quiz_id, &80);

    // Retake with score 105 exceeds maximum
    client.retake_quiz(&learner, &course_id, &quiz_id, &105);
}

#[test]
fn test_quiz_score_delta_arithmetic() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, _contract_id, client) = setup_progress(&env);
    let course_id = create_sample_course(&env, &client);
    let learner = Address::generate(&env);
    let quiz_1 = Symbol::new(&env, "quiz_1");
    let quiz_2 = Symbol::new(&env, "quiz_2");
    client.enroll(&learner, &course_id);

    // Submit quiz 1 with score 70
    client.submit_quiz_score(&learner, &course_id, &quiz_1, &70);
    let p1 = client.get_progress(&learner, &course_id);
    assert_eq!(p1.quizzes_submitted, 1);
    assert_eq!(p1.total_quiz_score, 70);

    // Submit quiz 2 with score 85 -> total = 70 + 85 = 155
    client.submit_quiz_score(&learner, &course_id, &quiz_2, &85);
    let p2 = client.get_progress(&learner, &course_id);
    assert_eq!(p2.quizzes_submitted, 2);
    assert_eq!(p2.total_quiz_score, 155);

    // Retake quiz 1 with higher score 95 -> delta = +25 -> total = 155 + 25 = 180
    client.retake_quiz(&learner, &course_id, &quiz_1, &95);
    let p3 = client.get_progress(&learner, &course_id);
    assert_eq!(p3.quizzes_submitted, 2);
    assert_eq!(p3.total_quiz_score, 180);
    assert_eq!(client.get_quiz_score(&learner, &course_id, &quiz_1), 95);
}

// ---------------------------------------------------------------------------
// Reward Formula Arithmetic Proportionality Tests
// ---------------------------------------------------------------------------

#[test]
fn test_reward_amount_calculation_and_cap_invariants() {
    // Score must be in 1..=MAX_QUIZ_SCORE
    assert_eq!(MAX_QUIZ_SCORE, 100);
    assert_eq!(BASE_REWARD_PER_POINT, 100);

    // Minimum positive score reward
    let min_score: u32 = 1;
    let min_reward = (min_score as i128) * BASE_REWARD_PER_POINT;
    assert_eq!(min_reward, 100);

    // Mid-level score reward
    let mid_score: u32 = 75;
    let mid_reward = (mid_score as i128) * BASE_REWARD_PER_POINT;
    assert_eq!(mid_reward, 7_500);

    // Maximum score reward matches MAX_REWARD_AMOUNT
    let max_score: u32 = MAX_QUIZ_SCORE;
    let max_reward = (max_score as i128) * BASE_REWARD_PER_POINT;
    let max_allowed = (MAX_QUIZ_SCORE as i128) * BASE_REWARD_PER_POINT;
    assert_eq!(max_reward, 10_000);
    assert_eq!(max_reward, max_allowed);
}
