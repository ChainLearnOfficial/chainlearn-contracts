#![no_std]

mod metadata;
mod mint;
mod verify;

use chainlearn_shared::ContractMetadata;
use metadata::{CredentialDataKey, CredentialInfo};
use soroban_sdk::{contract, contracterror, contractimpl, Address, Env, Symbol, Vec};

/// Subset of the progress-tracker interface used to verify course completion
/// and the score a credential claims.
#[soroban_sdk::contractclient(name = "ProgressTrackerClient")]
pub trait ProgressTrackerInterface {
    fn course_exists(env: Env, course_id: Symbol) -> bool;
    fn is_eligible_for_credential(env: Env, learner: Address, course_id: Symbol) -> bool;
    fn get_course_score(env: Env, learner: Address, course_id: Symbol) -> u32;
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
        if env.storage().persistent().has(&CredentialDataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }
        env.storage()
            .persistent()
            .set(&CredentialDataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&CredentialDataKey::ProgressTracker, &progress_tracker);
        env.storage()
            .persistent()
            .set(&CredentialDataKey::CredentialCounter, &0u64);
        env.storage().persistent().set(
            &CredentialDataKey::Metadata,
            &ContractMetadata::new(&env, "credential-nft"),
        );
        Ok(())
    }

    /// Get the contract's on-chain name and version (#107).
    ///
    /// Lets external tools (indexers, block explorers, upgrade tooling)
    /// identify which contract and release is deployed without inferring it
    /// from behavior.
    pub fn contract_metadata(env: Env) -> ContractMetadata {
        env.storage()
            .persistent()
            .get(&CredentialDataKey::Metadata)
            .expect("not initialized")
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
    ///
    /// # Examples
    ///
    /// ```ignore
    /// env.mock_all_auths();
    /// let learner = Address::generate(&env);
    /// let course_id = Symbol::new(&env, "rust_101");
    /// let metadata_uri = Symbol::new(&env, "ipfs_Qm123");
    ///
    /// // Set up and complete a course in progress-tracker
    /// let mut modules = Vec::new(&env);
    /// modules.push_back(Symbol::new(&env, "mod_1"));
    /// let mut quizzes = Vec::new(&env);
    /// quizzes.push_back(Symbol::new(&env, "quiz_1"));
    ///
    /// tracker_client.create_course(&course_id, &1, &1, &modules, &quizzes);
    /// tracker_client.enroll(&learner, &course_id);
    /// tracker_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
    /// tracker_client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &85);
    ///
    /// // Mint the credential
    /// let cred_id = client.mint_credential(&learner, &course_id, &85, &metadata_uri);
    /// assert_eq!(cred_id, 1);
    /// ```
    pub fn mint_credential(
        env: Env,
        to: Address,
        course_id: Symbol,
        score: u32,
        metadata_uri: Symbol,
    ) -> u64 {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&CredentialDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        to.require_auth();
        mint::mint_credential(&env, &to, &course_id, score, &metadata_uri)
    }

    /// Verify a credential and return its info.
    ///
    /// # Arguments
    /// * `credential_id` - The credential to verify
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let learner = Address::generate(&env);
    /// let course_id = Symbol::new(&env, "rust_101");
    /// let uri = Symbol::new(&env, "ipfs_meta");
    /// env.mock_all_auths();
    ///
    /// // After completing course and minting credential...
    /// let cred_id = client.mint_credential(&learner, &course_id, &85, &uri);
    ///
    /// let info = client.verify_credential(&cred_id);
    /// assert_eq!(info.learner, learner);
    /// assert_eq!(info.course_id, course_id);
    /// assert_eq!(info.score, 85);
    /// assert!(!info.revoked);
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let learner = Address::generate(&env);
    /// let course_id = Symbol::new(&env, "rust_101");
    /// let uri = Symbol::new(&env, "ipfs_meta");
    /// env.mock_all_auths();
    ///
    /// // Complete course and mint credential...
    /// client.mint_credential(&learner, &course_id, &85, &uri);
    ///
    /// let count = client.get_credential_count(&learner);
    /// assert_eq!(count, 1);
    ///
    /// let credentials = client.get_credentials_for(&learner, &0, &10);
    /// assert_eq!(credentials.len(), 1);
    /// ```
    pub fn get_credentials_for(env: Env, learner: Address, start: u32, limit: u32) -> Vec<u64> {
        verify::get_credentials_for(&env, &learner, start, limit)
    }

    /// Get the total number of credentials a learner holds.
    ///
    /// # Arguments
    /// * `learner` - The learner address
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let learner = Address::generate(&env);
    /// assert_eq!(client.get_credential_count(&learner), 0);
    ///
    /// // After completing a course and minting...
    /// client.mint_credential(&learner, &course_id, &85, &uri);
    /// assert_eq!(client.get_credential_count(&learner), 1);
    /// ```
    pub fn get_credential_count(env: Env, learner: Address) -> u32 {
        verify::get_credential_count(&env, &learner)
    }

    /// Get the total number of credentials issued across all learners (#103).
    ///
    /// This is the value of the credential ID counter, which increments on
    /// every successful mint and is never decremented.
    pub fn get_total_credentials_count(env: Env) -> u64 {
        env.storage()
            .persistent()
            .get(&CredentialDataKey::CredentialCounter)
            .unwrap_or(0)
    }

    /// Get all credential IDs for a given course (reverse lookup).
    ///
    /// # Arguments
    /// * `course_id` - The course identifier to look up
    ///
    /// # Returns
    /// A vector of credential IDs issued for this course.
    pub fn get_credentials_by_course(env: Env, course_id: Symbol) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&CredentialDataKey::CourseCredentials(course_id))
            .unwrap_or(Vec::new(&env))
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
            .get(&CredentialDataKey::Admin)
            .expect("not initialized")
    }

    /// Returns the progress-tracker contract address.
    pub fn progress_tracker(env: Env) -> Address {
        env.storage()
            .persistent()
            .get(&CredentialDataKey::ProgressTracker)
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
            .get(&CredentialDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&CredentialDataKey::Admin, &new_admin);
    }

    /// Reject transfer of a credential.
    ///
    /// Credentials are soulbound (non-transferable) and permanently bound to the
    /// learner who earned them. This function enforces that policy by rejecting
    /// all transfer attempts.
    ///
    /// # Arguments
    /// * `from` - The current holder (must authorize)
    /// * `to` - The intended recipient (not used, transfer rejected)
    /// * `credential_id` - The credential being transferred (not used, transfer rejected)
    ///
    /// # Panics
    /// Always panics with a message explaining credentials are non-transferable.
    pub fn transfer(_env: Env, from: Address, _to: Address, _credential_id: u64) {
        from.require_auth();
        panic!("credentials are soulbound and non-transferable");
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

    /// Enroll the learner and finish every module and quiz in the course,
    /// recording `score` on the single quiz.
    fn complete_course_with_score(
        env: &Env,
        tracker_id: &Address,
        learner: &Address,
        course_id: &Symbol,
        score: u32,
    ) {
        let tracker_client = progress_tracker::ProgressTrackerClient::new(env, tracker_id);
        tracker_client.enroll(learner, course_id);
        tracker_client.complete_module(learner, course_id, &Symbol::new(env, "mod_1"));
        tracker_client.complete_module(learner, course_id, &Symbol::new(env, "mod_2"));
        tracker_client.submit_quiz_score(learner, course_id, &Symbol::new(env, "quiz_1"), &score);
    }

    /// Enroll the learner and finish the course with the default score of 85.
    fn complete_course(env: &Env, tracker_id: &Address, learner: &Address, course_id: &Symbol) {
        complete_course_with_score(env, tracker_id, learner, course_id, 85);
    }

    /// Create a course and take a learner all the way through it, recording
    /// `score`. Minting must use the same score, since the credential contract
    /// verifies it against the tracker (#34).
    fn enrolled_and_completed_with_score(
        env: &Env,
        tracker_id: &Address,
        learner: &Address,
        course_id: &Symbol,
        score: u32,
    ) {
        create_course(env, tracker_id, course_id);
        complete_course_with_score(env, tracker_id, learner, course_id, score);
    }

    /// Create a course and take a learner all the way through it at score 85.
    fn enrolled_and_completed(
        env: &Env,
        tracker_id: &Address,
        learner: &Address,
        course_id: &Symbol,
    ) {
        create_course(env, tracker_id, course_id);
        complete_course(env, tracker_id, learner, course_id);
    }

    // ── Issue #107: initialize() stores contract name/version metadata ──────

    #[test]
    fn test_initialize_stores_contract_metadata() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let metadata = client.contract_metadata();
        assert_eq!(
            metadata.name,
            soroban_sdk::String::from_str(&env, "credential-nft")
        );
        assert_eq!(
            metadata.version,
            soroban_sdk::String::from_str(&env, chainlearn_shared::CONTRACT_VERSION)
        );
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
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course1, 90);
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course2, 75);

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
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course1, 90);
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course2, 75);
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course3, 80);

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

    // ── Issue #34: score verified on-chain against the progress-tracker ───

    #[test]
    #[should_panic(expected = "score 100 does not match verified score 50")]
    fn test_mint_rejects_inflated_score() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        // The learner actually scored 50.
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 50);

        // Claiming 100 must be rejected, not silently recorded.
        client.mint_credential(&learner, &course_id, &100, &uri);
    }

    #[test]
    #[should_panic(expected = "does not match verified score")]
    fn test_mint_rejects_understated_score() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 90);

        // A credential must record what the tracker recorded, in either direction.
        client.mint_credential(&learner, &course_id, &80, &uri);
    }

    #[test]
    fn test_mint_accepts_verified_score() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 72);

        let cred_id = client.mint_credential(&learner, &course_id, &72, &uri);
        assert_eq!(client.verify_credential(&cred_id).score, 72);
    }

    #[test]
    fn test_mint_verifies_against_average_of_all_quizzes() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let tracker = progress_tracker::ProgressTrackerClient::new(&env, &tracker_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "multi_quiz");
        let uri = Symbol::new(&env, "ipfs_meta");

        // A two-quiz course: 80 and 90 average to 85.
        let mut module_ids = Vec::new(&env);
        module_ids.push_back(Symbol::new(&env, "mod_1"));
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(Symbol::new(&env, "quiz_1"));
        quiz_ids.push_back(Symbol::new(&env, "quiz_2"));
        tracker.create_course(&course_id, &1, &2, &module_ids, &quiz_ids);

        tracker.enroll(&learner, &course_id);
        tracker.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        tracker.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);
        tracker.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &90);

        assert_eq!(tracker.get_course_score(&learner, &course_id), 85);

        let cred_id = client.mint_credential(&learner, &course_id, &85, &uri);
        assert_eq!(client.verify_credential(&cred_id).score, 85);
    }

    #[test]
    #[should_panic(expected = "does not match verified score")]
    fn test_mint_rejects_score_from_a_different_learner() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let strong_learner = Address::generate(&env);
        let weak_learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &strong_learner, &course_id, 95);
        complete_course_with_score(&env, &tracker_id, &weak_learner, &course_id, 60);

        // Borrowing the strong learner's score for the weak learner must fail.
        client.mint_credential(&weak_learner, &course_id, &95, &uri);
    }

    // ── Issue #105: reverse lookup from course_id to credentials ──────────

    #[test]
    fn test_get_credentials_by_course_returns_minted() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner1 = Address::generate(&env);
        let learner2 = Address::generate(&env);
        env.mock_all_auths();

        let course = Symbol::new(&env, "rust_101");
        let other_course = Symbol::new(&env, "web3_202");
        // Symbols cannot contain ':' or '/', so the fixture uses a symbol-safe
        // stand-in for the metadata URI.
        let uri = Symbol::new(&env, "ipfs_meta");

        enrolled_and_completed_with_score(&env, &tracker_id, &learner1, &course, 80);
        complete_course_with_score(&env, &tracker_id, &learner2, &course, 90);
        enrolled_and_completed_with_score(&env, &tracker_id, &learner1, &other_course, 70);

        let id1 = client.mint_credential(&learner1, &course, &80, &uri);
        let id2 = client.mint_credential(&learner2, &course, &90, &uri);
        client.mint_credential(&learner1, &other_course, &70, &uri);

        let course_creds = client.get_credentials_by_course(&course);
        assert_eq!(course_creds.len(), 2);
        assert!(course_creds.contains(id1));
        assert!(course_creds.contains(id2));
    }

    #[test]
    fn test_get_credentials_by_course_unknown_returns_empty() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let unknown_course = Symbol::new(&env, "nonexistent");
        let creds = client.get_credentials_by_course(&unknown_course);
        assert_eq!(creds.len(), 0);
    }

    #[test]
    fn test_get_credentials_by_course_multiple_courses() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_a = Symbol::new(&env, "course_a");
        let course_b = Symbol::new(&env, "course_b");
        // Symbols cannot contain ':' or '/', so the fixture uses a symbol-safe
        // stand-in for the metadata URI.
        let uri = Symbol::new(&env, "ipfs_meta");

        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_a, 85);
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_b, 75);

        let id_a = client.mint_credential(&learner, &course_a, &85, &uri);
        let id_b = client.mint_credential(&learner, &course_b, &75, &uri);

        let a_creds = client.get_credentials_by_course(&course_a);
        assert_eq!(a_creds.len(), 1);
        assert_eq!(a_creds.get(0).unwrap(), id_a);

        let b_creds = client.get_credentials_by_course(&course_b);
        assert_eq!(b_creds.len(), 1);
        assert_eq!(b_creds.get(0).unwrap(), id_b);
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
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 90);

        // Drive the counter to the point where the next ID would wrap to 0.
        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&CredentialDataKey::CredentialCounter, &u64::MAX);
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
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 80);

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
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 90);

        client.mint_credential(&learner, &course_id, &90, &uri);
        client.mint_credential(&learner, &course_id, &90, &uri); // should panic
    }

    #[test]
    #[should_panic(expected = "course does not exist")]
    fn test_mint_rejects_unknown_course() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        // No course was ever created via `create_course` on the tracker.
        let course_id = Symbol::new(&env, "ghost_course");
        let uri = Symbol::new(&env, "ipfs_meta");

        client.mint_credential(&learner, &course_id, &90, &uri); // should panic
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
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 80);

        let cred_id = client.mint_credential(&learner, &course_id, &80, &uri);
        assert!(client.is_credential_valid(&cred_id));

        client.revoke_credential(&cred_id);
        assert!(!client.is_credential_valid(&cred_id));

        let info = client.verify_credential(&cred_id);
        assert!(info.revoked);
    }

    // ── Issue #109: is_credential_valid does not deserialize CredentialInfo ──

    #[test]
    fn test_is_credential_valid_false_for_unminted_id() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        assert!(!client.is_credential_valid(&999));
    }

    #[test]
    fn test_revoke_sets_dedicated_revoked_flag() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 80);

        let cred_id = client.mint_credential(&learner, &course_id, &80, &uri);
        client.revoke_credential(&cred_id);

        env.as_contract(&contract_id, || {
            assert_eq!(
                env.storage()
                    .persistent()
                    .get::<_, bool>(&crate::metadata::CredentialDataKey::Revoked(cred_id)),
                Some(true)
            );
        });
    }

    // ── Issue #103: public total credentials count ──────────────────────────

    #[test]
    fn test_get_total_credentials_count_returns_counter() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        assert_eq!(client.get_total_credentials_count(), 0);

        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course = Symbol::new(&env, "rust_101");
        // Symbols cannot contain ':' or '/', so the fixture uses a symbol-safe
        // stand-in for the metadata URI.
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 80);

        client.mint_credential(&learner, &course, &80, &uri);
        assert_eq!(client.get_total_credentials_count(), 1);

        let learner2 = Address::generate(&env);
        complete_course_with_score(&env, &tracker_id, &learner2, &course, 90);
        client.mint_credential(&learner2, &course, &90, &uri);
        assert_eq!(client.get_total_credentials_count(), 2);
    }

    // ── Issue #104: learner credentials vec pruned on revoke ─────────────────

    #[test]
    fn test_revoke_prunes_learner_credentials_list() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course = Symbol::new(&env, "rust_101");
        // Symbols cannot contain ':' or '/', so the fixture uses a symbol-safe
        // stand-in for the metadata URI.
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 80);

        let cred_id = client.mint_credential(&learner, &course, &80, &uri);

        // Before revoke, the credential appears in the learner's list.
        let before = client.get_credentials_for(&learner, &0, &10);
        assert_eq!(before.len(), 1);
        assert_eq!(before.get(0).unwrap(), cred_id);

        client.revoke_credential(&cred_id);

        // After revoke, the credential is pruned from the learner's list.
        let after = client.get_credentials_for(&learner, &0, &10);
        assert_eq!(after.len(), 0);
    }

    #[test]
    fn test_revoke_prunes_course_credentials_index() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course = Symbol::new(&env, "rust_101");
        // Symbols cannot contain ':' or '/', so the fixture uses a symbol-safe
        // stand-in for the metadata URI.
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 80);

        let cred_id = client.mint_credential(&learner, &course, &80, &uri);

        let before = client.get_credentials_by_course(&course);
        assert_eq!(before.len(), 1);

        client.revoke_credential(&cred_id);

        let after = client.get_credentials_by_course(&course);
        assert_eq!(after.len(), 0);
    }
}
