#![no_std]

mod metadata;
mod mint;
mod verify;

use metadata::{CredentialInfo, DataKey};
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec};

/// Subset of the progress-tracker interface used to verify course completion.
#[soroban_sdk::contractclient(name = "ProgressTrackerClient")]
pub trait ProgressTrackerInterface {
    fn is_eligible_for_credential(env: Env, learner: Address, course_id: Symbol) -> bool;
}

/// NFT credential contract for ChainLearn course certificates.
///
/// Mints non-transferable credential NFTs to learners who complete courses
/// with a passing score. Each credential is unique and verifiable on-chain.
#[contract]
pub struct CredentialNft;

#[contractimpl]
impl CredentialNft {
    /// Initialize the credential contract with an admin.
    ///
    /// # Arguments
    /// * `admin` - Address that can revoke credentials
    /// * `progress_tracker` - Address of the progress-tracker contract used to
    ///   verify course completion before minting
    pub fn initialize(env: Env, admin: Address, progress_tracker: Address) {
        if env.storage().persistent().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::ProgressTracker, &progress_tracker);
        env.storage()
            .persistent()
            .set(&DataKey::CredentialCounter, &0u64);
    }

    /// Mint a new credential NFT.
    ///
    /// The learner's course completion is verified against the progress-tracker
    /// contract before anything is minted.
    ///
    /// # Arguments
    /// * `to` - Learner receiving the credential (must authorize)
    /// * `course_id` - Course identifier
    /// * `score` - Final score (must be >= 50)
    /// * `metadata_uri` - URI to off-chain metadata
    ///
    /// # Returns
    /// The unique credential ID.
    pub fn mint_credential(
        env: Env,
        to: Address,
        course_id: Symbol,
        score: u32,
        metadata_uri: Symbol,
    ) -> u64 {
        to.require_auth();
        mint::mint_credential(&env, &to, &course_id, score, &metadata_uri)
    }

    /// Verify a credential and return its info.
    ///
    /// # Arguments
    /// * `credential_id` - The credential to verify
    pub fn verify_credential(env: Env, credential_id: u64) -> CredentialInfo {
        verify::verify_credential(&env, credential_id)
    }

    /// Get all credential IDs for a learner.
    ///
    /// # Arguments
    /// * `learner` - The learner address
    pub fn get_credentials_for(env: Env, learner: Address) -> Vec<u64> {
        verify::get_credentials_for(&env, &learner)
    }

    /// Check if a credential is valid (exists and not revoked).
    ///
    /// # Arguments
    /// * `credential_id` - The credential to check
    pub fn is_credential_valid(env: Env, credential_id: u64) -> bool {
        verify::is_credential_valid(&env, credential_id)
    }

    /// Revoke a credential. Admin only.
    ///
    /// # Arguments
    /// * `credential_id` - The credential to revoke
    pub fn revoke_credential(env: Env, credential_id: u64) {
        verify::revoke_credential(&env, credential_id);
    }

    /// Returns the admin address.
    pub fn admin(env: Env) -> Address {
        env.storage()
            .persistent()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }

    /// Returns the progress-tracker contract address.
    pub fn progress_tracker(env: Env) -> Address {
        env.storage()
            .persistent()
            .get(&DataKey::ProgressTracker)
            .expect("not initialized")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address};

    /// Register both contracts and return `(admin, credential_id, tracker_id)`.
    fn setup_contract(env: &Env) -> (Address, Address, Address) {
        let admin = Address::generate(env);

        let tracker_id = env.register_contract(None, progress_tracker::ProgressTracker);
        let tracker_client = progress_tracker::ProgressTrackerClient::new(env, &tracker_id);
        tracker_client.initialize(&admin);

        let contract_id = env.register_contract(None, CredentialNft);
        let client = CredentialNftClient::new(env, &contract_id);
        client.initialize(&admin, &tracker_id);

        (admin, contract_id, tracker_id)
    }

    /// Create a course with two modules and one quiz.
    fn create_course(env: &Env, tracker_id: &Address, course_id: &Symbol) {
        let tracker_client = progress_tracker::ProgressTrackerClient::new(env, tracker_id);
        let mut module_ids = Vec::new(env);
        module_ids.push_back(Symbol::new(env, "mod_1"));
        module_ids.push_back(Symbol::new(env, "mod_2"));
        tracker_client.create_course(course_id, &2, &1, &module_ids);
    }

    /// Enroll the learner and finish every module and quiz in the course.
    fn complete_course(env: &Env, tracker_id: &Address, learner: &Address, course_id: &Symbol) {
        let tracker_client = progress_tracker::ProgressTrackerClient::new(env, tracker_id);
        tracker_client.enroll(learner, course_id);
        tracker_client.complete_module(learner, course_id, &Symbol::new(env, "mod_1"));
        tracker_client.complete_module(learner, course_id, &Symbol::new(env, "mod_2"));
        tracker_client.submit_quiz_score(learner, course_id, &Symbol::new(env, "quiz_1"), &85);
    }

    /// Create a course and take a learner all the way through it.
    fn enrolled_and_completed(
        env: &Env,
        tracker_id: &Address,
        learner: &Address,
        course_id: &Symbol,
    ) {
        create_course(env, tracker_id, course_id);
        complete_course(env, tracker_id, learner, course_id);
    }

    #[test]
    fn test_mint_and_verify() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let metadata_uri = Symbol::new(&env, "ipfs_Qm123");
        enrolled_and_completed(&env, &tracker_id, &learner, &course_id);

        let cred_id = client.mint_credential(&learner, &course_id, &85, &metadata_uri);
        assert_eq!(cred_id, 1);

        let info = client.verify_credential(&cred_id);
        assert_eq!(info.learner, learner);
        assert_eq!(info.course_id, course_id);
        assert_eq!(info.score, 85);
        assert!(!info.revoked);
    }

    #[test]
    fn test_get_credentials_for() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course1 = Symbol::new(&env, "rust_101");
        let course2 = Symbol::new(&env, "sol_201");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed(&env, &tracker_id, &learner, &course1);
        enrolled_and_completed(&env, &tracker_id, &learner, &course2);

        client.mint_credential(&learner, &course1, &90, &uri);
        client.mint_credential(&learner, &course2, &75, &uri);

        let creds = client.get_credentials_for(&learner);
        assert_eq!(creds.len(), 2);
    }

    #[test]
    #[should_panic(expected = "score 40 below minimum threshold 50")]
    fn test_mint_rejects_low_score() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed(&env, &tracker_id, &learner, &course_id);

        client.mint_credential(&learner, &course_id, &40, &uri);
    }

    #[test]
    #[should_panic(expected = "credential already exists for this learner and course")]
    fn test_mint_prevents_duplicates() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed(&env, &tracker_id, &learner, &course_id);

        client.mint_credential(&learner, &course_id, &90, &uri);
        client.mint_credential(&learner, &course_id, &95, &uri); // should panic
    }

    #[test]
    #[should_panic(expected = "learner has not completed the course requirements")]
    fn test_mint_rejects_ineligible_learner() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let tracker_client = progress_tracker::ProgressTrackerClient::new(&env, &tracker_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");

        // Enrolled, but no modules completed and no quizzes submitted.
        create_course(&env, &tracker_id, &course_id);
        tracker_client.enroll(&learner, &course_id);

        client.mint_credential(&learner, &course_id, &100, &uri); // should panic
    }

    #[test]
    #[should_panic(expected = "learner has not completed the course requirements")]
    fn test_mint_rejects_incomplete_modules() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let tracker_client = progress_tracker::ProgressTrackerClient::new(&env, &tracker_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");

        // Only one of the two modules completed, quiz passed.
        create_course(&env, &tracker_id, &course_id);
        tracker_client.enroll(&learner, &course_id);
        tracker_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        tracker_client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &95);

        client.mint_credential(&learner, &course_id, &95, &uri); // should panic
    }

    #[test]
    #[should_panic(expected = "learner has not completed the course requirements")]
    fn test_mint_rejects_missing_quiz() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let tracker_client = progress_tracker::ProgressTrackerClient::new(&env, &tracker_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");

        // All modules done, but the quiz was never submitted.
        create_course(&env, &tracker_id, &course_id);
        tracker_client.enroll(&learner, &course_id);
        tracker_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        tracker_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));

        client.mint_credential(&learner, &course_id, &90, &uri); // should panic
    }

    #[test]
    fn test_revoke_credential() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed(&env, &tracker_id, &learner, &course_id);

        let cred_id = client.mint_credential(&learner, &course_id, &80, &uri);
        assert!(client.is_credential_valid(&cred_id));

        client.revoke_credential(&cred_id);
        assert!(!client.is_credential_valid(&cred_id));

        let info = client.verify_credential(&cred_id);
        assert!(info.revoked);
    }
}
