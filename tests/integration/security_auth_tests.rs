#![cfg(test)]

use learn_token::{AdminRole, LearnTokenClient};
use progress_tracker::ProgressTracker;
use soroban_sdk::{testutils::Address as _, Address, Env, String as SorobanString};

fn setup_env(env: &Env) -> (Address, LearnTokenClient<'static>) {
    let admin = Address::generate(env);
    let pt_contract_id = env.register_contract(None, ProgressTracker);
    let contract_id = env.register_contract(None, learn_token::LearnToken);
    let client = LearnTokenClient::new(env, &contract_id);
    
    client.initialize(
        &admin,
        &SorobanString::from_str(env, "ChainLearn"),
        &SorobanString::from_str(env, "CLRN"),
        &7,
        &pt_contract_id,
        &1_000_000,
    );
    (admin, client)
}

#[test]
#[should_panic]
fn test_unauthorized_mint() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let recipient = Address::generate(&env);
    // This will panic because we didn't mock auths for 'malicious'
    client.mint(&malicious, &recipient, &1000);
}

#[test]
#[should_panic]
fn test_unauthorized_pause() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    client.pause(&malicious);
}

#[test]
#[should_panic]
fn test_unauthorized_grant_role() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let new_admin = Address::generate(&env);
    client.grant_role(&malicious, &new_admin, &AdminRole::Admin);
}
