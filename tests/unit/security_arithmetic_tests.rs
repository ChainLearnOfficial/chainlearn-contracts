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
