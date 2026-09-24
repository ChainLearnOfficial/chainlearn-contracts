//! Example Rust client using the soroban-sdk to interact with the ChainLearn progress-tracker contract.
#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, IntoVal};

#[contract]
pub struct ChainLearnClient;

#[contractimpl]
impl ChainLearnClient {
    /// Enrolls a learner into a course via the progress-tracker contract.
    pub fn enroll_learner(env: Env, tracker_id: Address, learner: Address, course_id: Symbol) {
        // Authorize the learner
        learner.require_auth();
        
        // Invoke the enroll function on the progress-tracker contract
        let mut args = soroban_sdk::vec![&env, learner.into_val(&env), course_id.into_val(&env)];
        env.invoke_contract::<()>(&tracker_id, &Symbol::new(&env, "enroll"), args);
    }
}
