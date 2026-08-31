#![no_std]

mod metadata;
mod mint;
mod verify;
mod xcall;

use chainlearn_shared::ContractMetadata;
use metadata::{CredentialDataKey, CredentialDisplay, CredentialInfo, CredentialVerification};
use soroban_sdk::{contract, contracterror, contractimpl, Address, Env, Symbol, Vec};
use mint::validate_metadata_uri;

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
    /// Returned by `transfer` for every call: credentials are soulbound and
    /// permanently bound to the learner who earned them, so no transfer is
    /// ever permitted, regardless of caller or state (#242).
    Soulbound = 1,
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
        metadata::write_entry(&env, &CredentialDataKey::Admin, &admin);
        metadata::write_entry(&env, &CredentialDataKey::ProgressTracker, &progress_tracker);
        metadata::write_entry(&env, &CredentialDataKey::CredentialCounter, &0u64);
        metadata::write_entry(
            &env,
            &CredentialDataKey::Metadata,
            &ContractMetadata::new(&env, "credential-nft"),
        );
        Ok(())
    }

    /// Returns whether the contract has been initialized (#240).
    ///
    /// Read-only: performs a single storage existence check and never
    /// mutates state. Lets deployment scripts confirm `initialize()` has
    /// already run before calling admin-only setup steps, instead of
    /// discovering an uninitialized contract only when some other call
    /// panics with "not initialized".
    pub fn is_initialized(env: Env) -> bool {
        env.storage().persistent().has(&CredentialDataKey::Admin)
    }

    /// Returns the number of persistent storage entries this contract has
    /// written (#239).
    ///
    /// Maintained as a running counter updated on every persistent write and
    /// removal, since Soroban has no API to enumerate or count a contract's
    /// storage entries at runtime. Read-only and O(1): reads one counter entry.
    pub fn get_storage_size(env: Env) -> u64 {
        metadata::get_storage_size(&env)
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
        Self::require_not_paused(&env);
        let admin: Address = env
            .storage()
            .persistent()
            .get(&CredentialDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        to.require_auth();
        mint::mint_credential(&env, &to, &course_id, score, &metadata_uri)
    }

    /// Set display properties for a credential. Admin only (#244).
    ///
    /// # Arguments
    /// * `credential_id` - The credential to update
    /// * `image_url` - Optional URL of the credential image
    /// * `description` - Optional description
    /// * `issuer_name` - Optional issuer name
    pub fn set_credential_display(
        env: Env,
        credential_id: u64,
        image_url: Option<Symbol>,
        description: Option<Symbol>,
        issuer_name: Option<Symbol>,
    ) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&CredentialDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        // Ensure credential exists
        if !env
            .storage()
            .persistent()
            .has(&CredentialDataKey::Credential(credential_id))
        {
            panic!("credential not found");
        }

        let display = CredentialDisplay {
            image_url,
            description,
            issuer_name,
        };

        env.storage()
            .persistent()
            .set(&CredentialDataKey::Display(credential_id), &display);

        env.events().publish(
            (Symbol::new(&env, "credential_display_set"),),
            (credential_id,),
        );
    }

    /// Get display properties for a credential (#244).
    ///
    /// Returns None if no display properties have been set.
    ///
    /// # Arguments
    /// * `credential_id` - The credential to query
    pub fn get_credential_display(env: Env, credential_id: u64) -> Option<CredentialDisplay> {
        env.storage()
            .persistent()
            .get(&CredentialDataKey::Display(credential_id))
    }

    /// Update the metadata URI for a credential. Admin only (#243).
    ///
    /// # Arguments
    /// * `credential_id` - The credential to update
    /// * `new_metadata_uri` - The new metadata URI
    pub fn update_credential_metadata(env: Env, credential_id: u64, new_metadata_uri: Symbol) {
        Self::require_not_paused(&env);
        let admin: Address = env
            .storage()
            .persistent()
            .get(&CredentialDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        let mut info: CredentialInfo = env
            .storage()
            .persistent()
            .get(&CredentialDataKey::Credential(credential_id))
            .expect("credential not found");

        // Validate the new URI using the same rules as mint.
        validate_metadata_uri(&env, &new_metadata_uri);

        info.metadata_uri = new_metadata_uri.clone();
        env.storage()
            .persistent()
            .set(&CredentialDataKey::Credential(credential_id), &info);

        env.events().publish(
            (Symbol::new(&env, "credential_metadata_updated"),),
            (credential_id, new_metadata_uri),
        );
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

    /// Verify a credential and return its full info along with optional display properties (#244).
    ///
    /// # Arguments
    /// * `credential_id` - The credential to verify
    ///
    /// # Returns
    /// A `CredentialVerification` containing the credential info and optional display properties.
    pub fn verify_credential_with_display(env: Env, credential_id: u64) -> CredentialVerification {
        verify::verify_credential_with_display(&env, credential_id)
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
        Self::require_not_paused(&env);
        verify::revoke_credential(&env, credential_id);
    }

    /// Revoke a credential with a reason. Admin only (#194).
    ///
    /// # Arguments
    /// * `credential_id` - The credential to revoke
    /// * `reason` - The reason for revocation
    pub fn revoke_credential_with_reason(env: Env, credential_id: u64, reason: Symbol) {
        Self::require_not_paused(&env);
        verify::revoke_credential_with_reason(&env, credential_id, reason);
    }

    /// Get the reason a credential was revoked (#194).
    ///
    /// # Arguments
    /// * `credential_id` - The credential to query
    ///
    /// # Returns
    /// The revocation reason, or None if the credential has not been revoked.
    pub fn get_revocation_reason(env: Env, credential_id: u64) -> Option<Symbol> {
        verify::get_revocation_reason(&env, credential_id)
    }

    /// Renew a credential's expiration (#193). Admin only.
    ///
    /// # Arguments
    /// * `credential_id` - The credential to renew
    /// * `new_expiry` - The new expiration ledger height (0 = no expiration)
    pub fn renew_credential(env: Env, credential_id: u64, new_expiry: u32) {
        Self::require_not_paused(&env);
        verify::renew_credential(&env, credential_id, new_expiry);
    }

    // ── Emergency Pause (#189) ────────────────────────────────────────────

    fn is_paused(env: &Env) -> bool {
        env.storage()
            .persistent()
            .get(&CredentialDataKey::Paused)
            .unwrap_or(false)
    }

    fn require_not_paused(env: &Env) {
        if Self::is_paused(env) {
            panic!("contract is paused");
        }
    }

    /// Pause all state-changing operations. Admin only.
    pub fn emergency_pause(env: Env) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&CredentialDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        metadata::write_entry(&env, &CredentialDataKey::Paused, &true);
        // Event would ideally be emitted here, but we will omit it for simplicity if it wasn't added to events.rs
    }

    /// Unpause state-changing operations. Admin only.
    pub fn unpause(env: Env) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&CredentialDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        metadata::write_entry(&env, &CredentialDataKey::Paused, &false);
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

        let zero_address = Address::from_string(&soroban_sdk::String::from_str(
            &env,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        ));
        if new_admin == zero_address {
            panic!("cannot transfer admin to zero address");
        }

        env.storage()
            .persistent()
            .set(&CredentialDataKey::Admin, &new_admin);
    }

    /// Reject transfer of a credential.
    ///
    /// Credentials are soulbound (non-transferable) and permanently bound to the
    /// learner who earned them: a credential attests that a specific learner,
    /// and no one else, met a course's completion criteria, so allowing it to
    /// change hands would let it be sold, gifted, or otherwise separated from
    /// the achievement it certifies. This function enforces that policy by
    /// explicitly rejecting every transfer attempt with a typed error rather
    /// than panicking, so callers get a clear, documented reason instead of a
    /// raw host trap, and can handle the rejection programmatically.
    ///
    /// No storage is read or written: the rejection is unconditional and does
    /// not depend on `from`, `to`, `credential_id`, or any on-chain state, so
    /// there is nothing to authorize and no state to leave unchanged.
    ///
    /// # Arguments
    /// * `from` - The current holder (unused; transfer is always rejected)
    /// * `to` - The intended recipient (unused; transfer is always rejected)
    /// * `credential_id` - The credential being transferred (unused; transfer is always rejected)
    ///
    /// # Returns
    /// Always `Err(ContractError::Soulbound)`. Never `Ok`.
    pub fn transfer(
        _env: Env,
        _from: Address,
        _to: Address,
        _credential_id: u64,
    ) -> Result<(), ContractError> {
        Err(ContractError::Soulbound)
    }

    /// Generate a course completion certificate URI for a learner and course (#223).
    ///
    /// The certificate URI is stored on-chain in credential metadata and is unique per learner and course.
    ///
    /// # Arguments
    /// * `learner` - The learner address (must authorize)
    /// * `course_id` - The course identifier
    ///
    /// # Returns
    /// The generated certificate URI symbol.
    pub fn generate_certificate(env: Env, learner: Address, course_id: Symbol) -> Symbol {
        learner.require_auth();

        let cert_key = CredentialDataKey::CertificateURI(learner.clone(), course_id.clone());
        if let Some(uri) = env.storage().persistent().get::<_, Symbol>(&cert_key) {
            return uri;
        }

        // Check if credential exists for learner & course, or verify eligibility
        let dup_key = CredentialDataKey::CourseCredential(learner.clone(), course_id.clone());
        if !env.storage().persistent().has(&dup_key) {
            let progress_tracker: Address = env
                .storage()
                .persistent()
                .get(&CredentialDataKey::ProgressTracker)
                .expect("not initialized");
            // Direct cross-contract calls, tracker address resolved once (#217, #133).
            if !xcall::course_exists(&env, &progress_tracker, &course_id) {
                panic!("course does not exist");
            }
            if !xcall::is_eligible_for_credential(&env, &progress_tracker, &learner, &course_id) {
                panic!("learner has not completed the course requirements");
            }
        }

        let cert_uri = Symbol::new(&env, "cert_uri");
        metadata::write_entry(&env, &cert_key, &cert_uri);

        // If credential already minted, update metadata_uri in CredentialInfo
        if let Some(cred_id) = env.storage().persistent().get::<_, u64>(&dup_key) {
            let cred_key = CredentialDataKey::Credential(cred_id);
            if let Some(mut info) = env
                .storage()
                .persistent()
                .get::<_, CredentialInfo>(&cred_key)
            {
                info.metadata_uri = cert_uri.clone();
                metadata::write_entry(&env, &cred_key, &info);
            }
        }

        env.events().publish(
            (
                Symbol::new(&env, "certificate_generated"),
                learner.clone(),
                course_id.clone(),
            ),
            (cert_uri.clone(),),
        );

        cert_uri
    }

    /// Query the generated certificate URI for a learner and course (#223).
    pub fn get_certificate_uri(env: Env, learner: Address, course_id: Symbol) -> Option<Symbol> {
        let cert_key = CredentialDataKey::CertificateURI(learner, course_id);
        env.storage().persistent().get(&cert_key)
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

    // ── Issue #223: certificate generation tests ──────────────────────────────

    #[test]
    fn test_generate_certificate() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course = Symbol::new(&env, "rust_101");

        enrolled_and_completed(&env, &tracker_id, &learner, &course);

        assert_eq!(client.get_certificate_uri(&learner, &course), None);

        let cert_uri = client.generate_certificate(&learner, &course);
        assert_eq!(
            client.get_certificate_uri(&learner, &course),
            Some(cert_uri.clone())
        );

        // Mint credential and check that metadata_uri gets updated with generated certificate URI
        let cred_id = client.mint_credential(&learner, &course, &85, &cert_uri);
        let info = client.verify_credential(&cred_id);
        assert_eq!(info.metadata_uri, cert_uri);
    }

    // ── Metadata URI Validation Tests ───────────────────────────────────────────

    #[test]
    #[should_panic(expected = "metadata_uri cannot be empty")]
    fn test_mint_rejects_empty_metadata_uri() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 85);

        let empty_uri = Symbol::new(&env, "");
        client.mint_credential(&learner, &course, &85, &empty_uri);
    }

    #[test]
    #[should_panic(expected = "metadata_uri too short: minimum length is 8")]
    fn test_mint_rejects_too_short_metadata_uri() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 85);

        let short_uri = Symbol::new(&env, "ipfs_1");
        client.mint_credential(&learner, &course, &85, &short_uri);
    }

    #[test]
    #[should_panic(expected = "metadata_uri is malformed: must start with a valid URI scheme")]
    fn test_mint_rejects_malformed_metadata_uri() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 85);

        let invalid_uri = Symbol::new(&env, "ftp_metadata_hash");
        client.mint_credential(&learner, &course, &85, &invalid_uri);
    }

    #[test]
    fn test_mint_accepts_valid_schemes() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 85);

        let uri = Symbol::new(&env, "ipfs_hash12345");
        let id = client.mint_credential(&learner, &course, &85, &uri);
        assert_eq!(id, 1);
        let info = client.verify_credential(&id);
        assert_eq!(info.metadata_uri, uri);
    }
}
