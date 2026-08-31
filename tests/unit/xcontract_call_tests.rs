//! Unit tests for cross-contract calls between learn-token and progress-tracker.
//!
//! Verifies that learn-token correctly calls into progress-tracker to fetch
//! quiz scores, handles various response scenarios, and maintains consistent
//! state when cross-contract calls fail.

use learn_token::LearnTokenClient;
use progress_tracker::{ProgressTracker, ProgressTrackerClient};
use soroban_sdk::{
    testutils::Address as _, Address, Env, String as SorobanString, Symbol, Vec,
};

#[cfg(test)]
mod xcontract_call_tests {
    use super::*;

    fn setup(env: &Env) -> (Address, Address, Address) {
        let admin = Address::generate(env);

        let pt_contract_id = env.register_contract(None, ProgressTracker);
        let pt_client = ProgressTrackerClient::new(env, &pt_contract_id);
        pt_client.initialize(&admin);

        let token_contract_id = env.register_contract(None, learn_token::LearnToken);
        let token_client = LearnTokenClient::new(env, &token_contract_id);
        token_client.initialize(
            &admin,
            &SorobanString::from_str(env, "CLearn"),
            &SorobanString::from_str(env, "CLRN"),
            &7,
            &pt_contract_id,
            &1_000_000_000_000_000,
        );

        (admin, token_contract_id, pt_contract_id)
    }

    fn create_course_and_submit_quiz(
        env: &Env,
        pt_client: &ProgressTrackerClient,
        learner: &Address,
        course_id: &Symbol,
        quiz_id: &Symbol,
        score: u32,
    ) {
        let mut module_ids = Vec::new(env);
        module_ids.push_back(Symbol::new(env, "mod_1"));
        let mut quiz_ids = Vec::new(env);
        quiz_ids.push_back(quiz_id.clone());
        pt_client.create_course(course_id, &1, &1, &module_ids, &quiz_ids);
        pt_client.enroll(learner, course_id);
        pt_client.submit_quiz_score(learner, course_id, quiz_id, &score);
    }

    // ── Successful cross-contract calls ──────────────────────────────────

    #[test]
    fn test_claim_reward_fetches_score_from_progress_tracker() {
        let env = Env::default();
        let (_admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 80);

        // claim_reward calls get_quiz_score on progress-tracker via
        // env.invoke_contract. Score 80 * BASE_REWARD_PER_POINT (100) = 8000.
        token_client.claim_reward(&learner, &course_id, &quiz_id);
        assert_eq!(token_client.balance(&learner), 8000);
        assert_eq!(token_client.total_supply(), 8000);
    }

    #[test]
    fn test_cross_contract_call_uses_correct_score_for_reward() {
        let env = Env::default();
        let (_admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 100);

        token_client.claim_reward(&learner, &course_id, &quiz_id);
        // Score 100 * 100 = 10000
        assert_eq!(token_client.balance(&learner), 10000);
    }

    #[test]
    fn test_cross_contract_call_with_low_score() {
        let env = Env::default();
        let (_admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 1);

        token_client.claim_reward(&learner, &course_id, &quiz_id);
        // Score 1 * 100 = 100
        assert_eq!(token_client.balance(&learner), 100);
    }

    // ── Cross-contract call failures ─────────────────────────────────────

    #[test]
    #[should_panic(expected = "quiz not submitted")]
    fn test_claim_reward_fails_when_quiz_not_submitted() {
        let env = Env::default();
        let (_admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        let mut module_ids = Vec::new(&env);
        module_ids.push_back(Symbol::new(&env, "mod_1"));
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(quiz_id.clone());
        pt_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
        pt_client.enroll(&learner, &course_id);
        // Do NOT submit the quiz score — the cross-contract call will fail

        token_client.claim_reward(&learner, &course_id, &quiz_id);
    }

    #[test]
    #[should_panic(expected = "quiz not submitted")]
    fn test_claim_reward_fails_when_quiz_does_not_exist_in_course() {
        let env = Env::default();
        let (_admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "real_quiz");
        let fake_quiz_id = Symbol::new(&env, "fake_quiz");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 80);

        // Try to claim for a quiz_id that was never submitted — the
        // cross-contract get_quiz_score call returns an error
        token_client.claim_reward(&learner, &course_id, &fake_quiz_id);
    }

