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
