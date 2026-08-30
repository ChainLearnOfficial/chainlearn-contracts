//! Unit tests for the learn-token contract.

use learn_token::{LearnToken, LearnTokenClient};
use progress_tracker::{ProgressTracker, ProgressTrackerClient};
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    Address, Env, IntoVal, String as SorobanString, Symbol, Vec,
};

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
            &1_000_000_000_000_000,
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
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let recipient = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &recipient, &1000);
        assert_eq!(client.balance(&recipient), 1000);
        assert_eq!(client.total_supply(), 1000);
    }

    #[test]
    fn test_transfer_moves_tokens() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &alice, &500);
        client.transfer(&alice, &bob, &200);

        assert_eq!(client.balance(&alice), 300);
        assert_eq!(client.balance(&bob), 200);
    }

    #[test]
    #[should_panic(expected = "insufficient balance")]
    fn test_transfer_insufficient_balance() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &alice, &100);
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

        client.claim_reward(&learner, &course_id, &quiz_id);

        // 80 * 100 (BASE_REWARD_PER_POINT) = 8000
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
        let (_admin, _contract_id, pt_contract_id) = setup_token(&env);
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
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &owner, &1000);
        client.approve(&owner, &spender, &500, &999999);

        client.transfer_from(&spender, &owner, &recipient, &300);

        assert_eq!(client.balance(&owner), 700);
        assert_eq!(client.balance(&recipient), 300);
        assert_eq!(client.allowance(&owner, &spender), 200);
    }

    #[test]
    #[should_panic(expected = "insufficient allowance")]
    fn test_transfer_from_insufficient_allowance() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &owner, &1000);
        client.approve(&owner, &spender, &200, &999999);
        client.transfer_from(&spender, &owner, &recipient, &500);
    }

    #[test]
    #[should_panic(expected = "insufficient allowance")]
    fn test_transfer_from_expired_allowance() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &owner, &1000);
        client.approve(&owner, &spender, &500, &10);

        env.ledger().with_mut(|l| {
            l.sequence_number = 20;
        });

        assert_eq!(client.allowance(&owner, &spender), 0);
        client.transfer_from(&spender, &owner, &recipient, &100);
    }

    #[test]
    fn test_approve_zero_allowance_revokes() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        client.approve(&owner, &spender, &500, &999999);
        assert_eq!(client.allowance(&owner, &spender), 500);

        client.approve(&owner, &spender, &0, &999999);
        assert_eq!(client.allowance(&owner, &spender), 0);
    }

    #[test]
    #[should_panic]
    fn test_mint_without_admin_auth_fails() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let recipient = Address::generate(&env);
        // We do NOT mock auths, so mint should fail auth requirement
        client.mint(&admin, &recipient, &1000);
    }

    #[test]
    fn test_mint_up_to_and_exactly_at_cap() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let pt_contract_id = env.register_contract(None, ProgressTracker);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);
        pt_client.initialize(&admin);

        let contract_id = env.register_contract(None, LearnToken);
        let client = LearnTokenClient::new(&env, &contract_id);
        client.initialize(
            &admin,
            &SorobanString::from_str(&env, "CLearn"),
            &SorobanString::from_str(&env, "CLRN"),
            &7,
            &pt_contract_id,
            &5000,
        );

        let user = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &user, &2000);
        assert_eq!(client.total_supply(), 2000);

        client.mint(&admin, &user, &3000);
        assert_eq!(client.total_supply(), 5000);
        assert_eq!(client.balance(&user), 5000);
    }

    #[test]
    #[should_panic(expected = "maximum supply cap exceeded")]
    fn test_mint_beyond_max_supply_panics() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let pt_contract_id = env.register_contract(None, ProgressTracker);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);
        pt_client.initialize(&admin);

        let contract_id = env.register_contract(None, LearnToken);
        let client = LearnTokenClient::new(&env, &contract_id);
        client.initialize(
            &admin,
            &SorobanString::from_str(&env, "CLearn"),
            &SorobanString::from_str(&env, "CLRN"),
            &7,
            &pt_contract_id,
            &5000,
        );

        let user = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &user, &3000);
        client.mint(&admin, &user, &2001);
    }

    #[test]
    fn test_admin_configures_max_supply_cap() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let pt_contract_id = env.register_contract(None, ProgressTracker);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);
        pt_client.initialize(&admin);

        let contract_id = env.register_contract(None, LearnToken);
        let client = LearnTokenClient::new(&env, &contract_id);
        client.initialize(
            &admin,
            &SorobanString::from_str(&env, "CLearn"),
            &SorobanString::from_str(&env, "CLRN"),
            &7,
            &pt_contract_id,
            &5000,
        );

        env.mock_all_auths();

        assert_eq!(client.max_supply(), 5000);

        client.mint(&admin, &Address::generate(&env), &3000);

        client.set_max_supply(&10000);
        assert_eq!(client.max_supply(), 10000);
    }

    #[test]
    #[should_panic(expected = "new cap cannot be less than current total supply")]
    fn test_admin_cannot_set_max_supply_below_current_supply() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let pt_contract_id = env.register_contract(None, ProgressTracker);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);
        pt_client.initialize(&admin);

        let contract_id = env.register_contract(None, LearnToken);
        let client = LearnTokenClient::new(&env, &contract_id);
        client.initialize(
            &admin,
            &SorobanString::from_str(&env, "CLearn"),
            &SorobanString::from_str(&env, "CLRN"),
            &7,
            &pt_contract_id,
            &5000,
        );

        env.mock_all_auths();

        client.mint(&admin, &Address::generate(&env), &3000);
        client.set_max_supply(&2000);
    }

    #[test]
    fn test_set_max_supply_emits_event() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let pt_contract_id = env.register_contract(None, ProgressTracker);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);
        pt_client.initialize(&admin);

        let contract_id = env.register_contract(None, LearnToken);
        let client = LearnTokenClient::new(&env, &contract_id);
        client.initialize(
            &admin,
            &SorobanString::from_str(&env, "CLearn"),
            &SorobanString::from_str(&env, "CLRN"),
            &7,
            &pt_contract_id,
            &5000,
        );

        env.mock_all_auths();

        client.set_max_supply(&8000);
        assert_eq!(client.max_supply(), 8000);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id.clone(),
                    (Symbol::new(&env, "max_supply_updated"),).into_val(&env),
                    (5000i128, 8000i128).into_val(&env),
                )
            ]
        );
    }

    #[test]
    #[should_panic(expected = "max supply increase exceeds governance limit")]
    fn test_set_max_supply_rejects_exceeding_2x_increase() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let pt_contract_id = env.register_contract(None, ProgressTracker);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);
        pt_client.initialize(&admin);

        let contract_id = env.register_contract(None, LearnToken);
        let client = LearnTokenClient::new(&env, &contract_id);
        client.initialize(
            &admin,
            &SorobanString::from_str(&env, "CLearn"),
            &SorobanString::from_str(&env, "CLRN"),
            &7,
            &pt_contract_id,
            &5000,
        );

        env.mock_all_auths();

        // 5000 -> 15000 is 3x increase, exceeding the 2x limit (max 10000)
        client.set_max_supply(&15000);
    }

    #[test]
    fn test_set_max_supply_allows_reduction() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let pt_contract_id = env.register_contract(None, ProgressTracker);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);
        pt_client.initialize(&admin);

        let contract_id = env.register_contract(None, LearnToken);
        let client = LearnTokenClient::new(&env, &contract_id);
        client.initialize(
            &admin,
            &SorobanString::from_str(&env, "CLearn"),
            &SorobanString::from_str(&env, "CLRN"),
            &7,
            &pt_contract_id,
            &5000,
        );

        env.mock_all_auths();

        client.mint(&admin, &Address::generate(&env), &1000);
        assert_eq!(client.total_supply(), 1000);

        // Reducing from 5000 to 3000 (above current supply 1000) is allowed
        client.set_max_supply(&3000);
        assert_eq!(client.max_supply(), 3000);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id.clone(),
                    (Symbol::new(&env, "max_supply_updated"),).into_val(&env),
                    (5000i128, 3000i128).into_val(&env),
                )
            ]
        );
    }

    #[test]
    #[should_panic(expected = "cannot mint to zero address")]
    fn test_mint_to_zero_address_panics() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let zero_address = Address::from_string(&SorobanString::from_str(
            &env,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        ));
        client.mint(&admin, &zero_address, &1000);
    }

    #[test]
    fn test_transfer_from_emits_transfer_from_event() {
        use soroban_sdk::testutils::Events;

        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &owner, &1000);
        client.approve(&owner, &spender, &500, &999999);
        client.transfer_from(&spender, &owner, &recipient, &300);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "transfer_from"), owner.clone(), recipient.clone())
                        .into_val(&env),
                    (spender, 300i128).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_transfer_from_event_indexes_from_and_to_in_topics() {
        use soroban_sdk::testutils::Events;

        // #200: from/to must be queryable via topic filters, not just present
        // somewhere in the data payload.
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &owner, &1000);
        client.approve(&owner, &spender, &500, &999999);
        client.transfer_from(&spender, &owner, &recipient, &300);

        let all = env.events().all();
        let (_, topics, _) = all.last().expect("no events emitted");
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        assert_eq!(topics.len(), 3);
        let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
        let from_topic: Address = topics.get(1).unwrap().into_val(&env);
        let to_topic: Address = topics.get(2).unwrap().into_val(&env);
        assert_eq!(event_name, Symbol::new(&env, "transfer_from"));
        assert_eq!(from_topic, owner);
        assert_eq!(to_topic, recipient);
    }

    #[test]
    fn test_reward_claimed_event_indexes_learner_and_course() {
        let env = Env::default();
        let (_admin, contract_id, pt_contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        env.mock_all_auths();

        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 80);
        client.claim_reward(&learner, &course_id, &quiz_id);

        let all = env.events().all();
        let (_, topics, _) = all.last().expect("no events emitted");
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        assert_eq!(topics.len(), 3);
        let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
        let learner_topic: Address = topics.get(1).unwrap().into_val(&env);
        let course_topic: Symbol = topics.get(2).unwrap().into_val(&env);
        assert_eq!(event_name, Symbol::new(&env, "reward"));
        assert_eq!(learner_topic, learner);
        assert_eq!(course_topic, course_id);
    }

    #[test]
    fn test_cleanup_expired_allowances_removes_only_expired() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender_expiring = Address::generate(&env);
        let spender_valid = Address::generate(&env);
        env.mock_all_auths();

        client.approve(&owner, &spender_expiring, &100, &10);
        client.approve(&owner, &spender_valid, &200, &999999);

        assert_eq!(client.allowance_spender_count(&owner), 2);

        env.ledger().with_mut(|l| {
            l.sequence_number = 20;
        });

        let removed = client.cleanup_expired_allowances(&owner);

        // Only the expired allowance is removed; the valid one is preserved
        // and storage (the spender registry) shrinks accordingly.
        assert_eq!(removed, 1);
        assert_eq!(client.allowance_spender_count(&owner), 1);
        assert_eq!(client.allowance(&owner, &spender_valid), 200);
        assert_eq!(client.allowance(&owner, &spender_expiring), 0);

        // No side effects: cleaning up again finds nothing left to remove.
        let removed_again = client.cleanup_expired_allowances(&owner);
        assert_eq!(removed_again, 0);
        assert_eq!(client.allowance_spender_count(&owner), 1);
    }

    #[test]
    fn test_initialize_twice_returns_already_initialized_error() {
        let env = Env::default();
        let (admin, contract_id, pt_contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let result = client.try_initialize(
            &admin,
            &SorobanString::from_str(&env, "CLearn"),
            &SorobanString::from_str(&env, "CLRN"),
            &7,
            &pt_contract_id,
            &1_000_000_000_000_000,
        );

        assert!(result.is_err(), "second initialize call should fail");
        let contract_err = result
            .err()
            .expect("expected an error")
            .expect("expected a typed contract error, not a host trap");
        assert_eq!(contract_err, learn_token::ContractError::AlreadyInitialized);
    }
}
