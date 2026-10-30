//! Unit tests for the learn-token contract.

use learn_token::{AdminInfo, AdminRole, ClaimRecord, ContractError, LearnToken, LearnTokenClient};
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
    #[should_panic(expected = "Error(Contract, #5)")]
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
    #[should_panic(expected = "Error(Contract, #12)")]
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
    #[should_panic(expected = "Error(Contract, #6)")]
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
    #[should_panic(expected = "Error(Contract, #6)")]
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
        let err = client.try_mint(&admin, &user, &2001)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::MaxSupplyExceeded);
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
    #[should_panic]
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
    #[should_panic]
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
    fn test_mint_to_zero_address_panics() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let zero_address = Address::from_string(&SorobanString::from_str(
            &env,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        ));
        let err = client.try_mint(&admin, &zero_address, &1000)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::ZeroAddress);
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
                    (
                        Symbol::new(&env, "transfer_from"),
                        owner.clone(),
                        recipient.clone()
                    )
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

        let expiring_ledger = env.ledger().sequence() + 5;
        let valid_ledger = env.ledger().sequence() + 999999;
        client.approve(&owner, &spender_expiring, &100, &expiring_ledger);
        client.approve(&owner, &spender_valid, &200, &valid_ledger);

        assert_eq!(client.allowance_spender_count(&owner), 2);

        env.ledger().with_mut(|l| {
            l.sequence_number = expiring_ledger + 1;
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

    // ── Security: overflow/underflow protection (#291) ─────────────────────
    //
    // `overflow-checks = true` is set for both the dev and release profiles
    // (see Cargo.toml), so i128 arithmetic traps instead of silently
    // wrapping. These tests verify that trap actually fires on the
    // reachable overflow/underflow paths, that the domain-specific guards
    // (e.g. "insufficient balance") catch underflow before it can happen,
    // and that a reverted call leaves balances and total supply untouched.

    fn setup_token_with_max_supply(env: &Env, max_supply: i128) -> (Address, Address, Address) {
        let admin = Address::generate(env);

        let pt_contract_id = env.register_contract(None, ProgressTracker);
        let pt_client = ProgressTrackerClient::new(env, &pt_contract_id);
        pt_client.initialize(&admin);

        let contract_id = env.register_contract(None, LearnToken);
        let client = LearnTokenClient::new(env, &contract_id);
        client.initialize(
            &admin,
            &SorobanString::from_str(env, "CLearn"),
            &SorobanString::from_str(env, "CLRN"),
            &7,
            &pt_contract_id,
            &max_supply,
        );

        (admin, contract_id, pt_contract_id)
    }

    #[test]
    fn test_security_mint_supply_overflow_reverts() {
        let env = Env::default();
        let (admin, contract_id, _pt_contract_id) = setup_token_with_max_supply(&env, i128::MAX);
        let client = LearnTokenClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &user, &i128::MAX);
        assert_eq!(client.total_supply(), i128::MAX);

        let err = client.try_mint(&admin, &user, &1)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::MaxSupplyExceeded);
    }

    #[test]
    fn test_security_mint_supply_overflow_does_not_corrupt_state() {
        let env = Env::default();
        let (admin, contract_id, _pt_contract_id) = setup_token_with_max_supply(&env, i128::MAX);
        let client = LearnTokenClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &user, &i128::MAX);

        let supply_before = client.total_supply();
        let balance_before = client.balance(&user);
        assert_eq!(supply_before, i128::MAX);
        assert_eq!(balance_before, i128::MAX);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.mint(&admin, &user, &1);
        }));
        assert!(result.is_err(), "overflowing mint should revert");

        // The trap must unwind before any storage write lands — supply and
        // balance are exactly as they were before the reverted call.
        assert_eq!(client.total_supply(), supply_before);
        assert_eq!(client.balance(&user), balance_before);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #5)")]
    fn test_security_transfer_balance_underflow_reverts() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();

        // Bob holds nothing; subtracting anything from a zero balance would
        // underflow an unsigned/unchecked path. The explicit balance check
        // must catch this with a clear message before any subtraction runs.
        client.transfer(&bob, &alice, &1);
    }

    #[test]
    fn test_security_transfer_underflow_does_not_corrupt_state() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &alice, &500);

        let alice_before = client.balance(&alice);
        let bob_before = client.balance(&bob);
        let supply_before = client.total_supply();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.transfer(&bob, &alice, &1);
        }));
        assert!(result.is_err(), "underflowing transfer should revert");

        assert_eq!(client.balance(&alice), alice_before);
        assert_eq!(client.balance(&bob), bob_before);
        assert_eq!(client.total_supply(), supply_before);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #5)")]
    fn test_security_burn_balance_underflow_reverts() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        // Same underflow shape as transfer, on the burn path: a zero balance
        // must reject a burn instead of wrapping to a huge positive balance.
        client.burn(&user, &1);
    }

    #[test]
    fn test_security_burn_underflow_does_not_corrupt_state() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &user, &300);

        let balance_before = client.balance(&user);
        let supply_before = client.total_supply();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.burn(&user, &1_000);
        }));
        assert!(result.is_err(), "underflowing burn should revert");

        assert_eq!(client.balance(&user), balance_before);
        assert_eq!(client.total_supply(), supply_before);
    }

    #[test]
    fn test_security_batch_claim_reward_supply_overflow_skips_without_panicking() {
        // `batch_claim_reward` is documented to skip over-cap claims rather
        // than aborting the whole batch (partial failures don't block
        // successful claims). With supply already at `i128::MAX`, the cap
        // check's `current_supply + reward_amount` would overflow before
        // ever comparing against `max_supply` unless it uses `checked_add`
        // — an unchecked `+` there turns a should-be-skipped claim into a
        // raw arithmetic-overflow panic, aborting the whole batch and
        // breaking that "partial failures don't block" guarantee.
        let env = Env::default();
        let (admin, contract_id, pt_contract_id) = setup_token_with_max_supply(&env, i128::MAX);
        let client = LearnTokenClient::new(&env, &contract_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);

        let filler = Address::generate(&env);
        let learner = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &filler, &i128::MAX);
        assert_eq!(client.total_supply(), i128::MAX);

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 80);

        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(quiz_id);

        let successful = client.batch_claim_reward(&learner, &course_id, &quiz_ids);

        // No panic, no revert: the overflowing claim is simply skipped, and
        // state is left exactly as it was before the call.
        assert_eq!(successful.len(), 0);
        assert_eq!(client.balance(&learner), 0);
        assert_eq!(client.total_supply(), i128::MAX);
    }

    #[test]
    fn test_governance_proposal_lifecycle() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let voter = Address::generate(&env);
        client.mint(&admin, &voter, &1000);

        env.ledger().with_mut(|l| l.sequence_number = 99);
        let snapshot_ledger = env.ledger().sequence();
        client.snapshot(&snapshot_ledger);
        client.record_balance_snapshot(&voter, &snapshot_ledger);
        env.ledger().with_mut(|l| l.sequence_number = 100);
        let proposal_id = client.create_proposal(
            &SorobanString::from_str(&env, "Increase rewards"),
            &2,
            &0,
            &1000,
            &snapshot_ledger,
        );

        let proposal = client.get_proposal(&proposal_id).unwrap();
        assert_eq!(proposal.choices, 2);
        assert!(!proposal.executed);

        // Voting power equals the voter's balance at the snapshot ledger.
        assert_eq!(client.balance(&voter), 1000);

        client.vote(&voter, &proposal_id, &0);

        let after_vote = client.get_proposal(&proposal_id).unwrap();
        assert_eq!(after_vote.vote_totals.get(0).unwrap(), 1000);

        env.ledger().with_mut(|l| {
            l.timestamp = 2000;
        });

        let winning_choice = client.execute_proposal(&proposal_id);
        assert_eq!(winning_choice, 0);

        let executed = client.get_proposal(&proposal_id).unwrap();
        assert!(executed.executed);
    }

    #[test]
    fn test_vote_rejects_missing_snapshot_instead_of_using_current_balance() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let voter = Address::generate(&env);
        client.mint(&admin, &voter, &1_000);
        env.ledger().with_mut(|l| l.sequence_number = 100);
        let proposal_id = client.create_proposal(
            &SorobanString::from_str(&env, "Missing snapshot"),
            &2,
            &0,
            &1_000,
            &99,
        );
        env.ledger().with_mut(|l| l.timestamp = 500);

        let err = client.try_vote(&voter, &proposal_id, &0)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::NotAuthorized);
    }

    #[test]
    fn test_create_proposal_rejects_current_or_future_snapshot_ledger() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.sequence_number = 100);

        let current_err = client.try_create_proposal(
            &SorobanString::from_str(&env, "Current snapshot"),
            &2,
            &0,
            &1_000,
            &100,
        ).err().expect("should fail").expect("expected typed error");
        assert_eq!(current_err, ContractError::ExpirationInPast);

        let future_err = client.try_create_proposal(
            &SorobanString::from_str(&env, "Future snapshot"),
            &2,
            &0,
            &1_000,
            &101,
        ).err().expect("should fail").expect("expected typed error");
        assert_eq!(future_err, ContractError::ExpirationInPast);
        assert_eq!(client.proposal_count(), 0);
    }

    #[test]
    fn test_execute_proposal_rejects_zero_votes() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        env.ledger().with_mut(|l| {
            l.sequence_number = 100;
            l.timestamp = 0;
        });
        let proposal_id = client.create_proposal(
            &SorobanString::from_str(&env, "No votes"),
            &2,
            &0,
            &1_000,
            &99,
        );

        env.ledger().with_mut(|l| l.timestamp = 2_000);
        let err = client.try_execute_proposal(&proposal_id)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::NotAuthorized);

        let proposal = client.get_proposal(&proposal_id).unwrap();
        assert!(!proposal.executed);
        assert_eq!(proposal.winning_choice, u32::MAX);
        assert_eq!(proposal.vote_totals.get(0).unwrap(), 0);
        assert_eq!(proposal.vote_totals.get(1).unwrap(), 0);
    }

    // Regression guard (#581): these three pause-path tests must stay at
    // module scope so the Rust test harness registers each as an independent
    // test. Keep them out of any other test function's body.
    #[test]
    fn test_create_proposal_fails_while_paused() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();
        client.pause(&admin);
        let err = client.try_create_proposal(
            &SorobanString::from_str(&env, "Paused proposal"),
            &2,
            &0,
            &100,
            &env.ledger().sequence(),
        ).err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::ContractPaused);
    }

    #[test]
    fn test_vote_fails_while_paused() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let voter = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &voter, &100);
        let snap_seq = 10;
        env.ledger().with_mut(|li| li.sequence_number = snap_seq);
        client.snapshot(&snap_seq);
        client.record_balance_snapshot(&voter, &snap_seq);
        env.ledger().with_mut(|li| li.sequence_number = snap_seq + 1);
        let proposal_id = client.create_proposal(
            &SorobanString::from_str(&env, "Paused vote"),
            &2,
            &0,
            &100,
            &snap_seq,
        );
        client.pause(&admin);
        let err = client.try_vote(&voter, &proposal_id, &0)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::ContractPaused);
    }

    #[test]
    fn test_execute_proposal_fails_while_paused() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();
        let snap_seq = 10;
        env.ledger().with_mut(|li| li.sequence_number = snap_seq);
        client.snapshot(&snap_seq);
        env.ledger().with_mut(|li| li.sequence_number = snap_seq + 1);
        let proposal_id = client.create_proposal(
            &SorobanString::from_str(&env, "Paused execution"),
            &2,
            &0,
            &100,
            &snap_seq,
        );
        env.ledger().with_mut(|ledger| ledger.timestamp = 100);
        client.pause(&admin);
        let err = client.try_execute_proposal(&proposal_id)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::ContractPaused);
    }

    #[test]
    fn test_whitelist_restriction_waits_before_activation() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &alice, &100);
        client.mint(&admin, &bob, &100);

        client.set_transfer_restriction(&learn_token::TransferRestriction::WhitelistOnly);
        assert_eq!(
            client.get_transfer_restriction(),
            learn_token::TransferRestriction::None
        );
        assert!(client.pending_transfer_restriction().is_some());
        assert!(client.try_accept_transfer_restriction().is_err());

        // Existing holders can be whitelisted throughout the grace period.
        client.add_to_whitelist(&alice);
        client.add_to_whitelist(&bob);
        env.ledger().with_mut(|ledger| ledger.timestamp += 172_800);
        client.accept_transfer_restriction();

        assert_eq!(
            client.get_transfer_restriction(),
            learn_token::TransferRestriction::WhitelistOnly
        );
        assert!(client.pending_transfer_restriction().is_none());
    }

    #[test]
    fn test_admin_transfer_delay_rejects_zero() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();
        let err = client.try_set_admin_transfer_delay(&0)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::NegativeAmount);
    }

    #[test]
    fn test_admin_transfer_delay_accepts_one_hour_minimum() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();
        client.set_admin_transfer_delay(&3_600);
        assert_eq!(client.admin_transfer_delay(), 3_600);
    }

    #[test]
    fn test_admin_transfer_delay_rejects_below_one_hour() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();
        let err = client.try_set_admin_transfer_delay(&3_599)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::NegativeAmount);
    }

    #[test]
    fn test_vesting_schedule_cliff_linear_vesting_and_claiming() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let beneficiary = Address::generate(&env);

        env.mock_all_auths();

        let total_amount: i128 = 10_000;
        let cliff_timestamp: u64 = 100;
        let duration_seconds: u64 = 1_000;

        env.ledger().with_mut(|l| {
            l.timestamp = 0;
        });

        client.create_vesting(
            &beneficiary,
            &total_amount,
            &cliff_timestamp,
            &duration_seconds,
        );

        let schedule = client
            .get_vesting_schedule(&beneficiary)
            .expect("schedule should exist");
        assert_eq!(schedule.total_amount, 10_000);
        assert_eq!(schedule.cliff_timestamp, 100);
        assert_eq!(schedule.duration_seconds, 1_000);
        assert!(!schedule.exhausted);
        assert_eq!(client.get_vesting_claimed(&beneficiary), 0);

        // Linear vesting halfway through cliff + 500s (50% vested)
        env.ledger().with_mut(|l| {
            l.timestamp = cliff_timestamp + 500;
        });

        client.claim_vested(&beneficiary);
        assert_eq!(client.balance(&beneficiary), 5_000);
        assert_eq!(client.get_vesting_claimed(&beneficiary), 5_000);
        assert_eq!(client.total_supply(), 5_000);

        let mid_schedule = client.get_vesting_schedule(&beneficiary).unwrap();
        assert!(!mid_schedule.exhausted);

        // Complete linear vesting cliff + 1000s (100% vested)
        env.ledger().with_mut(|l| {
            l.timestamp = cliff_timestamp + 1_000;
        });

        client.claim_vested(&beneficiary);
        assert_eq!(client.balance(&beneficiary), 10_000);
        assert_eq!(client.get_vesting_claimed(&beneficiary), 10_000);
        assert_eq!(client.total_supply(), 10_000);

        let final_schedule = client.get_vesting_schedule(&beneficiary).unwrap();
        assert!(final_schedule.exhausted);
    }

    #[test]
    fn test_vesting_cliff_enforced() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let beneficiary = Address::generate(&env);
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = 0);
        client.create_vesting(&beneficiary, &10_000, &100, &1_000);
        env.ledger().with_mut(|l| l.timestamp = 50);
        let err = client.try_claim_vested(&beneficiary)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::ExpirationInPast);
    }

    #[test]
    fn test_vesting_claiming_after_exhausted_panics() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let beneficiary = Address::generate(&env);
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = 0);
        client.create_vesting(&beneficiary, &10_000, &100, &1_000);
        env.ledger().with_mut(|l| l.timestamp = 1100);
        client.claim_vested(&beneficiary);
        assert_eq!(client.balance(&beneficiary), 10_000);
        let err = client.try_claim_vested(&beneficiary)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::AlreadyInitialized);
    }

    #[test]
    fn test_vesting_rejects_amount_above_remaining_supply() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token_with_max_supply(&env, 1_000);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();
        let minted = Address::generate(&env);
        client.mint(&admin, &minted, &600);
        let beneficiary = Address::generate(&env);
        let err = client.try_create_vesting(&beneficiary, &401, &0, &1_000)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::MaxSupplyExceeded);
    }

    // ── Issue #241: admin transfer delay ─────────────────────────────────────

    #[test]
    fn test_transfer_admin_does_not_change_admin_immediately() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        env.mock_all_auths();

        client.transfer_admin(&new_admin);

        assert_eq!(
            client.admin(),
            admin,
            "admin must not change until accepted"
        );
        let pending = client.pending_admin().expect("pending transfer expected");
        assert_eq!(pending.new_admin, new_admin);
    }

    #[test]
    fn test_accept_admin_before_delay_elapses_fails() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        env.mock_all_auths();

        client.transfer_admin(&new_admin);

        // No time has passed at all yet.
        let result = client.try_accept_admin();
        assert!(result.is_err(), "accept before delay elapses must fail");
    }

    #[test]
    fn test_accept_admin_after_delay_elapses_succeeds() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        env.mock_all_auths();

        client.transfer_admin(&new_admin);
        let delay = client.admin_transfer_delay();

        env.ledger().with_mut(|l| {
            l.timestamp += delay;
        });

        client.accept_admin();

        assert_eq!(client.admin(), new_admin);
        assert_eq!(
            client.pending_admin(),
            None,
            "pending transfer must be cleared after acceptance"
        );
        assert_ne!(client.admin(), admin);
    }

    #[test]
    fn test_accept_admin_requires_new_admin_auth() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        env.mock_all_auths();
        client.transfer_admin(&new_admin);
        let delay = client.admin_transfer_delay();
        env.ledger().with_mut(|l| {
            l.timestamp += delay;
        });

        // Only new_admin's auth should satisfy accept_admin's require_auth;
        // asserting the exact auth tree catches a caller-address check being
        // silently dropped or swapped for a different address.
        client.accept_admin();
        assert_eq!(
            env.auths()[0].0,
            new_admin,
            "accept_admin must require new_admin's auth, not the caller's"
        );
    }

    #[test]
    fn test_cancel_admin_transfer_aborts_pending_transfer() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        env.mock_all_auths();

        client.transfer_admin(&new_admin);
        assert!(client.pending_admin().is_some());

        client.cancel_admin_transfer();

        assert_eq!(client.pending_admin(), None);
        assert_eq!(client.admin(), admin);

        // The delay elapsing afterward must not resurrect the cancelled transfer.
        let delay = client.admin_transfer_delay();
        env.ledger().with_mut(|l| {
            l.timestamp += delay;
        });
        let result = client.try_accept_admin();
        assert!(result.is_err(), "cancelled transfer must not be acceptable");
        assert_eq!(client.admin(), admin);
    }

    #[test]
    fn test_cancel_admin_transfer_requires_admin_auth() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        env.mock_all_auths();
        client.transfer_admin(&new_admin);

        client.cancel_admin_transfer();
        assert_eq!(
            env.auths()[0].0,
            admin,
            "cancel_admin_transfer must require the current admin's auth"
        );
    }

    #[test]
    fn test_admin_transfer_delay_is_configurable() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let default_delay = client.admin_transfer_delay();
        let custom_delay = default_delay * 2;
        client.set_admin_transfer_delay(&custom_delay);

        assert_eq!(client.admin_transfer_delay(), custom_delay);

        // A transfer initiated after the change is gated by the new delay.
        let new_admin = Address::generate(&env);
        client.transfer_admin(&new_admin);

        env.ledger().with_mut(|l| {
            l.timestamp += default_delay;
        });
        // Old (shorter) delay must not be enough anymore.
        assert!(client.try_accept_admin().is_err());

        env.ledger().with_mut(|l| {
            l.timestamp += custom_delay - default_delay;
        });
        client.accept_admin();
        assert_eq!(client.admin(), new_admin);
    }

    #[test]
    fn test_transfer_admin_emits_initiated_event() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        env.mock_all_auths();

        client.transfer_admin(&new_admin);

        let all = env.events().all();
        let (_, topics, _) = all.last().expect("no events emitted");
        let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
        let new_admin_topic: Address = topics.get(1).unwrap().into_val(&env);
        assert_eq!(event_name, Symbol::new(&env, "admin_transfer_initiated"));
        assert_eq!(new_admin_topic, new_admin);
    }

    #[test]
    fn test_accept_admin_emits_accepted_event() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        env.mock_all_auths();

        client.transfer_admin(&new_admin);
        let delay = client.admin_transfer_delay();
        env.ledger().with_mut(|l| {
            l.timestamp += delay;
        });
        client.accept_admin();

        let all = env.events().all();
        let (_, topics, _) = all.last().expect("no events emitted");
        let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
        let new_admin_topic: Address = topics.get(1).unwrap().into_val(&env);
        assert_eq!(event_name, Symbol::new(&env, "admin_transfer_accepted"));
        assert_eq!(new_admin_topic, new_admin);
    }

    #[test]
    fn test_cancel_admin_transfer_emits_cancelled_event() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        env.mock_all_auths();

        client.transfer_admin(&new_admin);
        client.cancel_admin_transfer();

        let all = env.events().all();
        let (_, topics, _) = all.last().expect("no events emitted");
        let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
        let new_admin_topic: Address = topics.get(1).unwrap().into_val(&env);
        assert_eq!(event_name, Symbol::new(&env, "admin_transfer_cancelled"));
        assert_eq!(new_admin_topic, new_admin);
    }

    #[test]
    fn test_re_initiating_transfer_overwrites_previous_pending_admin() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let first_candidate = Address::generate(&env);
        let second_candidate = Address::generate(&env);
        env.mock_all_auths();

        client.transfer_admin(&first_candidate);
        client.transfer_admin(&second_candidate);

        let pending = client.pending_admin().expect("pending transfer expected");
        assert_eq!(
            pending.new_admin, second_candidate,
            "second transfer_admin call must overwrite the first candidate"
        );

        let delay = client.admin_transfer_delay();
        env.ledger().with_mut(|l| {
            l.timestamp += delay;
        });

        client.accept_admin();
        assert_eq!(
            client.admin(),
            second_candidate,
            "only the surviving (second) candidate can complete the transfer"
        );
        assert_ne!(client.admin(), first_candidate);
    }

    // ── Issue #240: contract initialization verification ────────────────────

    #[test]
    fn test_is_initialized_false_before_initialize() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LearnToken);
        let client = LearnTokenClient::new(&env, &contract_id);

        assert!(!client.is_initialized());
    }

    #[test]
    fn test_is_initialized_true_after_initialize() {
        let env = Env::default();
        let (_admin, contract_id, _pt_contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        assert!(client.is_initialized());
    }

    #[test]
    fn test_is_initialized_does_not_mutate_state() {
        let env = Env::default();
        let (_admin, contract_id, _pt_contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let before = client.contract_metadata();
        let _ = client.is_initialized();
        let _ = client.is_initialized();
        let after = client.contract_metadata();

        assert_eq!(before, after, "is_initialized must be read-only");
    }

    // Issue #239 (storage-size tracking) is already implemented and covered
    // by an existing, more thorough test suite in
    // `contracts/learn-token/src/lib.rs` (see its "Issue #254: storage size
    // tracking" section) -- that implementation predates this branch on
    // `main`, so no duplicate tests are added here.

    // ── Issue #304: comprehensive admin transfer delay test ─────────────────

    #[test]
    fn test_admin_transfer_delay_full_flow() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        env.mock_all_auths();

        // 1. Initiate admin transfer
        client.transfer_admin(&new_admin);

        // 2. Verify pending admin is set
        let pending = client.pending_admin().expect("pending transfer expected");
        assert_eq!(pending.new_admin, new_admin);

        // 3. Verify current admin hasn't changed
        assert_eq!(
            client.admin(),
            admin,
            "admin must not change until accepted"
        );

        // 4. Attempt to accept before delay elapses - should fail
        let result = client.try_accept_admin();
        assert!(result.is_err(), "accept before delay elapses must fail");

        // 5. Advance time past the delay
        let delay = client.admin_transfer_delay();
        env.ledger().with_mut(|l| {
            l.timestamp += delay;
        });

        // 6. Accept admin - should succeed
        client.accept_admin();

        // 7. Verify admin has changed
        assert_eq!(client.admin(), new_admin);

        // 8. Verify pending transfer is cleared
        assert_eq!(
            client.pending_admin(),
            None,
            "pending transfer must be cleared after acceptance"
        );
    }

    // ── Snapshots (#429) ──────────────────────────────────────────────────

    #[test]
    fn test_snapshot_record_balance_at_round_trip() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let holder = Address::generate(&env);
        client.mint(&admin, &holder, &500);
        env.ledger().with_mut(|l| l.sequence_number = 10);
        client.snapshot(&10);
        client.record_balance_snapshot(&holder, &10);

        // Later balance changes do not affect the recorded snapshot.
        env.ledger().with_mut(|l| l.sequence_number = 20);
        client.mint(&admin, &holder, &250);
        client.snapshot(&20);
        client.record_balance_snapshot(&holder, &20);

        assert_eq!(client.balance_at(&holder, &10), 500);
        assert_eq!(client.balance_at(&holder, &20), 750);
        assert_eq!(client.balance(&holder), 750);
    }

    /// #424: `snapshot` is only a marker; balances are captured per address
    /// by `record_balance_snapshot`, and only for the addresses recorded.
    #[test]
    fn test_snapshot_workflow_marker_then_per_address_record() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        let carol = Address::generate(&env);
        client.mint(&admin, &alice, &300);
        client.mint(&admin, &bob, &700);
        client.mint(&admin, &carol, &50);

        // The marker alone stores no balances.
        env.ledger().with_mut(|li| li.sequence_number = 42);
        client.snapshot(&42);
        assert_eq!(client.balance_at(&alice, &42), 0);
        assert_eq!(client.balance_at(&bob, &42), 0);

        // Record alice and bob only, then let balances move.
        client.record_balance_snapshot(&alice, &42);
        client.record_balance_snapshot(&bob, &42);
        client.transfer(&bob, &alice, &200);

        assert_eq!(client.balance(&alice), 500);
        assert_eq!(client.balance(&bob), 500);
        assert_eq!(client.balance_at(&alice, &42), 300);
        assert_eq!(client.balance_at(&bob, &42), 700);
        // carol was never recorded, so nothing is stored for her.
        assert_eq!(client.balance_at(&carol, &42), 0);
        // A different height is an independent snapshot.
        assert_eq!(client.balance_at(&alice, &43), 0);
    }

    #[test]
    fn test_balance_at_unknown_snapshot_is_zero() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let holder = Address::generate(&env);
        client.mint(&admin, &holder, &500);
        assert_eq!(client.balance_at(&holder, &99), 0);
        assert_eq!(client.balance_at(&Address::generate(&env), &99), 0);
    }

    #[test]
    fn test_snapshot_marker_cannot_be_backdated() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.sequence_number = 100);
        let err = client.try_snapshot(&99)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::ExpirationInPast);
    }

    #[test]
    fn test_balance_snapshot_cannot_be_backdated() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.sequence_number = 100);
        let err = client.try_record_balance_snapshot(&Address::generate(&env), &99)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::ExpirationInPast);
    }

    #[test]
    fn test_snapshot_requires_admin_auth() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let holder = Address::generate(&env);

        assert!(client.try_snapshot(&10).is_err());
        assert!(client.try_record_balance_snapshot(&holder, &10).is_err());
        assert_eq!(client.balance_at(&holder, &10), 0);
    }

    // ── Permit (#429) ─────────────────────────────────────────────────────

    #[test]
    fn test_permit_sets_allowance_and_increments_nonce() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        assert_eq!(client.permit_nonce(&owner), 0);

        client.permit(&owner, &spender, &300, &1_000, &0);
        assert_eq!(client.allowance(&owner, &spender), 300);
        assert_eq!(client.permit_nonce(&owner), 1);
    }

    #[test]
    fn test_permit_expired_is_rejected() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.sequence_number = 500);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        assert!(client.try_permit(&owner, &spender, &300, &500, &0).is_err());
        assert!(client.try_permit(&owner, &spender, &300, &100, &0).is_err());
        assert_eq!(client.permit_nonce(&owner), 0);
    }

    #[test]
    fn test_permit_nonce_replay_is_rejected() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        client.permit(&owner, &spender, &300, &1_000, &0);

        // Replaying nonce 0 fails; a future nonce fails too.
        assert!(client
            .try_permit(&owner, &spender, &900, &1_000, &0)
            .is_err());
        assert!(client
            .try_permit(&owner, &spender, &900, &1_000, &5)
            .is_err());
        assert_eq!(client.allowance(&owner, &spender), 300);
        assert_eq!(client.permit_nonce(&owner), 1);
    }

    #[test]
    fn test_permit_wrong_signer_is_rejected() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        // Only the spender signs; the owner's authorization is missing.
        env.mock_auths(&[soroban_sdk::testutils::MockAuth {
            address: &spender,
            invoke: &soroban_sdk::testutils::MockAuthInvoke {
                contract: &contract_id,
                fn_name: "permit",
                args: (&owner, &spender, 300_i128, 1_000_u32, 0_u64).into_val(&env),
                sub_invokes: &[],
            },
        }]);
        assert!(client
            .try_permit(&owner, &spender, &300, &1_000, &0)
            .is_err());
        assert_eq!(client.permit_nonce(&owner), 0);
    }

    // ── estimate_claim_gas (#429) ─────────────────────────────────────────

    #[test]
    fn test_estimate_claim_gas_normal() {
        let env = Env::default();
        let (_admin, contract_id, pt_contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 80);

        let est = client.estimate_claim_gas(&learner, &course_id, &quiz_id);
        assert!(est.would_succeed);
        assert_eq!(est.estimated_reward, 8000);
        assert!(est.estimated_gas > 0);
        assert_eq!(est.failure_reason, SorobanString::from_str(&env, ""));
        // Preview only: nothing is minted.
        assert_eq!(client.total_supply(), 0);
    }

    #[test]
    fn test_estimate_claim_gas_already_claimed() {
        let env = Env::default();
        let (_admin, contract_id, pt_contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 80);
        client.claim_reward(&learner, &course_id, &quiz_id);

        let est = client.estimate_claim_gas(&learner, &course_id, &quiz_id);
        assert!(!est.would_succeed);
        assert_eq!(est.estimated_reward, 0);
        assert_eq!(
            est.failure_reason,
            SorobanString::from_str(&env, "reward already claimed")
        );
    }

    #[test]
    fn test_estimate_claim_gas_is_side_effect_free_and_matches_claim() {
        let env = Env::default();
        let (_admin, contract_id, pt_contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 55);

        let first = client.estimate_claim_gas(&learner, &course_id, &quiz_id);
        let second = client.estimate_claim_gas(&learner, &course_id, &quiz_id);
        assert_eq!(first, second);

        client.claim_reward(&learner, &course_id, &quiz_id);
        assert_eq!(client.balance(&learner), first.estimated_reward);
    }

    #[test]
    fn test_estimate_claim_gas_supply_cap() {
        let env = Env::default();
        let (admin, contract_id, pt_contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 80);
        client.mint(&admin, &admin, &(1_000_000_000_000_000 - 100));

        let est = client.estimate_claim_gas(&learner, &course_id, &quiz_id);
        assert!(!est.would_succeed);
        assert_eq!(
            est.failure_reason,
            SorobanString::from_str(&env, "maximum supply cap exceeded")
        );
    }

    // ── claim_vested supply cap (#431) ────────────────────────────────────

    fn setup_vesting(env: &Env, max_supply: i128) -> (Address, LearnTokenClient<'_>) {
        let admin = Address::generate(env);
        let pt_contract_id = env.register_contract(None, ProgressTracker);
        ProgressTrackerClient::new(env, &pt_contract_id).initialize(&admin);
        let contract_id = env.register_contract(None, LearnToken);
        let client = LearnTokenClient::new(env, &contract_id);
        client.initialize(
            &admin,
            &SorobanString::from_str(env, "CLearn"),
            &SorobanString::from_str(env, "CLRN"),
            &7,
            &pt_contract_id,
            &max_supply,
        );
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = 0);
        (admin, client)
    }

    #[test]
    fn test_claim_vested_at_exact_supply_cap_succeeds() {
        let env = Env::default();
        let (_admin, client) = setup_vesting(&env, 10_000);
        let beneficiary = Address::generate(&env);
        client.create_vesting(&beneficiary, &10_000, &0, &100);
        env.ledger().with_mut(|l| l.timestamp = 100);

        client.claim_vested(&beneficiary);
        assert_eq!(client.total_supply(), 10_000);
    }

    #[test]
    fn test_claim_vested_over_supply_cap_panics() {
        let env = Env::default();
        let (_admin, client) = setup_vesting(&env, 10_000);
        let beneficiary = Address::generate(&env);
        client.create_vesting(&beneficiary, &10_000, &0, &100);
        client.set_max_supply(&9_999);
        env.ledger().with_mut(|l| l.timestamp = 100);
        let err = client.try_claim_vested(&beneficiary)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::MaxSupplyExceeded);
    }

    #[test]
    fn test_vesting_remaining_supply_check_near_i128_max() {
        let env = Env::default();
        let (admin, client) = setup_vesting(&env, i128::MAX);
        client.mint(&admin, &admin, &(i128::MAX - 10));
        let beneficiary = Address::generate(&env);
        let err = client.try_create_vesting(&beneficiary, &11, &0, &100)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::MaxSupplyExceeded);
    }

    // ── #522: SEP-41 burn_from unit tests ───────────────────────────────────

    #[test]
    fn test_burn_from_with_valid_allowance_reduces_balance_supply_allowance() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &owner, &1000);
        client.approve(&owner, &spender, &300, &999999);

        assert_eq!(client.balance(&owner), 1000);
        assert_eq!(client.total_supply(), 1000);
        assert_eq!(client.allowance(&owner, &spender), 300);

        client.burn_from(&spender, &owner, &200);

        assert_eq!(client.balance(&owner), 800);
        assert_eq!(client.total_supply(), 800);
        assert_eq!(client.allowance(&owner, &spender), 100);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #6)")]
    fn test_burn_from_with_insufficient_allowance_panics() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &owner, &1000);
        client.approve(&owner, &spender, &100, &999999);

        client.burn_from(&spender, &owner, &101);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #5)")]
    fn test_burn_from_from_zero_balance_panics() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        // Spender has allowance, but owner balance is zero
        client.approve(&owner, &spender, &500, &999999);
        assert_eq!(client.balance(&owner), 0);

        client.burn_from(&spender, &owner, &100);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #6)")]
    fn test_burn_from_with_expired_allowance_panics() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &owner, &1000);
        let exp_ledger = env.ledger().sequence() + 10;
        client.approve(&owner, &spender, &500, &exp_ledger);

        env.ledger()
            .with_mut(|l| l.sequence_number = exp_ledger + 1);

        client.burn_from(&spender, &owner, &100);
    }

    #[test]
    fn test_burn_from_emits_event_with_correct_topics() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &owner, &1000);
        client.approve(&owner, &spender, &300, &999999);

        client.burn_from(&spender, &owner, &200);

        let all_events = env.events().all();
        let last_event = all_events.last().expect("expected burn_from event");
        assert_eq!(
            soroban_sdk::vec![&env, last_event],
            soroban_sdk::vec![
                &env,
                (
                    contract_id.clone(),
                    (Symbol::new(&env, "burn_from"), owner.clone()).into_val(&env),
                    (spender.clone(), 200i128).into_val(&env),
                )
            ]
        );
    }

    // ── #523: total_minted_to unit tests ────────────────────────────────────

    #[test]
    fn test_total_minted_to_returns_zero_for_unminted_address() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        assert_eq!(client.total_minted_to(&user), 0);
    }

    #[test]
    fn test_total_minted_to_returns_correct_amount_after_single_mint() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &user, &500);
        assert_eq!(client.total_minted_to(&user), 500);
    }

    #[test]
    fn test_total_minted_to_accumulates_across_multiple_mints() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &user, &500);
        assert_eq!(client.total_minted_to(&user), 500);

        client.mint(&admin, &user, &300);
        assert_eq!(client.total_minted_to(&user), 800);

        client.mint(&admin, &user, &200);
        assert_eq!(client.total_minted_to(&user), 1000);
    }

    #[test]
    fn test_total_minted_to_is_independent_per_address() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &alice, &400);
        assert_eq!(client.total_minted_to(&alice), 400);
        assert_eq!(client.total_minted_to(&bob), 0);

        client.mint(&admin, &bob, &700);
        assert_eq!(client.total_minted_to(&alice), 400);
        assert_eq!(client.total_minted_to(&bob), 700);
    }

    #[test]
    fn test_total_minted_to_survives_transfer() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &alice, &1000);
        assert_eq!(client.total_minted_to(&alice), 1000);

        client.transfer(&alice, &bob, &600);
        assert_eq!(client.balance(&alice), 400);
        assert_eq!(client.balance(&bob), 600);

        // Cumulative invariant: total_minted_to never decreases from transfers,
        // and transfers to bob do not count as mints to bob.
        assert_eq!(client.total_minted_to(&alice), 1000);
        assert_eq!(client.total_minted_to(&bob), 0);
    }

    // ── #524: get_claim_history unit tests ──────────────────────────────────

    #[test]
    fn test_get_claim_history_returns_empty_vec_for_learner_with_no_claims() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        let history = client.get_claim_history(&learner);
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_get_claim_history_returns_single_claim_after_one_reward() {
        let env = Env::default();
        let (_admin, contract_id, pt_contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        env.ledger().with_mut(|li| li.timestamp = 1000);
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 80);

        client.claim_reward(&learner, &course_id, &quiz_id);

        let history = client.get_claim_history(&learner);
        assert_eq!(history.len(), 1);

        let record: ClaimRecord = history.get(0).unwrap();
        assert_eq!(record.course_id, course_id);
        assert_eq!(record.quiz_id, quiz_id);
        assert_eq!(record.amount, 8000); // 80 * BASE_REWARD_PER_POINT (100)
        assert_eq!(record.timestamp, 1000);
    }

    #[test]
    fn test_get_claim_history_returns_all_claims_in_order_after_multiple_rewards() {
        let env = Env::default();
        let (_admin, contract_id, pt_contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "course_multi");
        let q1 = Symbol::new(&env, "quiz_1");
        let q2 = Symbol::new(&env, "quiz_2");

        let mut module_ids = Vec::new(&env);
        module_ids.push_back(Symbol::new(&env, "mod_1"));
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(q1.clone());
        quiz_ids.push_back(q2.clone());
        pt_client.create_course(&course_id, &1, &2, &module_ids, &quiz_ids);
        pt_client.enroll(&learner, &course_id);

        // Claim 1
        env.ledger().with_mut(|li| li.timestamp = 1000);
        pt_client.submit_quiz_score(&learner, &course_id, &q1, &70);
        client.claim_reward(&learner, &course_id, &q1);

        // Claim 2
        env.ledger().with_mut(|li| li.timestamp = 2000);
        pt_client.submit_quiz_score(&learner, &course_id, &q2, &90);
        client.claim_reward(&learner, &course_id, &q2);

        let history = client.get_claim_history(&learner);
        assert_eq!(history.len(), 2);

        let r0 = history.get(0).unwrap();
        assert_eq!(r0.course_id, course_id);
        assert_eq!(r0.quiz_id, q1);
        assert_eq!(r0.amount, 7000);
        assert_eq!(r0.timestamp, 1000);

        let r1 = history.get(1).unwrap();
        assert_eq!(r1.course_id, course_id);
        assert_eq!(r1.quiz_id, q2);
        assert_eq!(r1.amount, 9000);
        assert_eq!(r1.timestamp, 2000);
    }

    #[test]
    fn test_get_claim_history_claims_from_different_courses_included() {
        let env = Env::default();
        let (_admin, contract_id, pt_contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_a = Symbol::new(&env, "math_101");
        let quiz_a = Symbol::new(&env, "math_q1");
        let course_b = Symbol::new(&env, "rust_101");
        let quiz_b = Symbol::new(&env, "rust_q1");

        env.ledger().with_mut(|li| li.timestamp = 1500);
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_a, &quiz_a, 60);
        client.claim_reward(&learner, &course_a, &quiz_a);

        env.ledger().with_mut(|li| li.timestamp = 2500);
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_b, &quiz_b, 95);
        client.claim_reward(&learner, &course_b, &quiz_b);

        let history = client.get_claim_history(&learner);
        assert_eq!(history.len(), 2);

        let r0 = history.get(0).unwrap();
        assert_eq!(r0.course_id, course_a);
        assert_eq!(r0.quiz_id, quiz_a);
        assert_eq!(r0.amount, 6000);
        assert_eq!(r0.timestamp, 1500);

        let r1 = history.get(1).unwrap();
        assert_eq!(r1.course_id, course_b);
        assert_eq!(r1.quiz_id, quiz_b);
        assert_eq!(r1.amount, 9500);
        assert_eq!(r1.timestamp, 2500);
    }

    #[test]
    fn test_get_claim_history_batch_claims_appear_in_history() {
        let env = Env::default();
        let (_admin, contract_id, pt_contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "batch_course");
        let q1 = Symbol::new(&env, "q1");
        let q2 = Symbol::new(&env, "q2");

        let mut module_ids = Vec::new(&env);
        module_ids.push_back(Symbol::new(&env, "mod_1"));
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(q1.clone());
        quiz_ids.push_back(q2.clone());
        pt_client.create_course(&course_id, &1, &2, &module_ids, &quiz_ids);
        pt_client.enroll(&learner, &course_id);

        pt_client.submit_quiz_score(&learner, &course_id, &q1, &75);
        pt_client.submit_quiz_score(&learner, &course_id, &q2, &85);

        env.ledger().with_mut(|li| li.timestamp = 3000);

        let mut quizzes = Vec::new(&env);
        quizzes.push_back(q1.clone());
        quizzes.push_back(q2.clone());

        client.batch_claim_reward(&learner, &course_id, &quizzes);

        let history = client.get_claim_history(&learner);
        assert_eq!(history.len(), 2);

        let r0 = history.get(0).unwrap();
        assert_eq!(r0.course_id, course_id);
        assert_eq!(r0.quiz_id, q1);
        assert_eq!(r0.amount, 7500);
        assert_eq!(r0.timestamp, 3000);

        let r1 = history.get(1).unwrap();
        assert_eq!(r1.course_id, course_id);
        assert_eq!(r1.quiz_id, q2);
        assert_eq!(r1.amount, 8500);
        assert_eq!(r1.timestamp, 3000);
    }

    // ── #525: increase_allowance and decrease_allowance unit tests ───────────

    #[test]
    fn test_increase_allowance_adds_to_existing() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        let exp = env.ledger().sequence() + 1000;
        client.approve(&owner, &spender, &100, &exp);
        assert_eq!(client.allowance(&owner, &spender), 100);

        client.increase_allowance(&owner, &spender, &50, &exp);
        assert_eq!(client.allowance(&owner, &spender), 150);
    }

    #[test]
    fn test_increase_allowance_from_zero_works() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        assert_eq!(client.allowance(&owner, &spender), 0);

        let exp = env.ledger().sequence() + 1000;
        client.increase_allowance(&owner, &spender, &250, &exp);
        assert_eq!(client.allowance(&owner, &spender), 250);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #7)")]
    fn test_increase_allowance_overflow_panics() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        let exp = env.ledger().sequence() + 1000;
        client.approve(&owner, &spender, &i128::MAX, &exp);
        client.increase_allowance(&owner, &spender, &1, &exp);
    }

    #[test]
    fn test_decrease_allowance_subtracts_from_existing() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        let exp = env.ledger().sequence() + 1000;
        client.approve(&owner, &spender, &200, &exp);
        client.decrease_allowance(&owner, &spender, &80);
        assert_eq!(client.allowance(&owner, &spender), 120);
    }

    #[test]
    fn test_decrease_allowance_underflow_panics() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();
        let exp = env.ledger().sequence() + 1000;
        client.approve(&owner, &spender, &50, &exp);
        let err = client.try_decrease_allowance(&owner, &spender, &51)
            .err().expect("should fail").expect("expected typed error");
        assert_eq!(err, ContractError::InsufficientAllowance);
    }

    #[test]
    fn test_decrease_allowance_to_exactly_zero_works() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        let exp = env.ledger().sequence() + 1000;
        client.approve(&owner, &spender, &100, &exp);
        client.decrease_allowance(&owner, &spender, &100);
        assert_eq!(client.allowance(&owner, &spender), 0);
    }

    #[test]
    fn test_increase_and_decrease_allowance_emit_approve_event() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        let exp = env.ledger().sequence() + 1000;
        client.increase_allowance(&owner, &spender, &150, &exp);

        let all_events = env.events().all();
        let last_event = all_events.last().expect("expected approve event from increase");
        assert_eq!(
            soroban_sdk::vec![&env, last_event],
            soroban_sdk::vec![
                &env,
                (
                    contract_id.clone(),
                    (Symbol::new(&env, "approve"), owner.clone(), spender.clone()).into_val(&env),
                    (150i128, exp).into_val(&env),
                )
            ]
        );

        client.decrease_allowance(&owner, &spender, &50);

        let all_events = env.events().all();
        let last_event = all_events.last().expect("expected approve event from decrease");
        assert_eq!(
            soroban_sdk::vec![&env, last_event],
            soroban_sdk::vec![
                &env,
                (
                    contract_id.clone(),
                    (Symbol::new(&env, "approve"), owner.clone(), spender.clone()).into_val(&env),
                    (100i128, exp).into_val(&env),
                )
            ]
        );
    }

    // ── #505: dedicated query-function unit tests ───────────────────────────

    #[test]
    fn test_get_storage_size_tracks_mint_and_claim_entries() {
        let env = Env::default();
        let (admin, contract_id, pt_contract_id) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_contract_id);
        env.mock_all_auths();

        // initialize() records the primary admin role as one tracked entry.
        assert_eq!(client.get_storage_size(), 1);

        let user = Address::generate(&env);
        client.mint(&admin, &user, &1_000);
        assert_eq!(client.get_storage_size(), 2);

        client.mint(&admin, &user, &500);
        assert_eq!(client.get_storage_size(), 2);

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 80);
        client.claim_reward(&learner, &course_id, &quiz_id);
        // RewardClaimed + ClaimHistory + TotalMintedTo(learner)
        assert_eq!(client.get_storage_size(), 5);
    }

    #[test]
    fn test_get_vesting_schedule_returns_none_until_created() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let beneficiary = Address::generate(&env);

        assert!(client.get_vesting_schedule(&beneficiary).is_none());
        assert_eq!(client.get_vesting_claimed(&beneficiary), 0);

        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = 50);
        client.create_vesting(&beneficiary, &10_000, &100, &1_000);

        let schedule = client
            .get_vesting_schedule(&beneficiary)
            .expect("schedule should exist after create_vesting");
        assert_eq!(schedule.total_amount, 10_000);
        assert_eq!(schedule.cliff_timestamp, 100);
        assert_eq!(schedule.duration_seconds, 1_000);
        assert_eq!(schedule.created_at, 50);
        assert!(!schedule.exhausted);
        assert!(client
            .get_vesting_schedule(&Address::generate(&env))
            .is_none());
    }

    #[test]
    fn test_permit_nonce_starts_at_zero_and_increments_only_on_success() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        assert_eq!(client.permit_nonce(&owner), 0);

        client.permit(&owner, &spender, &100, &1_000, &0);
        assert_eq!(client.permit_nonce(&owner), 1);
        client.permit(&owner, &spender, &200, &1_000, &1);
        assert_eq!(client.permit_nonce(&owner), 2);

        assert!(client
            .try_permit(&owner, &spender, &300, &1_000, &1)
            .is_err());
        assert_eq!(client.permit_nonce(&owner), 2);
    }

    #[test]
    fn test_get_admins_lists_primary_and_added_admins() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let admins = client.get_admins();
        assert_eq!(admins.len(), 1);
        assert_eq!(admins.get(0).unwrap().address, admin);
        assert_eq!(admins.get(0).unwrap().role, AdminRole::Admin);

        let minter = Address::generate(&env);
        client.add_admin(
            &admin,
            &AdminInfo {
                address: minter.clone(),
                role: AdminRole::Minter,
            },
        );

        let admins = client.get_admins();
        assert_eq!(admins.len(), 2);
        assert_eq!(admins.get(1).unwrap().address, minter);
        assert_eq!(admins.get(1).unwrap().role, AdminRole::Minter);

        client.remove_admin(
            &admin,
            &AdminInfo {
                address: minter.clone(),
                role: AdminRole::Minter,
            },
        );
        let admins = client.get_admins();
        assert_eq!(admins.len(), 1);
        assert_eq!(admins.get(0).unwrap().address, admin);
    }

    #[test]
    fn test_has_role_reflects_primary_admin_and_granted_roles() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        assert!(client.has_role(&admin, &AdminRole::Admin));
        assert!(client.has_role(&admin, &AdminRole::Minter));
        assert!(client.has_role(&admin, &AdminRole::Pauser));

        let stranger = Address::generate(&env);
        assert!(!client.has_role(&stranger, &AdminRole::Admin));
        assert!(!client.has_role(&stranger, &AdminRole::Minter));

        let minter = Address::generate(&env);
        client.grant_role(&admin, &minter, &AdminRole::Minter);
        assert!(client.has_role(&minter, &AdminRole::Minter));
        assert!(!client.has_role(&minter, &AdminRole::Pauser));
        assert!(!client.has_role(&minter, &AdminRole::Admin));

        client.revoke_role(&admin, &minter, &AdminRole::Minter);
        assert!(!client.has_role(&minter, &AdminRole::Minter));
    }

    #[test]
    fn test_prune_expired_allowance_removes_expired_entry_only() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let owner = Address::generate(&env);
        let expired_spender = Address::generate(&env);
        let active_spender = Address::generate(&env);
        let expiring_ledger = env.ledger().sequence() + 5;
        let valid_ledger = env.ledger().sequence() + 999_999;

        client.approve(&owner, &expired_spender, &100, &expiring_ledger);
        client.approve(&owner, &active_spender, &200, &valid_ledger);
        assert!(!client.prune_expired_allowance(&owner, &expired_spender));
        assert_eq!(client.allowance_spender_count(&owner), 2);

        env.ledger().with_mut(|l| {
            l.sequence_number = expiring_ledger + 1;
        });

        assert!(client.prune_expired_allowance(&owner, &expired_spender));
        assert_eq!(client.allowance(&owner, &expired_spender), 0);
        assert_eq!(client.allowance(&owner, &active_spender), 200);
        assert_eq!(client.allowance_spender_count(&owner), 1);
        assert!(!client.prune_expired_allowance(&owner, &expired_spender));
        assert!(!client.prune_expired_allowance(&owner, &active_spender));
        assert!(!client.prune_expired_allowance(&owner, &Address::generate(&env)));
    }
}
