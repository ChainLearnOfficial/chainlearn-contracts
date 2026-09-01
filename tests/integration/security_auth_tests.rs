#![cfg(test)]

use learn_token::{AdminRole, LearnTokenClient};
use progress_tracker::ProgressTracker;
use soroban_sdk::{testutils::Address as _, Address, Env, String as SorobanString, Symbol, BytesN};

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

#[test]
#[should_panic]
fn test_unauthorized_revoke_role() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let existing_admin = Address::generate(&env);
    client.revoke_role(&malicious, &existing_admin, &AdminRole::Admin);
}

#[test]
#[should_panic]
fn test_unauthorized_add_admin() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let admin_info = learn_token::AdminInfo {
        address: new_admin,
        role: AdminRole::Admin,
    };
    client.add_admin(&malicious, &admin_info);
}

#[test]
#[should_panic]
fn test_unauthorized_remove_admin() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let admin_to_remove = Address::generate(&env);
    let admin_info = learn_token::AdminInfo {
        address: admin_to_remove,
        role: AdminRole::Admin,
    };
    client.remove_admin(&malicious, &admin_info);
}

#[test]
#[should_panic]
fn test_unauthorized_execute_multisig() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let co_signer = Address::generate(&env);
    client.execute_multisig_op(&malicious, &co_signer, &Symbol::new(env, "test"));
}

#[test]
#[should_panic]
fn test_unauthorized_upgrade_multisig() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let co_signer = Address::generate(&env);
    client.upgrade_multisig(&malicious, &co_signer, &BytesN::from_array(env, &[0; 32]));
}

#[test]
#[should_panic]
fn test_unauthorized_pause_unpaused() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    client.unpause(&malicious);
}

#[test]
#[should_panic]
fn test_unauthorized_set_max_supply() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    client.set_max_supply(&10000);
}

#[test]
#[should_panic]
fn test_unauthorized_upgrade() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    client.upgrade(&BytesN::from_array(env, &[0; 32]));
}

#[test]
#[should_panic]
fn test_unauthorized_transfer_admin() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let new_admin = Address::generate(&env);
    client.transfer_admin(&new_admin);
}

#[test]
#[should_panic]
fn test_unauthorized_cancel_admin_transfer() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    client.cancel_admin_transfer();
}

#[test]
#[should_panic]
fn test_unauthorized_set_admin_transfer_delay() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    client.set_admin_transfer_delay(&60);
}

#[test]
#[should_panic]
fn test_unauthorized_set_progress_tracker() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let new_tracker = Address::generate(&env);
    client.set_progress_tracker(&new_tracker);
}