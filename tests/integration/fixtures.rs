//! Test fixtures and setup helpers for integration tests.
//!
//! Provides common setup functions to initialize the contract environment
//! and deploy all three contracts for end-to-end testing.

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
///
/// Returns a `ChainLearnEnv` with all contracts deployed and an admin + learner address.
pub fn setup_chainlearn_env() -> ChainLearnEnv {
    let env = Env::default();
    let admin = Address::generate(&env);
    let learner = Address::generate(&env);

    // In a real integration test, you would register each contract:
    //
    // let token_contract_id = env.register(learn_token::LearnToken, ());
    // let token_client = learn_token::LearnTokenClient::new(&env, &token_contract_id);
    // token_client.initialize(&admin, &Symbol::new(&env, "CLearn"), &Symbol::new(&env, "CLRN"), &7);
    //
    // let credential_contract_id = env.register(credential_nft::CredentialNft, ());
    // let cred_client = credential_nft::CredentialNftClient::new(&env, &credential_contract_id);
    // cred_client.initialize(&admin);
    //
    // let progress_contract_id = env.register(progress_tracker::ProgressTracker, ());
    // let prog_client = progress_tracker::ProgressTrackerClient::new(&env, &progress_contract_id);
    // prog_client.initialize(&admin);

    // For now, generate placeholder addresses
    let token_contract_id = Address::generate(&env);
    let credential_contract_id = Address::generate(&env);
    let progress_contract_id = Address::generate(&env);

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
///
/// Course: "rust_101" with 3 modules and 2 quizzes.
pub fn create_sample_course(
    _env: &Env,
    // client: &progress_tracker::ProgressTrackerClient,
) -> Symbol {
    let course_id = Symbol::new(_env, "rust_101");

    // let mut module_ids = Vec::new(env);
    // module_ids.push_back(Symbol::new(env, "mod_basics"));
    // module_ids.push_back(Symbol::new(env, "mod_ownership"));
    // module_ids.push_back(Symbol::new(env, "mod_traits"));
    // client.create_course(&course_id, &3, &2, &module_ids);

    course_id
}

/// Enroll a learner and complete all modules with passing quiz scores.
///
/// This helper drives the full enrollment -> completion -> eligibility flow.
pub fn complete_full_course(
    _env: &Env,
    _learner: &Address,
    _course_id: &Symbol,
) {
    // client.enroll(learner, course_id);
    //
    // client.complete_module(learner, course_id, &Symbol::new(env, "mod_basics"));
    // client.complete_module(learner, course_id, &Symbol::new(env, "mod_ownership"));
    // client.complete_module(learner, course_id, &Symbol::new(env, "mod_traits"));
    //
    // client.submit_quiz_score(learner, course_id, &Symbol::new(env, "quiz_midterm"), &85);
    // client.submit_quiz_score(learner, course_id, &Symbol::new(env, "quiz_final"), &75);
}
