//! Test fixtures and setup helpers for integration tests.
//!
//! Provides common setup functions to initialize the contract environment
//! and deploy all three contracts for end-to-end testing.

use learn_token::{LearnToken, LearnTokenClient};
use credential_nft::{CredentialNft, CredentialNftClient};
use progress_tracker::{ProgressTracker, ProgressTrackerClient};
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol, Vec};

/// Represents a fully deployed ChainLearn test environment.
pub struct ChainLearnEnv {
    pub env: Env,
    pub admin: Address,
    pub learner: Address,
    pub token_contract_id: Address,
    pub credential_contract_id: Address,
    pub progress_contract_id: Address,
}

/// Deploy all ChainLearn contracts and initialize them.
pub fn setup_chainlearn_env() -> ChainLearnEnv {
    let env = Env::default();
    let admin = Address::generate(&env);
    let learner = Address::generate(&env);

    // Register and initialize LearnToken
    let token_contract_id = env.register(LearnToken, ());
    let token_client = LearnTokenClient::new(&env, &token_contract_id);
    token_client.initialize(
        &admin,
        &Symbol::new(&env, "CLearn"),
        &Symbol::new(&env, "CLRN"),
        &7,
    );

    // Register and initialize CredentialNft
    let credential_contract_id = env.register(CredentialNft, ());
    let credential_client = CredentialNftClient::new(&env, &credential_contract_id);
    credential_client.initialize(&admin);

    // Register and initialize ProgressTracker
    let progress_contract_id = env.register(ProgressTracker, ());
    let progress_client = ProgressTrackerClient::new(&env, &progress_contract_id);
    progress_client.initialize(&admin);

    ChainLearnEnv {
        env,
        admin,
        learner,
        token_contract_id,
        credential_contract_id,
        progress_contract_id,
    }
}

/// Create a sample course with modules and quizzes.
pub fn create_sample_course(env: &Env, client: &ProgressTrackerClient) -> Symbol {
    let course_id = Symbol::new(env, "rust_101");
    let mut module_ids = Vec::new(env);
    module_ids.push_back(Symbol::new(env, "mod_basics"));
    module_ids.push_back(Symbol::new(env, "mod_ownership"));
    module_ids.push_back(Symbol::new(env, "mod_traits"));
    client.create_course(&course_id, &3, &2, &module_ids);
    course_id
}

/// Enroll a learner and complete all modules with passing quiz scores.
pub fn complete_full_course(
    env: &Env,
    learner: &Address,
    course_id: &Symbol,
    client: &ProgressTrackerClient,
) {
    client.enroll(learner, course_id);

    client.complete_module(learner, course_id, &Symbol::new(env, "mod_basics"));
    client.complete_module(learner, course_id, &Symbol::new(env, "mod_ownership"));
    client.complete_module(learner, course_id, &Symbol::new(env, "mod_traits"));

    client.submit_quiz_score(learner, course_id, &Symbol::new(env, "quiz_midterm"), &85);
    client.submit_quiz_score(learner, course_id, &Symbol::new(env, "quiz_final"), &75);
}
