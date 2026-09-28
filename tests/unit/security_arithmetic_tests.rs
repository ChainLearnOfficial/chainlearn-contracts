#![cfg(test)]

use learn_token::LearnTokenClient;
use progress_tracker::ProgressTracker;
use soroban_sdk::{testutils::Address as _, Address, Env, String as SorobanString};

#[test]
#[should_panic(expected = "insufficient balance")]
fn test_underflow_balance_subtraction() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let pt_contract_id = env.register_contract(None, ProgressTracker);
    let contract_id = env.register_contract(None, learn_token::LearnToken);
    let client = LearnTokenClient::new(&env, &contract_id);

    client.initialize(
        &admin,
        &SorobanString::from_str(&env, "ChainLearn"),
        &SorobanString::from_str(&env, "CLRN"),
        &7,
        &pt_contract_id,
        &1_000_000,
    );

    let user = Address::generate(&env);
    env.mock_all_auths();

    client.transfer(&user, &admin, &1000);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn test_underflow_balance_burn() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let pt_contract_id = env.register_contract(None, ProgressTracker);
    let contract_id = env.register_contract(None, learn_token::LearnToken);
    let client = LearnTokenClient::new(&env, &contract_id);

    client.initialize(
        &admin,
        &SorobanString::from_str(&env, "ChainLearn"),
        &SorobanString::from_str(&env, "CLRN"),
        &7,
        &pt_contract_id,
        &1_000_000,
    );

    let user = Address::generate(&env);
    env.mock_all_auths();

    client.burn(&user, &1000);
}

#[test]
#[should_panic(expected = "maximum supply cap exceeded")]
fn test_overflow_supply() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let pt_contract_id = env.register_contract(None, ProgressTracker);
    let contract_id = env.register_contract(None, learn_token::LearnToken);
    let client = LearnTokenClient::new(&env, &contract_id);

    client.initialize(
        &admin,
        &SorobanString::from_str(&env, "ChainLearn"),
        &SorobanString::from_str(&env, "CLRN"),
        &7,
        &pt_contract_id,
        &i128::MAX,
    );

    let user = Address::generate(&env);
    env.mock_all_auths();

    client.mint(&admin, &user, &i128::MAX);
    // This will trigger the maximum supply cap exceeded panic
    client.mint(&admin, &user, &1);
}

#[test]
#[should_panic(expected = "insufficient allowance")]
fn test_underflow_allowance_spend() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let pt_contract_id = env.register_contract(None, ProgressTracker);
    let contract_id = env.register_contract(None, learn_token::LearnToken);
    let client = LearnTokenClient::new(&env, &contract_id);

    client.initialize(
        &admin,
        &SorobanString::from_str(&env, "ChainLearn"),
        &SorobanString::from_str(&env, "CLRN"),
        &7,
        &pt_contract_id,
        &1_000_000,
    );

    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);
    env.mock_all_auths();

    client.mint(&admin, &owner, &1000);
    client.approve(&owner, &spender, &200, &1000);

    // Attempting to spend 201 when allowance is 200 must trigger underflow protection
    client.transfer_from(&spender, &owner, &recipient, &201);
}

#[test]
fn test_safe_transfer_arithmetic_balances() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let pt_contract_id = env.register_contract(None, ProgressTracker);
    let contract_id = env.register_contract(None, learn_token::LearnToken);
    let client = LearnTokenClient::new(&env, &contract_id);

    client.initialize(
        &admin,
        &SorobanString::from_str(&env, "ChainLearn"),
        &SorobanString::from_str(&env, "CLRN"),
        &7,
        &pt_contract_id,
        &1_000_000,
    );

    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);
    env.mock_all_auths();

    client.mint(&admin, &user_a, &500);
    assert_eq!(client.balance(&user_a), 500);
    assert_eq!(client.balance(&user_b), 0);

    client.transfer(&user_a, &user_b, &200);
    assert_eq!(client.balance(&user_a), 300);
    assert_eq!(client.balance(&user_b), 200);
    assert_eq!(client.total_supply(), 500);
}