    #[test]
    #[should_panic(expected = "course not found")]
    fn test_claim_reward_fails_when_course_does_not_exist() {
        let env = Env::default();
        let (_admin, token_id, _pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "nonexistent_course");
        let quiz_id = Symbol::new(&env, "quiz_1");

        token_client.claim_reward(&learner, &course_id, &quiz_id);
    }

    // ── State consistency after failed cross-contract calls ──────────────

    #[test]
    fn test_failed_claim_does_not_corrupt_token_state() {
        let env = Env::default();
        let (_admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 80);

        let supply_before = token_client.total_supply();
        let balance_before = token_client.balance(&learner);

        // Attempt to claim for a non-existent quiz — cross-contract call fails
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            token_client.claim_reward(&learner, &course_id, &Symbol::new(&env, "nonexistent"));
        }));
        assert!(result.is_err(), "cross-contract failure should revert");

        // Token state is unchanged
        assert_eq!(token_client.total_supply(), supply_before);
        assert_eq!(token_client.balance(&learner), balance_before);
    }

    #[test]
    fn test_batch_claim_handles_mixed_success_and_failure() {
        let env = Env::default();
        let (_admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let mut module_ids = Vec::new(&env);
        module_ids.push_back(Symbol::new(&env, "mod_1"));
        let mut quiz_ids_src = Vec::new(&env);
        quiz_ids_src.push_back(Symbol::new(&env, "quiz_real"));
        quiz_ids_src.push_back(Symbol::new(&env, "quiz_fake"));
        pt_client.create_course(&course_id, &1, &2, &module_ids, &quiz_ids_src);
        pt_client.enroll(&learner, &course_id);
        pt_client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_real"), &80);
        // quiz_fake is NOT submitted — the cross-contract call will return 0

        let mut claim_ids = Vec::new(&env);
        claim_ids.push_back(Symbol::new(&env, "quiz_real"));
        claim_ids.push_back(Symbol::new(&env, "quiz_fake"));

        let successful = token_client.batch_claim_reward(&learner, &course_id, &claim_ids);

        // quiz_real succeeds, quiz_fake is skipped (score 0 from cross-contract)
        assert_eq!(successful.len(), 1);
        assert_eq!(token_client.balance(&learner), 8000);
    }

    // ── Cross-contract state isolation ───────────────────────────────────

    #[test]
    fn test_progress_tracker_state_unaffected_by_claim() {
        let env = Env::default();
        let (_admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 80);

        // Record progress-tracker state before claim
        let progress_before = pt_client.get_progress(&learner, &course_id);
        let score_before = pt_client.get_quiz_score(&learner, &course_id, &quiz_id);

        // Claim reward — crosses into progress-tracker but should not modify it
        token_client.claim_reward(&learner, &course_id, &quiz_id);

        // Progress-tracker state is unchanged
        let progress_after = pt_client.get_progress(&learner, &course_id);
        let score_after = pt_client.get_quiz_score(&learner, &course_id, &quiz_id);
        assert_eq!(progress_before.overall_progress, progress_after.overall_progress);
        assert_eq!(progress_before.quizzes_submitted, progress_after.quizzes_submitted);
        assert_eq!(score_before, score_after);
    }

    // ── Multiple learners, same course ───────────────────────────────────

    #[test]
    fn test_cross_contract_calls_for_different_learners_are_isolated() {
        let env = Env::default();
        let (_admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        let mut module_ids = Vec::new(&env);
        module_ids.push_back(Symbol::new(&env, "mod_1"));
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(quiz_id.clone());
        pt_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);

        // Alice scores 80
        pt_client.enroll(&alice, &course_id);
        pt_client.submit_quiz_score(&alice, &course_id, &quiz_id, &80);
        // Bob scores 90
        pt_client.enroll(&bob, &course_id);
        pt_client.submit_quiz_score(&bob, &course_id, &quiz_id, &90);

        // Claims are cross-contract but learner-specific
        token_client.claim_reward(&alice, &course_id, &quiz_id);
        token_client.claim_reward(&bob, &course_id, &quiz_id);

        assert_eq!(token_client.balance(&alice), 8000);
        assert_eq!(token_client.balance(&bob), 9000);
    }
}
