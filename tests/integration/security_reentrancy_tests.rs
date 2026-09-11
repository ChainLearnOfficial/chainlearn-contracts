#![cfg(test)]

use credential_nft::{CredentialNft, CredentialNftClient};
use learn_token::{LearnToken, LearnTokenClient};
use progress_tracker::ProgressTracker;
use soroban_sdk::{
    contract, contractimpl, symbol_short, testutils::Address as _, Address, Env,
    String as SorobanString, Symbol,
};

mod malicious_base {
    use super::*;

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
}
use malicious_base::*;

mod reentrant_claim_pt {
    use super::*;

    /// Malicious progress tracker that attempts same-function reentrancy
    /// into `claim_reward` during `get_quiz_score` (#354).
    #[contract]
    pub struct ReentrantClaimProgressTracker;

    #[contractimpl]
    impl ReentrantClaimProgressTracker {
        pub fn set_target(env: Env, target: Address) {
            env.storage()
                .instance()
                .set(&symbol_short!("target"), &target);
        }

        pub fn get_quiz_score(
            env: Env,
            learner: Address,
            course_id: Symbol,
            quiz_id: Symbol,
        ) -> u32 {
            if let Some(target) = env
                .storage()
                .instance()
                .get::<_, Address>(&symbol_short!("target"))
            {
                let token_client = LearnTokenClient::new(&env, &target);
                // Attempt reentrancy into claim_reward while the original claim_reward is in flight
                token_client.claim_reward(&learner, &course_id, &quiz_id);
            }
            85
        }
    }
}
use reentrant_claim_pt::*;

mod cross_func_pt {
    use super::*;

    /// Malicious progress tracker that attempts cross-function reentrancy
    /// into `mint` during `get_quiz_score` (#354).
    #[contract]
    pub struct CrossFunctionReentrantProgressTracker;

    #[contractimpl]
    impl CrossFunctionReentrantProgressTracker {
        pub fn set_target(env: Env, target: Address, admin: Address) {
            env.storage()
                .instance()
                .set(&symbol_short!("target"), &target);
            env.storage()
                .instance()
                .set(&symbol_short!("admin"), &admin);
        }

        pub fn get_quiz_score(
            env: Env,
            learner: Address,
            _course_id: Symbol,
            _quiz_id: Symbol,
        ) -> u32 {
            if let Some(target) = env
                .storage()
                .instance()
                .get::<_, Address>(&symbol_short!("target"))
            {
                let admin = env
                    .storage()
                    .instance()
                    .get::<_, Address>(&symbol_short!("admin"))
                    .unwrap();
                let token_client = LearnTokenClient::new(&env, &target);
                // Attempt reentrant mint call while claim_reward is in flight
                token_client.mint(&admin, &learner, &5_000);
            }
            90
        }
    }
}
use cross_func_pt::*;

mod reentrant_cred_pt {
    use super::*;

    /// Malicious progress tracker that attempts reentrancy into
    /// `CredentialNft::mint_credential` during eligibility check (#354).
    #[contract]
    pub struct ReentrantCredentialTracker;

    #[contractimpl]
    impl ReentrantCredentialTracker {
        pub fn set_target(env: Env, target: Address) {
            env.storage()
                .instance()
                .set(&symbol_short!("target"), &target);
        }

        pub fn course_exists(_env: Env, _course_id: Symbol) -> bool {
            true
        }

        pub fn is_eligible_for_credential(env: Env, learner: Address, course_id: Symbol) -> bool {
            if let Some(target) = env
                .storage()
                .instance()
                .get::<_, Address>(&symbol_short!("target"))
            {
                let nft_client = CredentialNftClient::new(&env, &target);
                let meta = Symbol::new(&env, "ipfs_reentrant");
                // Attempt reentrant mint while outer mint is evaluating eligibility
                nft_client.mint_credential(&learner, &course_id, &85, &meta);
            }
            true
        }

        pub fn get_course_score(_env: Env, _learner: Address, _course_id: Symbol) -> u32 {
            85
        }
    }
}
use reentrant_cred_pt::*;

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

