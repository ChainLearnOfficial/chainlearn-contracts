//! Integration tests for the end-to-end token reward flow.
//!
//! Tests the full journey: enroll -> complete modules -> submit quiz -> claim reward.

mod fixtures;

use learn_token::LearnTokenClient;
use progress_tracker::ProgressTrackerClient;
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol, Vec};

/// Create a minimal course (one module, just the given quiz ids) and enroll
/// `learner` in it. `claim_reward` only requires enrollment plus a submitted
/// quiz score -- not module completion -- so tests that only exercise the
/// reward flow don't need a full course.
fn setup_course_and_enroll(
    env: &Env,
    progress_client: &ProgressTrackerClient,
    learner: &Address,
    course_id: &Symbol,
    quiz_ids: &[Symbol],
) {
    let mut module_ids = Vec::new(env);
    module_ids.push_back(Symbol::new(env, "mod_1"));
    let mut quiz_ids_vec = Vec::new(env);
    for q in quiz_ids {
        quiz_ids_vec.push_back(q.clone());
    }
    progress_client.create_course(course_id, &1, &(quiz_ids.len() as u32), &module_ids, &quiz_ids_vec);
    progress_client.enroll(learner, course_id);
}

#[test]
fn test_end_to_end_reward_flow() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let token_client = LearnTokenClient::new(env, &setup.token_contract_id);
    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);

    let course_id = fixtures::create_sample_course(env, &progress_client);

    // Enroll and complete all modules
    progress_client.enroll(learner, &course_id);
    progress_client.complete_module(learner, &course_id, &Symbol::new(env, "mod_basics"));
    progress_client.complete_module(learner, &course_id, &Symbol::new(env, "mod_ownership"));
    progress_client.complete_module(learner, &course_id, &Symbol::new(env, "mod_traits"));

    // Submit quiz score
    let quiz_id = Symbol::new(env, "quiz_final");
    progress_client.submit_quiz_score(learner, &course_id, &quiz_id, &85);

    // Claim reward -- claim_reward(learner, course_id, quiz_id) fetches the
    // score from progress-tracker itself rather than taking it as an arg.
    token_client.claim_reward(learner, &course_id, &quiz_id);

    // Verify: 85 * 100 (BASE_REWARD_PER_POINT) = 8500
    assert_eq!(token_client.balance(learner), 8500);
}

#[test]
#[should_panic(expected = "reward already claimed")]
fn test_double_claim_prevented() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let token_client = LearnTokenClient::new(env, &setup.token_contract_id);
    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);

    let course_id = Symbol::new(env, "course_1");
    let quiz_id = Symbol::new(env, "quiz_1");
    setup_course_and_enroll(env, &progress_client, learner, &course_id, &[quiz_id.clone()]);
    progress_client.submit_quiz_score(learner, &course_id, &quiz_id, &80);

    token_client.claim_reward(learner, &course_id, &quiz_id);
    token_client.claim_reward(learner, &course_id, &quiz_id);
}

#[test]
fn test_multiple_quiz_rewards() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let token_client = LearnTokenClient::new(env, &setup.token_contract_id);
    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);

    let course_id = Symbol::new(env, "course_1");
    let quiz1 = Symbol::new(env, "quiz_1");
    let quiz2 = Symbol::new(env, "quiz_2");
    setup_course_and_enroll(env, &progress_client, learner, &course_id, &[quiz1.clone(), quiz2.clone()]);
    progress_client.submit_quiz_score(learner, &course_id, &quiz1, &80);
    progress_client.submit_quiz_score(learner, &course_id, &quiz2, &60);

    token_client.claim_reward(learner, &course_id, &quiz1); // 8000 tokens
    token_client.claim_reward(learner, &course_id, &quiz2); // 6000 tokens

    assert_eq!(token_client.balance(learner), 14000);
    assert_eq!(token_client.total_supply(), 14000);
}

#[test]
fn test_learner_to_learner_transfer() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    let other_learner = Address::generate(env);
    env.mock_all_auths();

    let token_client = LearnTokenClient::new(env, &setup.token_contract_id);
    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);

    let course_id = Symbol::new(env, "course_1");
    let quiz_id = Symbol::new(env, "quiz_1");
    setup_course_and_enroll(env, &progress_client, learner, &course_id, &[quiz_id.clone()]);
    progress_client.submit_quiz_score(learner, &course_id, &quiz_id, &100);

    token_client.claim_reward(learner, &course_id, &quiz_id); // 10000 tokens

    token_client.transfer(learner, &other_learner, &3000);

    assert_eq!(token_client.balance(learner), 7000);
    assert_eq!(token_client.balance(&other_learner), 3000);
}

#[test]
fn test_total_supply_consistency() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner1 = &setup.learner;
    let learner2 = Address::generate(env);
    env.mock_all_auths();

    let token_client = LearnTokenClient::new(env, &setup.token_contract_id);
    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);

    let course_id = Symbol::new(env, "course_1");
    let q1 = Symbol::new(env, "q1");
    let q2 = Symbol::new(env, "q2");
    setup_course_and_enroll(env, &progress_client, learner1, &course_id, &[q1.clone(), q2.clone()]);
    progress_client.enroll(&learner2, &course_id);
    progress_client.submit_quiz_score(learner1, &course_id, &q1, &80);
    progress_client.submit_quiz_score(&learner2, &course_id, &q2, &50);

    token_client.claim_reward(learner1, &course_id, &q1); // 8000
    token_client.claim_reward(&learner2, &course_id, &q2); // 5000

    assert_eq!(token_client.total_supply(), 13000);
}
