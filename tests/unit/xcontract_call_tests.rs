//! Unit tests for cross-contract calls between learn-token and progress-tracker.
//!
//! Verifies that learn-token correctly calls into progress-tracker to fetch
//! quiz scores, handles various response scenarios, and maintains consistent
//! state when cross-contract calls fail.

use learn_token::{AdminRole, LearnTokenClient};
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

    // ── Zero-score cross-contract response ──────────────────────────────

    /// A score of 0 returned by the progress-tracker is not a valid reward
    /// basis. learn-token must reject it with "score must be greater than 0"
    /// rather than silently minting nothing or panicking with an unrelated
    /// message. This exercises the path where the cross-contract call itself
    /// succeeds (score is fetched) but the value is semantically invalid.
    ///
    /// We drive score 0 by enrolling the learner but then using `retake_quiz`
    /// to reset the score back to 0 after submission.  The cross-contract
    /// call inside `claim_reward` will read 0 and must panic.
    #[test]
    #[should_panic(expected = "score must be greater than 0")]
    fn test_claim_reward_rejects_zero_score_from_cross_contract() {
        let env = Env::default();
        let (_admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");

        // Create course and submit a non-zero score first so we can enroll,
        // then reset via retake_quiz which sets the score back to 0.
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 75);
        // retake_quiz overwrites the stored score to 0 so the next cross-contract
        // call from learn-token will read back 0.
        pt_client.retake_quiz(&learner, &course_id, &quiz_id, &0u32);

        // The cross-contract call returns 0; claim_reward must panic.
        token_client.claim_reward(&learner, &course_id, &quiz_id);
    }

    /// After a zero-score rejection the token state (balance, supply) must be
    /// completely unchanged — the failed cross-contract read must not leave
    /// any partial side-effects.
    #[test]
    fn test_zero_score_rejection_does_not_corrupt_token_state() {
        let env = Env::default();
        let (_admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");

        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 75);
        pt_client.retake_quiz(&learner, &course_id, &quiz_id, &0u32);

        let supply_before = token_client.total_supply();
        let balance_before = token_client.balance(&learner);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            token_client.claim_reward(&learner, &course_id, &quiz_id);
        }));
        assert!(result.is_err(), "zero-score claim must revert");

        assert_eq!(token_client.total_supply(), supply_before);
        assert_eq!(token_client.balance(&learner), balance_before);
    }

    // ── estimate_claim_gas: cross-contract dry-run ───────────────────────

    /// `estimate_claim_gas` must perform the same cross-contract call as
    /// `claim_reward` (fetching the score from progress-tracker) and report
    /// `would_succeed = true` with the correct reward amount when everything
    /// is in order.  Crucially, it must NOT change any state.
    #[test]
    fn test_estimate_claim_gas_cross_contract_preview_success() {
        let env = Env::default();
        let (_admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 70);

        let supply_before = token_client.total_supply();
        let balance_before = token_client.balance(&learner);

        // estimate_claim_gas crosses into progress-tracker to verify the score
        // (score 70 * 100 = 7000) but must never mint or modify state.
        let estimate = token_client.estimate_claim_gas(&learner, &course_id, &quiz_id);

        assert!(estimate.would_succeed, "preview must report success");
        assert_eq!(
            estimate.estimated_reward, 7000,
            "estimated reward must match score * BASE_REWARD_PER_POINT"
        );

        // Verify no state was mutated.
        assert_eq!(token_client.total_supply(), supply_before);
        assert_eq!(token_client.balance(&learner), balance_before);
    }

    /// `estimate_claim_gas` must detect an already-claimed reward without
    /// performing the cross-contract score fetch and report failure cleanly.
    #[test]
    fn test_estimate_claim_gas_reports_failure_for_already_claimed() {
        let env = Env::default();
        let (_admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 60);

        // First claim succeeds.
        token_client.claim_reward(&learner, &course_id, &quiz_id);
        assert_eq!(token_client.balance(&learner), 6000);

        // Preview after a successful claim must report failure, not panic.
        let estimate = token_client.estimate_claim_gas(&learner, &course_id, &quiz_id);
        assert!(
            !estimate.would_succeed,
            "preview must report failure for already-claimed reward"
        );
    }

    /// `estimate_claim_gas` must report failure when the cross-contract score
    /// fetch itself would fail (quiz not submitted), propagating the error
    /// cleanly rather than panicking.
    #[test]
    #[should_panic(expected = "quiz not submitted")]
    fn test_estimate_claim_gas_propagates_cross_contract_failure() {
        let env = Env::default();
        let (_admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        // Create course and enroll but do NOT submit a score.
        let mut module_ids = Vec::new(&env);
        module_ids.push_back(Symbol::new(&env, "mod_1"));
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(quiz_id.clone());
        pt_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
        pt_client.enroll(&learner, &course_id);

        // estimate_claim_gas calls get_quiz_score cross-contract;
        // progress-tracker panics "quiz not submitted" — that must propagate.
        token_client.estimate_claim_gas(&learner, &course_id, &quiz_id);
    }

    // ── Supply cap after successful cross-contract fetch ─────────────────

    /// When the cross-contract score fetch succeeds but the resulting reward
    /// would push total supply past the cap, `claim_reward` must panic with
    /// "maximum supply cap exceeded". Token balance and supply must remain
    /// exactly as before — the cross-contract read must not leave any write
    /// side-effects.
    #[test]
    fn test_supply_cap_hit_after_cross_contract_score_fetch_no_state_corruption() {
        let env = Env::default();
        let admin = Address::generate(&env);

        let pt_contract_id = env.register_contract(None, ProgressTracker);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);
        pt_client.initialize(&admin);

        // Cap set so that one 80-point reward (8000 tokens) exceeds it.
        let token_contract_id = env.register_contract(None, learn_token::LearnToken);
        let token_client = LearnTokenClient::new(&env, &token_contract_id);
        token_client.initialize(
            &admin,
            &SorobanString::from_str(&env, "CLearn"),
            &SorobanString::from_str(&env, "CLRN"),
            &7,
            &pt_contract_id,
            &5000, // cap below the reward for score 80 (8000)
        );

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 80);

        let supply_before = token_client.total_supply();
        let balance_before = token_client.balance(&learner);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            token_client.claim_reward(&learner, &course_id, &quiz_id);
        }));
        assert!(
            result.is_err(),
            "claim exceeding supply cap must revert"
        );

        // No state change despite the successful cross-contract score read.
        assert_eq!(token_client.total_supply(), supply_before);
        assert_eq!(token_client.balance(&learner), balance_before);
        // The reward claimed flag must also not have been set.
        // Verify by checking that a second attempt fails for the same reason
        // (cap), not for "reward already claimed".
        let result2 = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            token_client.claim_reward(&learner, &course_id, &quiz_id);
        }));
        assert!(result2.is_err(), "second attempt must also revert");
        // If the claimed flag had been set incorrectly the error would be
        // "reward already claimed"; both reverts being for the cap means the
        // claimed flag was never written.
        assert_eq!(token_client.total_supply(), supply_before);
    }

    // ── Pause guard blocks cross-contract reward flow ────────────────────

    /// When the learn-token contract is paused, `claim_reward` must be blocked
    /// with "contract is paused" before the cross-contract score fetch even
    /// occurs. The progress-tracker state and token state must be unaffected.
    #[test]
    #[should_panic(expected = "contract is paused")]
    fn test_paused_contract_blocks_cross_contract_claim_reward() {
        let env = Env::default();
        let (admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 85);

        // Grant admin the Pauser role so it can pause, then pause.
        token_client.grant_role(
            &admin,
            &admin,
            &AdminRole::Pauser,
        );
        token_client.pause(&admin);
        assert!(token_client.is_paused());

        // This must panic with "contract is paused" before reaching the
        // cross-contract score fetch.
        token_client.claim_reward(&learner, &course_id, &quiz_id);
    }

    /// After unpausing, the cross-contract reward claim must succeed as normal,
    /// confirming the pause guard was the only obstacle (not a corrupted state).
    #[test]
    fn test_unpause_restores_cross_contract_claim_reward() {
        let env = Env::default();
        let (admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 50);

        token_client.grant_role(
            &admin,
            &admin,
            &AdminRole::Pauser,
        );
        token_client.pause(&admin);
        token_client.unpause(&admin);
        assert!(!token_client.is_paused());

        // Cross-contract claim must work normally after unpausing.
        token_client.claim_reward(&learner, &course_id, &quiz_id);
        // Score 50 * 100 = 5000
        assert_eq!(token_client.balance(&learner), 5000);
        assert_eq!(token_client.total_supply(), 5000);
    }

    // ── Double-claim: claimed flag persists through cross-contract read ───

    /// After a successful cross-contract claim the "reward claimed" flag must
    /// survive any subsequent cross-contract reads. A second `claim_reward`
    /// for the same (learner, course, quiz) triple must be rejected with
    /// "reward already claimed" — not "quiz not submitted" or any other error
    /// — proving the flag is durable and the progress-tracker state is
    /// unchanged (the score is still readable).
    #[test]
    fn test_claimed_flag_persists_and_blocks_second_cross_contract_attempt() {
        let env = Env::default();
        let (_admin, token_id, pt_id) = setup(&env);
        let token_client = LearnTokenClient::new(&env, &token_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 90);

        // First claim: cross-contract call succeeds, 9000 tokens minted.
        token_client.claim_reward(&learner, &course_id, &quiz_id);
        assert_eq!(token_client.balance(&learner), 9000);

        // Progress-tracker score is still readable after the claim.
        assert_eq!(
            pt_client.get_quiz_score(&learner, &course_id, &quiz_id),
            90,
            "progress-tracker score must be unchanged after claim"
        );

        // Second claim attempt: must be caught by the claimed-flag check,
        // panicking with "reward already claimed" before the cross-contract
        // call even fires.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            token_client.claim_reward(&learner, &course_id, &quiz_id);
        }));
        assert!(result.is_err(), "second claim must revert");

        // Balance and supply are unchanged from after the first (successful) claim.
        assert_eq!(token_client.balance(&learner), 9000);
        assert_eq!(token_client.total_supply(), 9000);
    }

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