#[test]
fn test_reentrancy_during_claim_reward_blocked_by_guard() {
    let env = Env::default();
    let admin = Address::generate(&env);

    // Register malicious tracker that attempts to re-enter claim_reward
    let malicious_pt_id = env.register_contract(None, ReentrantClaimProgressTracker);
    let pt_client = ReentrantClaimProgressTrackerClient::new(&env, &malicious_pt_id);

    // Register learn-token with the malicious tracker
    let token_id = env.register_contract(None, LearnToken);
    let token_client = LearnTokenClient::new(&env, &token_id);

    token_client.initialize(
        &admin,
        &SorobanString::from_str(&env, "ChainLearn"),
        &SorobanString::from_str(&env, "CLRN"),
        &7,
        &malicious_pt_id,
        &1_000_000_000_000_000,
    );

    // Arm the malicious contract with the token contract address
    pt_client.set_target(&token_id);

    env.mock_all_auths();
    let learner = Address::generate(&env);
    let course_id = Symbol::new(&env, "soroban_101");
    let quiz_id = Symbol::new(&env, "quiz_1");

    // Calling claim_reward triggers malicious callback which attempts reentrancy.
    // The ReentrancyGuard detects reentrance and halts execution cleanly.
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        token_client.claim_reward(&learner, &course_id, &quiz_id);
    }));

    assert!(
        res.is_err(),
        "reentrant claim_reward call must be blocked by ReentrancyGuard"
    );
    // Verify zero tokens were minted to the learner
    assert_eq!(token_client.balance(&learner), 0);
}

#[test]
fn test_cross_function_reentrancy_blocked_by_guard() {
    let env = Env::default();
    let admin = Address::generate(&env);

    // Register malicious tracker that attempts to call mint during claim_reward
    let malicious_pt_id = env.register_contract(None, CrossFunctionReentrantProgressTracker);
    let pt_client = CrossFunctionReentrantProgressTrackerClient::new(&env, &malicious_pt_id);

    let token_id = env.register_contract(None, LearnToken);
    let token_client = LearnTokenClient::new(&env, &token_id);

    token_client.initialize(
        &admin,
        &SorobanString::from_str(&env, "ChainLearn"),
        &SorobanString::from_str(&env, "CLRN"),
        &7,
        &malicious_pt_id,
        &1_000_000_000_000_000,
    );

    pt_client.set_target(&token_id, &admin);

    env.mock_all_auths();
    let learner = Address::generate(&env);
    let course_id = Symbol::new(&env, "soroban_101");
    let quiz_id = Symbol::new(&env, "quiz_1");

    // Cross-function reentrancy attack must be rejected
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        token_client.claim_reward(&learner, &course_id, &quiz_id);
    }));

    assert!(
        res.is_err(),
        "cross-function reentrant call into mint must be blocked"
    );
    assert_eq!(token_client.balance(&learner), 0);
    assert_eq!(token_client.total_supply(), 0);
}

#[test]
fn test_credential_nft_reentrancy_blocked_by_guard() {
    let env = Env::default();
    let admin = Address::generate(&env);

    let malicious_pt_id = env.register_contract(None, ReentrantCredentialTracker);
    let pt_client = ReentrantCredentialTrackerClient::new(&env, &malicious_pt_id);

    let nft_id = env.register_contract(None, CredentialNft);
    let nft_client = CredentialNftClient::new(&env, &nft_id);

    nft_client.initialize(&admin, &malicious_pt_id);
    pt_client.set_target(&nft_id);

    env.mock_all_auths();
    let learner = Address::generate(&env);
    let course_id = Symbol::new(&env, "soroban_101");
    let metadata_uri = Symbol::new(&env, "ipfs_legit");

    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        nft_client.mint_credential(&learner, &course_id, &85, &metadata_uri);
    }));

    assert!(
        res.is_err(),
        "reentrant mint_credential call must be blocked"
    );
    assert_eq!(nft_client.get_credential_count(&learner), 0);
}
