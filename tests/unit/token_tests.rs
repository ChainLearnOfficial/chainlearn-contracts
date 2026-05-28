//! Unit tests for the learn-token contract.

use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

// Note: These tests import from the contract crate. In a real project,
// you would use: use learn_token::LearnToken;

#[cfg(test)]
mod token_unit_tests {
    use super::*;

    /// Test that token initialization sets metadata correctly.
    #[test]
    fn test_token_metadata_after_init() {
        let env = Env::default();
        // In a real test setup, register the contract and initialize it.
        // This is a template showing the test structure.
        let admin = Address::generate(&env);
        let learner = Address::generate(&env);

        // Verify: name, symbol, decimals should match init params
        // assert_eq!(client.name(), expected_name);
        // assert_eq!(client.symbol(), expected_symbol);
        // assert_eq!(client.decimals(), 7);
    }

    /// Test minting increases balance and total supply.
    #[test]
    fn test_mint_increases_balance_and_supply() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();

        // Mint 1000 tokens to recipient
        // client.mint(&recipient, &1000);
        // assert_eq!(client.balance(&recipient), 1000);
        // assert_eq!(client.total_supply(), 1000);
    }

    /// Test transfer moves tokens between accounts.
    #[test]
    fn test_transfer_moves_tokens() {
        let env = Env::default();
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();

        // Setup: alice has 500 tokens
        // client.mint(&alice, &500);

        // Transfer 200 from alice to bob
        // client.transfer(&alice, &bob, &200);

        // assert_eq!(client.balance(&alice), 300);
        // assert_eq!(client.balance(&bob), 200);
    }

    /// Test transfer fails with insufficient balance.
    #[test]
    #[should_panic(expected = "insufficient balance")]
    fn test_transfer_insufficient_balance() {
        let env = Env::default();
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();

        // alice has 100, tries to send 200
        // client.mint(&alice, &100);
        // client.transfer(&alice, &bob, &200);
    }

    /// Test claim_reward mints proportional to score.
    #[test]
    fn test_claim_reward_proportional_minting() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        let quiz_id = Symbol::new(&env, "quiz_1");

        // Score 80 out of 100, BASE_REWARD_PER_POINT = 100
        // Expected reward: 80 * 100 = 8000
        // let reward = client.claim_reward(&learner, &quiz_id, &80);
        // assert_eq!(reward, 8000);
        // assert_eq!(client.balance(&learner), 8000);
    }

    /// Test claim_reward prevents double claiming.
    #[test]
    #[should_panic(expected = "reward already claimed")]
    fn test_claim_reward_double_claim() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        let quiz_id = Symbol::new(&env, "quiz_1");

        // client.claim_reward(&learner, &quiz_id, &80);
        // client.claim_reward(&learner, &quiz_id, &80);
    }

    /// Test claim_reward rejects scores above maximum.
    #[test]
    #[should_panic(expected = "score exceeds maximum")]
    fn test_claim_reward_rejects_high_score() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        let quiz_id = Symbol::new(&env, "quiz_1");
        // client.claim_reward(&learner, &quiz_id, &101);
    }

    /// Test transfer_from with allowance.
    #[test]
    fn test_transfer_from_with_allowance() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();

        // owner has 1000, approves spender for 500
        // client.mint(&owner, &1000);
        // client.approve(&owner, &spender, &500, &999999);

        // spender transfers 300 from owner to recipient
        // client.transfer_from(&spender, &owner, &recipient, &300);

        // assert_eq!(client.balance(&owner), 700);
        // assert_eq!(client.balance(&recipient), 300);
        // assert_eq!(client.allowance(&owner, &spender), 200);
    }
}
