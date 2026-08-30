#![cfg(test)]

use learn_token::{LearnToken, LearnTokenClient};
use progress_tracker::ProgressTracker;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String as SorobanString};

#[test]
fn test_contract_upgrade() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let pt_contract_id = env.register_contract(None, ProgressTracker);
    let token_id = env.register_contract(None, LearnToken);
    let client = LearnTokenClient::new(&env, &token_id);
    
    client.initialize(
        &admin,
        &SorobanString::from_str(&env, "ChainLearn"),
        &SorobanString::from_str(&env, "CLRN"),
        &7,
        &pt_contract_id,
        &1_000_000,
    );
    
    env.mock_all_auths();
    
    let user = Address::generate(&env);
    client.mint(&admin, &user, &100);
    assert_eq!(client.balance(&user), 100);
    assert_eq!(client.upgrade_version(), 0);
    assert_eq!(client.wasm_hash(), None);
    
    // Verify initial upgrade state
    assert_eq!(client.upgrade_version(), 0);
    assert_eq!(client.wasm_hash(), None);
    
    // Verify state before and after upgrade verification
    assert_eq!(client.balance(&user), 100);

    // Verify multi-sig operation for critical upgrades
    let co_admin = Address::generate(&env);
    client.add_admin(&admin, &learn_token::AdminInfo {
        address: co_admin.clone(),
        role: learn_token::AdminRole::Admin,
    });

    let dummy_hash = BytesN::from_array(&env, &[1; 32]);
    let result = client.try_upgrade_multisig(&admin, &admin, &dummy_hash);
    assert!(result.is_err(), "Same co-signer must be rejected");

    assert_eq!(client.balance(&user), 100);
}

#[test]
fn test_contract_upgrade_preserves_state_and_updates_version() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let pt_contract_id = env.register_contract(None, ProgressTracker);
    let token_id = env.register_contract(None, LearnToken);
    let client = LearnTokenClient::new(&env, &token_id);

    client.initialize(
        &admin,
        &SorobanString::from_str(&env, "ChainLearn"),
        &SorobanString::from_str(&env, "CLRN"),
        &7,
        &pt_contract_id,
        &1_000_000,
    );

    env.mock_all_auths();

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.mint(&admin, &alice, &500);
    client.mint(&admin, &bob, &300);

    assert_eq!(client.balance(&alice), 500);
    assert_eq!(client.balance(&bob), 300);
    assert_eq!(client.total_supply(), 800);
    assert_eq!(client.upgrade_version(), 0);
    assert_eq!(client.wasm_hash(), None);

    let new_wasm_hash = BytesN::from_array(&env, &[2u8; 32]);
    client.upgrade(&new_wasm_hash);

    assert_eq!(client.upgrade_version(), 1);
    assert_eq!(client.wasm_hash(), Some(new_wasm_hash));

    // Verify state preserved after upgrade
    assert_eq!(client.balance(&alice), 500);
    assert_eq!(client.balance(&bob), 300);
    assert_eq!(client.total_supply(), 800);
    assert_eq!(client.admin(), admin);
    assert_eq!(client.max_supply(), 1_000_000);

    // Verify contract functions operate correctly post-upgrade
    client.transfer(&alice, &bob, &200);
    assert_eq!(client.balance(&alice), 300);
    assert_eq!(client.balance(&bob), 500);

    client.mint(&admin, &alice, &100);
    assert_eq!(client.balance(&alice), 400);
    assert_eq!(client.total_supply(), 900);
}

