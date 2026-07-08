//! Unit tests for the learn-token contract.

use learn_token::{LearnToken, LearnTokenClient};
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

#[cfg(test)]
mod token_unit_tests {
    use super::*;

    fn setup_token(env: &Env) -> (Address, Address) {
        let admin = Address::generate(env);
        let contract_id = env.register(LearnToken, ());
        let client = LearnTokenClient::new(env, &contract_id);
        client.initialize(
            &admin,
            &Symbol::new(env, "CLearn"),
            &Symbol::new(env, "CLRN"),
            &7,
        );
        (admin, contract_id)
    }

    #[test]
    fn test_token_metadata_after_init() {
        let env = Env::default();
        let (admin, contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        assert_eq!(client.name(), Symbol::new(&env, "CLearn"));
        assert_eq!(client.symbol(), Symbol::new(&env, "CLRN"));
        assert_eq!(client.decimals(), 7);
        assert_eq!(client.total_supply(), 0);
        assert_eq!(client.admin(), admin);
    }

    #[test]
    fn test_mint_increases_balance_and_supply() {
        let env = Env::default();
        let (_admin, contract_id) = setup_token(&env);
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
        let (_admin, contract_id) = setup_token(&env);
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
        let (_admin, contract_id) = setup_token(&env);
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
        let (_admin, contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let quiz_id = Symbol::new(&env, "quiz_1");
        let reward = client.claim_reward(&learner, &quiz_id, &80);

        // 80 * 100 (BASE_REWARD_PER_POINT) = 8000
        assert_eq!(reward, 8000);
        assert_eq!(client.balance(&learner), 8000);
        assert_eq!(client.total_supply(), 8000);
    }

    #[test]
    #[should_panic(expected = "reward already claimed")]
    fn test_claim_reward_double_claim() {
        let env = Env::default();
        let (_admin, contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let quiz_id = Symbol::new(&env, "quiz_1");
        client.claim_reward(&learner, &quiz_id, &80);
        client.claim_reward(&learner, &quiz_id, &80);
    }

    #[test]
    #[should_panic(expected = "score exceeds maximum")]
    fn test_claim_reward_rejects_high_score() {
        let env = Env::default();
        let (_admin, contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let quiz_id = Symbol::new(&env, "quiz_1");
        client.claim_reward(&learner, &quiz_id, &101);
    }

    #[test]
    fn test_transfer_from_with_allowance() {
        let env = Env::default();
        let (_admin, contract_id) = setup_token(&env);
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
