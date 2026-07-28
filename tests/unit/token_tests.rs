//! Unit tests for the learn-token contract.

use learn_token::{LearnToken, LearnTokenClient};
use progress_tracker::{ProgressTracker, ProgressTrackerClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String as SorobanString, Symbol, Vec};

#[cfg(test)]
mod token_unit_tests {
    use super::*;

    fn setup_token(env: &Env) -> (Address, Address, Address) {
        let admin = Address::generate(env);

        // Register progress-tracker
        let pt_contract_id = env.register_contract(None, ProgressTracker);
        let pt_client = ProgressTrackerClient::new(env, &pt_contract_id);
        pt_client.initialize(&admin);

        // Register learn-token with progress-tracker address
        let contract_id = env.register_contract(None, LearnToken);
        let client = LearnTokenClient::new(env, &contract_id);
        client.initialize(
            &admin,
            &SorobanString::from_str(env, "CLearn"),
            &SorobanString::from_str(env, "CLRN"),
            &7,
            &pt_contract_id,
        );

        (admin, contract_id, pt_contract_id)
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

    #[test]
    fn test_token_metadata_after_init() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        assert_eq!(client.name(), SorobanString::from_str(&env, "CLearn"));
        assert_eq!(client.symbol(), SorobanString::from_str(&env, "CLRN"));
        assert_eq!(client.decimals(), 7);
        assert_eq!(client.total_supply(), 0);
        assert_eq!(client.admin(), admin);
    }

    #[test]
    fn test_mint_increases_balance_and_supply() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let recipient = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&recipient, &1000);
        assert_eq!(client.balance(&recipient), 1000);
        assert_eq!(client.total_supply(), 1000);
    }

    #[test]
    fn test_transfer_moves_tokens() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&alice, &500);
        client.transfer(&alice, &bob, &200);

        assert_eq!(client.balance(&alice), 300);
        assert_eq!(client.balance(&bob), 200);
    }

    #[test]
    #[should_panic(expected = "insufficient balance")]
    fn test_transfer_insufficient_balance() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&alice, &100);
        client.transfer(&alice, &bob, &200);
    }

    #[test]
    fn test_claim_reward_proportional_minting() {
        let env = Env::default();
        let (_admin, contract_id, pt_contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 80);

        let reward = client.claim_reward(&learner, &course_id, &quiz_id);

        // 80 * 100 (BASE_REWARD_PER_POINT) = 8000
        assert_eq!(reward, 8000);
        assert_eq!(client.balance(&learner), 8000);
        assert_eq!(client.total_supply(), 8000);
    }

    #[test]
    #[should_panic(expected = "reward already claimed")]
    fn test_claim_reward_double_claim() {
        let env = Env::default();
        let (_admin, contract_id, pt_contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 80);

        client.claim_reward(&learner, &course_id, &quiz_id);
        client.claim_reward(&learner, &course_id, &quiz_id);
    }

    #[test]
    #[should_panic(expected = "score exceeds maximum")]
    fn test_claim_reward_rejects_high_score() {
        let env = Env::default();
        let (_admin, contract_id, pt_contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 101);
    }

    #[test]
    fn test_transfer_from_with_allowance() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&owner, &1000);
        client.approve(&owner, &spender, &500, &999999);

        client.transfer_from(&spender, &owner, &recipient, &300);

        assert_eq!(client.balance(&owner), 700);
        assert_eq!(client.balance(&recipient), 300);
        assert_eq!(client.allowance(&owner, &spender), 200);
    }
}
