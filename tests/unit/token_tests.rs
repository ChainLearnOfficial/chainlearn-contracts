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
    #[should_panic(expected = "maximum supply cap exceeded")]
    fn test_security_mint_supply_overflow_reverts() {
        let env = Env::default();
        let (admin, contract_id, _pt_contract_id) = setup_token_with_max_supply(&env, i128::MAX);
        let client = LearnTokenClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        // Fill supply right up to the cap: current_supply becomes i128::MAX,
        // which does not overflow (i128::MAX + 0 offset is representable).
        client.mint(&admin, &user, &i128::MAX);
        assert_eq!(client.total_supply(), i128::MAX);

        // One more token would push `current_supply + amount` past
        // i128::MAX. `mint` guards this with `checked_add` rather than a
        // raw `+`, so the overflow itself never happens — it surfaces as
        // the same clear, domain-specific panic as an ordinary cap breach
        // instead of a raw arithmetic trap.
        client.mint(&admin, &user, &1);
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
    #[should_panic(expected = "insufficient balance")]
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
    #[should_panic(expected = "insufficient balance")]
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
    #[test]
    fn test_governance_proposal_lifecycle() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let voter = Address::generate(&env);
        client.mint(&admin, &voter, &1000);

        let snapshot_ledger = env.ledger().sequence();
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
    fn test_vesting_schedule_cliff_linear_vesting_and_claiming() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let beneficiary = Address::generate(&env);

        env.mock_all_auths();

        let total_amount: i128 = 10_000;
        let cliff_timestamp: u64 = 100;
        let duration_seconds: u64 = 1_000;

        env.ledger().with_mut(|l| {
            l.timestamp = 0;
        });

        client.create_vesting(&beneficiary, &total_amount, &cliff_timestamp, &duration_seconds);

        let schedule = client.get_vesting_schedule(&beneficiary).expect("schedule should exist");
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
    #[should_panic(expected = "cliff not reached")]
    fn test_vesting_cliff_enforced() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let beneficiary = Address::generate(&env);

        env.mock_all_auths();

        env.ledger().with_mut(|l| {
            l.timestamp = 0;
        });

        client.create_vesting(&beneficiary, &10_000, &100, &1_000);

        // Advance to timestamp 50 (cliff is 100)
        env.ledger().with_mut(|l| {
            l.timestamp = 50;
        });

        client.claim_vested(&beneficiary);
    }

    #[test]
    #[should_panic(expected = "vesting schedule fully claimed")]
    fn test_vesting_claiming_after_exhausted_panics() {
        let env = Env::default();
        let (admin, contract_id, _) = setup_token(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let beneficiary = Address::generate(&env);

        env.mock_all_auths();

        env.ledger().with_mut(|l| {
            l.timestamp = 0;
        });

        client.create_vesting(&beneficiary, &10_000, &100, &1_000);

        env.ledger().with_mut(|l| {
            l.timestamp = 1100;
        });

        client.claim_vested(&beneficiary);
        assert_eq!(client.balance(&beneficiary), 10_000);

        // Attempting to claim again after schedule is exhausted must panic
        client.claim_vested(&beneficiary);
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

        assert_eq!(client.admin(), admin, "admin must not change until accepted");
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
            env.auths()[0].0, new_admin,
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
            env.auths()[0].0, admin,
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
}

