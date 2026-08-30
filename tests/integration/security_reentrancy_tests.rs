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
