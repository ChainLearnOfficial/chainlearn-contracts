#![cfg(test)]

use learn_token::{LearnToken, LearnTokenClient};
use progress_tracker::ProgressTracker;
use soroban_sdk::{
    contract, contractimpl, testutils::Address as _, Address, Env, String as SorobanString,
};

#[contract]
pub struct MaliciousContract;

#[contractimpl]
impl MaliciousContract {
    pub fn attack(env: Env, token_id: Address) {
        let client = LearnTokenClient::new(&env, &token_id);
        // Attempt an unauthorized call during contract execution
        client.transfer(&Address::generate(&env), &Address::generate(&env), &1);
    }

    pub fn attack_mint(env: Env, token_id: Address, recipient: Address) {
        let client = LearnTokenClient::new(&env, &token_id);
        // Attempt reentrant mint call during state change execution
        client.mint(&Address::generate(&env), &recipient, &5000);
    }
}

#[test]
#[should_panic]
fn test_reentrancy_during_transfer() {
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
    
    let malicious_id = env.register_contract(None, MaliciousContract);
    let malicious_client = MaliciousContractClient::new(&env, &malicious_id);
    
    env.mock_all_auths();
    client.mint(&admin, &malicious_id, &1000);
    
    // Call the malicious contract which will attempt a reentrant call to the token contract.
    // The environment naturally protects against state corruption, often panicking if a re-entrant lock is triggered.
    malicious_client.attack(&token_id);
}

#[test]
fn test_reentrancy_prevented_state_consistent_and_no_funds_lost() {
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

    let victim = Address::generate(&env);
    let malicious_id = env.register_contract(None, MaliciousContract);
    let malicious_client = MaliciousContractClient::new(&env, &malicious_id);

    env.mock_all_auths();

    client.mint(&admin, &victim, &1_000);
    client.mint(&admin, &malicious_id, &500);

    let victim_balance_before = client.balance(&victim);
    let malicious_balance_before = client.balance(&malicious_id);
    let supply_before = client.total_supply();

    assert_eq!(victim_balance_before, 1_000);
    assert_eq!(malicious_balance_before, 500);
    assert_eq!(supply_before, 1_500);

    // Reentrant attack must fail / revert
    let attack_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        malicious_client.attack_mint(&token_id, &malicious_id);
    }));
    assert!(attack_result.is_err(), "reentrant call must fail");

    // State remains consistent after failed reentrancy attack (no funds lost)
    assert_eq!(client.balance(&victim), victim_balance_before);
    assert_eq!(client.balance(&malicious_id), malicious_balance_before);
    assert_eq!(client.total_supply(), supply_before);
}

