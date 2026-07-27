#![no_std]

mod metadata;
mod mint;
mod verify;

use metadata::{CredentialInfo, DataKey};
use soroban_sdk::{contract, contracterror, contractimpl, Address, Env, Symbol, Vec};

/// Subset of the progress-tracker interface used to verify course completion.
#[soroban_sdk::contractclient(name = "ProgressTrackerClient")]
pub trait ProgressTrackerInterface {
    fn is_eligible_for_credential(env: Env, learner: Address, course_id: Symbol) -> bool;
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    AlreadyInitialized = 0,
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
    pub fn initialize(
        env: Env,
        admin: Address,
        progress_tracker: Address,
    ) -> Result<(), ContractError> {
        if env.storage().persistent().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::ProgressTracker, &progress_tracker);
        env.storage()
            .persistent()
            .set(&DataKey::CredentialCounter, &0u64);
        Ok(())
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

    /// Get a page of credential IDs for a learner.
    ///
    /// Reads are paginated so learners holding many credentials do not produce
    /// unbounded responses. Pair with `get_credential_count` to page through the
    /// full list.
    ///
    /// # Arguments
    /// * `learner` - The learner address
    /// * `start` - Zero-based index of the first credential to return
    /// * `limit` - Page size (1..=`MAX_CREDENTIALS_PAGE_SIZE`)
    pub fn get_credentials_for(env: Env, learner: Address, start: u32, limit: u32) -> Vec<u64> {
        verify::get_credentials_for(&env, &learner, start, limit)
    }

    /// Get the total number of credentials a learner holds.
    ///
    /// # Arguments
    /// * `learner` - The learner address
    pub fn get_credential_count(env: Env, learner: Address) -> u32 {
        verify::get_credential_count(&env, &learner)
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

    /// Transfer admin rights to a new address.
    ///
    /// # Arguments
    /// * `new_admin` - The new admin address
    pub fn transfer_admin(env: Env, new_admin: Address) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &new_admin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Events as _},
        vec, Address, IntoVal,
    };

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
        let mut quiz_ids = Vec::new(env);
        quiz_ids.push_back(Symbol::new(env, "quiz_1"));
        tracker_client.create_course(course_id, &2, &1, &module_ids, &quiz_ids);
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

        let cred1 = client.mint_credential(&learner, &course1, &90, &uri);
        let cred2 = client.mint_credential(&learner, &course2, &75, &uri);

        assert_eq!(client.get_credential_count(&learner), 2);

        let creds = client.get_credentials_for(&learner, &0, &10);
        assert_eq!(creds.len(), 2);
        assert_eq!(creds.get(0).unwrap(), cred1);
        assert_eq!(creds.get(1).unwrap(), cred2);
    }

    #[test]
    fn test_get_credentials_for_paginates() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course1 = Symbol::new(&env, "rust_101");
        let course2 = Symbol::new(&env, "sol_201");
        let course3 = Symbol::new(&env, "web3_301");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed(&env, &tracker_id, &learner, &course1);
        enrolled_and_completed(&env, &tracker_id, &learner, &course2);
        enrolled_and_completed(&env, &tracker_id, &learner, &course3);

        let cred1 = client.mint_credential(&learner, &course1, &90, &uri);
        let cred2 = client.mint_credential(&learner, &course2, &75, &uri);
        let cred3 = client.mint_credential(&learner, &course3, &80, &uri);

        // First page.
        let page = client.get_credentials_for(&learner, &0, &2);
        assert_eq!(page.len(), 2);
        assert_eq!(page.get(0).unwrap(), cred1);
        assert_eq!(page.get(1).unwrap(), cred2);

        // Second (partial) page: the limit is clamped to what remains.
        let page = client.get_credentials_for(&learner, &2, &2);
        assert_eq!(page.len(), 1);
        assert_eq!(page.get(0).unwrap(), cred3);

        // Past the end.
        assert_eq!(client.get_credentials_for(&learner, &3, &2).len(), 0);
    }

    #[test]
    fn test_get_credentials_for_unknown_learner_is_empty() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        assert_eq!(client.get_credential_count(&learner), 0);
        assert_eq!(client.get_credentials_for(&learner, &0, &10).len(), 0);
    }

    #[test]
    #[should_panic(expected = "limit must be greater than zero")]
    fn test_get_credentials_for_rejects_zero_limit() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        client.get_credentials_for(&Address::generate(&env), &0, &0);
    }

    #[test]
    #[should_panic(expected = "exceeds maximum page size")]
    fn test_get_credentials_for_rejects_oversized_limit() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        client.get_credentials_for(
            &Address::generate(&env),
            &0,
            &(chainlearn_shared::MAX_CREDENTIALS_PAGE_SIZE + 1),
        );
    }

    #[test]
    #[should_panic(expected = "credential ID counter overflow")]
    fn test_mint_rejects_counter_overflow() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed(&env, &tracker_id, &learner, &course_id);

        // Drive the counter to the point where the next ID would wrap to 0.
        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&DataKey::CredentialCounter, &u64::MAX);
        });

        client.mint_credential(&learner, &course_id, &90, &uri); // should panic
    }

    #[test]
    fn test_minted_event_includes_metadata_uri() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_Qm123");
        enrolled_and_completed(&env, &tracker_id, &learner, &course_id);

        let cred_id = client.mint_credential(&learner, &course_id, &85, &uri);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            vec![&env, last],
            vec![
                &env,
                (
                    contract_id.clone(),
                    (Symbol::new(&env, "credential_minted"),).into_val(&env),
                    (learner, course_id, cred_id, 85u32, uri).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_revoked_event_includes_audit_details() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed(&env, &tracker_id, &learner, &course_id);

        let cred_id = client.mint_credential(&learner, &course_id, &80, &uri);
        client.revoke_credential(&cred_id);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            vec![&env, last],
            vec![
                &env,
                (
                    contract_id.clone(),
                    (Symbol::new(&env, "credential_revoked"),).into_val(&env),
                    (learner, course_id, cred_id, admin).into_val(&env),
                )
            ]
        );
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
