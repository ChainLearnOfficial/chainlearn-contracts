#![no_std]

mod events;
mod metadata;
mod mint;
mod verify;
mod xcall;

use chainlearn_shared::ContractMetadata;
use metadata::AdminRole;
use metadata::{
    remove_entry, write_entry, CredentialDataKey, CredentialDisplay, CredentialInfo,
    CredentialVerification,
};
use mint::validate_metadata_uri;
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{contract, contracterror, contractimpl, Address, BytesN, Env, Symbol, Vec};

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
    /// ever permitted, regardless of caller or state (#242, duplicate: #227).
    Soulbound = 1,
    /// Caller does not have the required role (#551).
    NotAuthorized = 2,
    /// Contract is paused; state-changing operations are blocked (#553).
    ContractPaused = 3,
    /// Requested credential or resource was not found (#553).
    NotFound = 4,
    /// A credential already exists for this learner and course (#553).
    DuplicateCredential = 5,
    /// Score is below the minimum threshold or does not match the verified score (#553).
    InvalidScore = 6,
    /// Metadata URI is empty, too short, or has an invalid scheme (#553).
    InvalidMetadataUri = 7,
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
        // Grant Admin role to the initial admin (#551).
        metadata::grant_role(&env, &admin, &AdminRole::Admin);
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

    // ── Role-Based Access Control (#551) ──────────────────────────────────────

    /// Grant a role to an address. Admin only.
    ///
    /// # Arguments
    /// * `caller` - The admin granting the role (must authorize)
    /// * `role` - The role to grant
    /// * `address` - The address to receive the role
    pub fn grant_role(
        env: Env,
        caller: Address,
        role: AdminRole,
        address: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        if !metadata::has_role(&env, &caller, &AdminRole::Admin) {
            return Err(ContractError::NotAuthorized);
        }
        metadata::grant_role(&env, &address, &role);
        events::role_granted(&env, &address, &role);
        Ok(())
    }

    /// Revoke a role from an address. Admin only.
    ///
    /// # Arguments
    /// * `caller` - The admin revoking the role (must authorize)
    /// * `role` - The role to revoke
    /// * `address` - The address to lose the role
    pub fn revoke_role(
        env: Env,
        caller: Address,
        role: AdminRole,
        address: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        if !metadata::has_role(&env, &caller, &AdminRole::Admin) {
            return Err(ContractError::NotAuthorized);
        }
        metadata::revoke_role(&env, &address, &role);
        events::role_revoked(&env, &address, &role);
        Ok(())
    }

    /// Check whether an address has a specific role.
    ///
    /// # Arguments
    /// * `role` - The role to check
    /// * `address` - The address to check
    pub fn has_role(env: Env, role: AdminRole, address: Address) -> bool {
        metadata::has_role(&env, &address, &role)
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
    /// let cred_id = client.mint_credential(&admin, &learner, &course_id, &85, &metadata_uri);
    /// assert_eq!(cred_id, 1);
    /// ```
    pub fn mint_credential(
        env: Env,
        caller: Address,
        to: Address,
        course_id: Symbol,
        score: u32,
        metadata_uri: Symbol,
    ) -> Result<u64, ContractError> {
        Self::ensure_not_paused(&env)?;
        caller.require_auth();
        if !metadata::has_role(&env, &caller, &AdminRole::Minter) {
            return Err(ContractError::NotAuthorized);
        }
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
    ) -> Result<(), ContractError> {
        Self::ensure_not_paused(&env)?;
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
            return Err(ContractError::NotFound);
        }

        let display = CredentialDisplay {
            image_url,
            description,
            issuer_name,
        };

        metadata::write_entry(&env, &CredentialDataKey::Display(credential_id), &display);

        events::credential_display_set(
            &env,
            credential_id,
            &display.image_url,
            &display.description,
            &display.issuer_name,
        );
        Ok(())
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
    pub fn update_credential_metadata(
        env: Env,
        credential_id: u64,
        new_metadata_uri: Symbol,
    ) -> Result<(), ContractError> {
        Self::ensure_not_paused(&env)?;
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
            .ok_or(ContractError::NotFound)?;

        // Validate the new URI using the same rules as mint.
        validate_metadata_uri(&env, &new_metadata_uri)?;

        info.metadata_uri = new_metadata_uri.clone();
        write_entry(&env, &CredentialDataKey::Credential(credential_id), &info);

        events::credential_metadata_updated(&env, credential_id, &new_metadata_uri);
        Ok(())
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
    /// let cred_id = client.mint_credential(&admin, &learner, &course_id, &85, &uri);
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
    /// client.mint_credential(&admin, &learner, &course_id, &85, &uri);
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
    /// client.mint_credential(&admin, &learner, &course_id, &85, &uri);
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
    pub fn revoke_credential(env: Env, credential_id: u64) -> Result<(), ContractError> {
        Self::ensure_not_paused(&env)?;
        verify::revoke_credential(&env, credential_id)
    }

    /// Revoke a credential with a reason. Admin only (#194).
    ///
    /// # Arguments
    /// * `credential_id` - The credential to revoke
    /// * `reason` - The reason for revocation
    pub fn revoke_credential_with_reason(
        env: Env,
        credential_id: u64,
        reason: Symbol,
    ) -> Result<(), ContractError> {
        Self::ensure_not_paused(&env)?;
        verify::revoke_credential_with_reason(&env, credential_id, reason)
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
    pub fn renew_credential(
        env: Env,
        credential_id: u64,
        new_expiry: u32,
    ) -> Result<(), ContractError> {
        Self::ensure_not_paused(&env)?;
        verify::renew_credential(&env, credential_id, new_expiry)
    }

    // ── Emergency Pause (#189, #475) ─────────────────────────────────────────

    /// Check if the contract is paused.
    fn is_paused(env: &Env) -> bool {
        env.storage()
            .persistent()
            .get(&CredentialDataKey::Paused)
            .unwrap_or(false)
    }

    /// Check if the contract is paused and return an error if so.
    fn ensure_not_paused(env: &Env) -> Result<(), ContractError> {
        if Self::is_paused(env) {
            return Err(ContractError::ContractPaused);
        }
        Ok(())
    }

    /// Pause all state-changing operations. Admin or Pauser role required.
    pub fn pause(env: Env, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();
        if !metadata::has_role(&env, &caller, &AdminRole::Pauser) {
            return Err(ContractError::NotAuthorized);
        }
        if Self::is_paused(&env) {
            return Err(ContractError::AlreadyInitialized); // already paused
        }
        metadata::write_entry(&env, &CredentialDataKey::Paused, &true);
        events::paused(&env, &caller, env.ledger().timestamp());
        Ok(())
    }

    /// Unpause state-changing operations. Admin or Pauser role required.
    pub fn unpause(env: Env, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();
        if !metadata::has_role(&env, &caller, &AdminRole::Pauser) {
            return Err(ContractError::NotAuthorized);
        }
        if !Self::is_paused(&env) {
            return Err(ContractError::AlreadyInitialized); // not paused
        }
        metadata::write_entry(&env, &CredentialDataKey::Paused, &false);
        events::unpaused(&env, &caller, env.ledger().timestamp());
        Ok(())
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

    /// Initiate a delayed transfer of admin rights to a new address (#423).
    ///
    /// This does **not** change the admin immediately. It records `new_admin`
    /// as pending; the transfer only takes effect once `new_admin` calls
    /// [`Self::accept_admin`] after [`Self::admin_transfer_delay`] has elapsed.
    /// The current admin can call [`Self::cancel_admin_transfer`] any time
    /// before acceptance to abort it.
    ///
    /// # Why a delay
    /// The previous immediate transfer meant a single compromised admin key
    /// could hand control to an attacker-controlled address in one
    /// transaction, with no window to notice or react. This now matches
    /// `learn-token`, which already used the delayed pattern.
    ///
    /// Calling this again before a pending transfer is accepted overwrites it
    /// with the new candidate and restarts the delay from now.
    ///
    /// # Arguments
    /// * `new_admin` - The new admin address
    ///
    /// # Panics
    /// * If the caller is not the current admin
    /// * If `new_admin` is the zero address
    pub fn transfer_admin(env: Env, new_admin: Address) -> Result<(), ContractError> {
        Self::ensure_not_paused(&env)?;
        let admin: Address = env
            .storage()
            .persistent()
            .get(&CredentialDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        let zero_address = chainlearn_shared::zero_address(&env);
        if new_admin == zero_address {
            return Err(ContractError::NotAuthorized); // cannot transfer admin to zero address
        }

        let initiated_at = env.ledger().timestamp();
        let delay = Self::admin_transfer_delay(env.clone());
        metadata::write_entry(
            &env,
            &CredentialDataKey::PendingAdmin,
            &metadata::PendingAdminTransfer {
                new_admin: new_admin.clone(),
                initiated_at,
            },
        );

        events::admin_transfer_initiated(
            &env,
            &admin,
            &new_admin,
            initiated_at,
            initiated_at.saturating_add(delay),
        );
        Ok(())
    }

    /// Complete a pending admin transfer once its delay has elapsed (#423).
    ///
    /// Must be called by the pending `new_admin` address, proving control of
    /// that key before it's granted admin rights.
    ///
    /// # Panics
    /// * If there is no pending admin transfer
    /// * If the caller is not the pending `new_admin`
    /// * If the delay has not yet elapsed
    pub fn accept_admin(env: Env) -> Result<(), ContractError> {
        let pending: metadata::PendingAdminTransfer = env
            .storage()
            .persistent()
            .get(&CredentialDataKey::PendingAdmin)
            .ok_or(ContractError::NotFound)?; // no pending admin transfer
        pending.new_admin.require_auth();

        let delay = Self::admin_transfer_delay(env.clone());
        let ready_at = pending.initiated_at.saturating_add(delay);
        if env.ledger().timestamp() < ready_at {
            return Err(ContractError::AlreadyInitialized); // admin transfer delay has not elapsed
        }

        let previous_admin: Address = env
            .storage()
            .persistent()
            .get(&CredentialDataKey::Admin)
            .expect("not initialized");
        write_entry(&env, &CredentialDataKey::Admin, &pending.new_admin);
        remove_entry(&env, &CredentialDataKey::PendingAdmin);

        events::admin_transfer_accepted(&env, &previous_admin, &pending.new_admin);
        Ok(())
    }

    /// Cancel a pending admin transfer before it is accepted (#423).
    ///
    /// Admin only. The primary safeguard against a compromised admin key: the
    /// legitimate admin can abort an unauthorized `transfer_admin` call any
    /// time before the pending `new_admin` accepts it.
    ///
    /// # Panics
    /// * If the caller is not the current admin
    /// * If there is no pending admin transfer
    pub fn cancel_admin_transfer(env: Env) -> Result<(), ContractError> {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&CredentialDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        let pending: metadata::PendingAdminTransfer = env
            .storage()
            .persistent()
            .get(&CredentialDataKey::PendingAdmin)
            .ok_or(ContractError::NotFound)?; // no pending admin transfer
        remove_entry(&env, &CredentialDataKey::PendingAdmin);

        events::admin_transfer_cancelled(&env, &admin, &pending.new_admin);
        Ok(())
    }

    /// Returns the in-flight pending admin transfer, if any (#423).
    pub fn pending_admin(env: Env) -> Option<metadata::PendingAdminTransfer> {
        env.storage()
            .persistent()
            .get(&CredentialDataKey::PendingAdmin)
    }

    /// Returns the current admin-transfer delay, in seconds (#423).
    ///
    /// Clamped to [`metadata::MIN_ADMIN_TRANSFER_DELAY_SECONDS`] so a value
    /// stored before the minimum was enforced can never shorten the handoff
    /// guard below one hour, matching `learn-token` (#546).
    pub fn admin_transfer_delay(env: Env) -> u64 {
        env.storage()
            .persistent()
            .get(&CredentialDataKey::AdminTransferDelay)
            .unwrap_or(metadata::DEFAULT_ADMIN_TRANSFER_DELAY_SECONDS)
            .max(metadata::MIN_ADMIN_TRANSFER_DELAY_SECONDS)
    }

    /// Set the admin-transfer delay, in seconds. Admin only (#423).
    ///
    /// Applies to transfers initiated after this call; it does not change the
    /// deadline of a transfer already pending.
    ///
    /// # Panics
    /// * If the caller is not the current admin
    /// * If `delay_seconds` is below
    ///   [`metadata::MIN_ADMIN_TRANSFER_DELAY_SECONDS`] (3,600s): a zero or
    ///   near-zero delay would let a compromised admin key hand control to an
    ///   attacker before anyone monitoring the contract could react, which is
    ///   exactly what the two-step transfer exists to prevent (#546)
    pub fn set_admin_transfer_delay(env: Env, delay_seconds: u64) -> Result<(), ContractError> {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&CredentialDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        if delay_seconds < metadata::MIN_ADMIN_TRANSFER_DELAY_SECONDS {
            return Err(ContractError::AlreadyInitialized); // below minimum delay
        }

        write_entry(&env, &CredentialDataKey::AdminTransferDelay, &delay_seconds);
        Ok(())
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
    pub fn generate_certificate(
        env: Env,
        learner: Address,
        course_id: Symbol,
    ) -> Result<Symbol, ContractError> {
        Self::ensure_not_paused(&env)?;
        learner.require_auth();

        let cert_key = CredentialDataKey::CertificateURI(learner.clone(), course_id.clone());
        if let Some(uri) = env.storage().persistent().get::<_, Symbol>(&cert_key) {
            return Ok(uri);
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
                return Err(ContractError::NotFound); // course does not exist
            }
            if !xcall::is_eligible_for_credential(&env, &progress_tracker, &learner, &course_id) {
                return Err(ContractError::NotFound); // learner has not completed the course requirements
            }
        }

        let mut payload = soroban_sdk::Bytes::new(&env);
        payload.append(&learner.clone().to_xdr(&env));
        payload.append(&course_id.clone().to_xdr(&env));
        let hash_bytes = env.crypto().sha256(&payload).to_array();

        const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
        let mut uri_buf = [0u8; 29];
        uri_buf[0..5].copy_from_slice(b"cert_");
        for i in 0..12 {
            let byte = hash_bytes[i];
            uri_buf[5 + i * 2] = HEX_CHARS[(byte >> 4) as usize];
            uri_buf[5 + i * 2 + 1] = HEX_CHARS[(byte & 0x0f) as usize];
        }
        let uri_str = match core::str::from_utf8(&uri_buf) {
            Ok(s) => s,
            Err(_) => return Err(ContractError::NotFound), // failed to format certificate uri
        };
        let cert_uri = Symbol::new(&env, uri_str);
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

        events::certificate_generated(&env, &learner, &course_id, &cert_uri);

        Ok(cert_uri)
    }

    /// Query the generated certificate URI for a learner and course (#223).
    pub fn get_certificate_uri(env: Env, learner: Address, course_id: Symbol) -> Option<Symbol> {
        let cert_key = CredentialDataKey::CertificateURI(learner, course_id);
        env.storage().persistent().get(&cert_key)
    }

    // ── Upgradeability ────────────────────────────────────────────────────────

    /// Upgrade the contract to a new Wasm code.
    ///
    /// # Arguments
    /// * `caller` - The admin performing the upgrade (must authorize, Admin role required)
    /// * `new_wasm_hash` - Hash of the already-uploaded wasm to install
    pub fn upgrade(
        env: Env,
        caller: Address,
        new_wasm_hash: BytesN<32>,
    ) -> Result<(), ContractError> {
        Self::ensure_not_paused(&env)?;
        caller.require_auth();
        if !metadata::has_role(&env, &caller, &AdminRole::Admin) {
            return Err(ContractError::NotAuthorized);
        }

        env.deployer()
            .update_current_contract_wasm(new_wasm_hash.clone());

        metadata::write_entry(&env, &CredentialDataKey::WasmHash, &new_wasm_hash);

        let mut version: u32 = env
            .storage()
            .persistent()
            .get(&CredentialDataKey::UpgradeVersion)
            .unwrap_or(0);
        version += 1;
        metadata::write_entry(&env, &CredentialDataKey::UpgradeVersion, &version);

        env.events()
            .publish((Symbol::new(&env, "upgraded"),), (new_wasm_hash, version));
        Ok(())
    }

    /// Wasm hash the contract was most recently upgraded to, or `None` if
    /// it has never been upgraded.
    pub fn wasm_hash(env: Env) -> Option<BytesN<32>> {
        env.storage().persistent().get(&CredentialDataKey::WasmHash)
    }

    /// Number of times the contract has been upgraded via `upgrade()`.
    /// Starts at `0` for a never-upgraded contract.
    pub fn upgrade_version(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&CredentialDataKey::UpgradeVersion)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Events as _, Ledger as _, MockAuth, MockAuthInvoke},
        vec, Address, Bytes, IntoVal, SymbolStr, TryFromVal,
    };

    /// Register both contracts and return `(admin, credential_id, tracker_id)`.
    // ── Issue #422: pause/unpause must be observable ─────────────────────

    #[test]
    fn test_pause_emits_paused_event() {
        let env = Env::default();
        let (admin, _contract_id, _tracker_id) = setup_contract(&env);
        let contract_id = env.register_contract(None, CredentialNft);
        let client = CredentialNftClient::new(&env, &contract_id);
        client.initialize(&admin, &Address::generate(&env));

        env.mock_all_auths();
        let now = env.ledger().timestamp();

        client.pause(&admin);

        let all = env.events().all();
        let last = all.last().expect("no event emitted");
        assert_eq!(
            vec![&env, last],
            vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "paused"),).into_val(&env),
                    (admin, now).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_unpause_emits_unpaused_event() {
        let env = Env::default();
        let (admin, _contract_id, _tracker_id) = setup_contract(&env);
        let contract_id = env.register_contract(None, CredentialNft);
        let client = CredentialNftClient::new(&env, &contract_id);
        client.initialize(&admin, &Address::generate(&env));

        env.mock_all_auths();
        let now = env.ledger().timestamp();

        client.pause(&admin);
        env.events().all(); // discard the pause event

        client.unpause(&admin);

        let all = env.events().all();
        let last = all.last().expect("no event emitted");
        assert_eq!(
            vec![&env, last],
            vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "unpaused"),).into_val(&env),
                    (admin, now).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_pause_rejects_double_pause() {
        let env = Env::default();
        let (admin, _contract_id, _tracker_id) = setup_contract(&env);
        let contract_id = env.register_contract(None, CredentialNft);
        let client = CredentialNftClient::new(&env, &contract_id);
        client.initialize(&admin, &Address::generate(&env));
        env.mock_all_auths();

        client.pause(&admin);
        let err = client
            .try_pause(&admin)
            .err()
            .expect("should fail")
            .expect("expected typed error");
        assert_eq!(err, ContractError::AlreadyInitialized);
    }

    #[test]
    fn test_unpause_rejects_when_not_paused() {
        let env = Env::default();
        let (admin, _contract_id, _tracker_id) = setup_contract(&env);
        let contract_id = env.register_contract(None, CredentialNft);
        let client = CredentialNftClient::new(&env, &contract_id);
        client.initialize(&admin, &Address::generate(&env));
        env.mock_all_auths();

        let err = client
            .try_unpause(&admin)
            .err()
            .expect("should fail")
            .expect("expected typed error");
        assert_eq!(err, ContractError::AlreadyInitialized);
    }

    // ── Issue #423: delayed two-step admin transfer ──────────────────────

    #[test]
    fn test_transfer_admin_defers_the_handover() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        env.mock_all_auths();
        client.transfer_admin(&new_admin);

        // The whole point of the delay: admin is unchanged until acceptance.
        assert_eq!(client.admin(), admin);
        let pending = client.pending_admin().expect("no pending transfer");
        assert_eq!(pending.new_admin, new_admin);
    }

    #[test]
    fn test_accept_admin_completes_the_transfer() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        env.mock_all_auths();
        client.set_admin_transfer_delay(&3_600);
        client.transfer_admin(&new_admin);
        // Advance past the configured one-hour minimum.
        env.ledger().with_mut(|l| l.timestamp += 3_600);
        client.accept_admin();

        assert_eq!(client.admin(), new_admin);
        assert!(client.pending_admin().is_none());
    }

    #[test]
    fn test_accept_admin_panics_before_the_delay_elapses() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        env.mock_all_auths();
        client.transfer_admin(&new_admin);
        let err = client
            .try_accept_admin()
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::AlreadyInitialized);
    }

    #[test]
    fn test_cancel_admin_transfer_aborts_the_handover() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        env.mock_all_auths();
        client.set_admin_transfer_delay(&3_600);
        client.transfer_admin(&new_admin);
        client.cancel_admin_transfer();

        assert!(client.pending_admin().is_none());
        assert_eq!(client.admin(), admin);
    }

    #[test]
    fn test_accept_admin_panics_after_cancellation() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        env.mock_all_auths();
        client.set_admin_transfer_delay(&3_600);
        client.transfer_admin(&new_admin);
        client.cancel_admin_transfer();
        let err = client
            .try_accept_admin()
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::NotFound);
    }

    #[test]
    fn test_set_admin_transfer_delay_rejects_zero() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();
        let err = client
            .try_set_admin_transfer_delay(&0)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::AlreadyInitialized);
    }

    #[test]
    fn test_set_admin_transfer_delay_rejects_below_one_hour() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();
        let err = client
            .try_set_admin_transfer_delay(&3_599)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::AlreadyInitialized);
    }

    #[test]
    fn test_set_admin_transfer_delay_accepts_one_hour_minimum() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        env.mock_all_auths();
        client.set_admin_transfer_delay(&3_600);
        assert_eq!(client.admin_transfer_delay(), 3_600);
    }

    #[test]
    fn test_accept_admin_honors_the_configured_minimum_delay() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        env.mock_all_auths();
        client.set_admin_transfer_delay(&3_600);
        client.transfer_admin(&new_admin);
        env.ledger().with_mut(|l| l.timestamp += 3_599);
        let err = client
            .try_accept_admin()
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::AlreadyInitialized);
    }

    #[test]
    fn test_set_admin_transfer_delay_requires_admin_auth() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        // No authorization is mocked, so the admin-only call must be rejected
        // before the minimum-delay check ever runs.
        assert!(client.try_set_admin_transfer_delay(&3_600).is_err());
    }

    #[test]
    fn test_default_admin_transfer_delay_matches_learn_token() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        assert_eq!(client.admin_transfer_delay(), 172_800);
    }

    // ── Issue #539: admin transfer unit tests (events, auth, lifecycle) ───

    #[test]
    fn test_transfer_admin_emits_initiated_event() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        env.mock_all_auths();
        client.transfer_admin(&new_admin);

        let events = env.events().all();
        let (_, topics, data) = events.last().unwrap();
        assert_eq!(
            topics,
            (
                Symbol::new(&env, "admin_transfer_initiated"),
                new_admin.clone()
            )
                .into_val(&env)
        );
        let data_vec: soroban_sdk::Vec<soroban_sdk::Val> = data.into_val(&env);
        assert_eq!(data_vec.len(), 3);
        let data_admin: Address = data_vec.get(0).unwrap().into_val(&env);
        let initiated_at: u64 = data_vec.get(1).unwrap().into_val(&env);
        let accept_after: u64 = data_vec.get(2).unwrap().into_val(&env);
        assert_eq!(data_admin, admin);
        assert_eq!(initiated_at, 0);
        assert_eq!(accept_after, 172_800);
    }

    #[test]
    fn test_transfer_admin_requires_admin_auth() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        assert!(client.try_transfer_admin(&new_admin).is_err());
        assert!(client.pending_admin().is_none());
    }

    #[test]
    fn test_transfer_admin_rejects_zero_address() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        env.mock_all_auths();
        let err = client
            .try_transfer_admin(&chainlearn_shared::zero_address(&env))
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::NotAuthorized);
    }

    #[test]
    fn test_cancel_admin_transfer_emits_cancelled_event() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        env.mock_all_auths();
        client.set_admin_transfer_delay(&3_600);
        client.transfer_admin(&new_admin);
        client.cancel_admin_transfer();

        let events = env.events().all();
        let (_, topics, data) = events.last().unwrap();
        assert_eq!(
            topics,
            (
                Symbol::new(&env, "admin_transfer_cancelled"),
                new_admin.clone()
            )
                .into_val(&env)
        );
        let data_vec: soroban_sdk::Vec<soroban_sdk::Val> = data.into_val(&env);
        assert_eq!(data_vec.len(), 1);
        let data_admin: Address = data_vec.get(0).unwrap().into_val(&env);
        assert_eq!(data_admin, admin);
    }

    #[test]
    fn test_cancel_admin_transfer_requires_admin_auth() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        env.as_contract(&contract_id, || {
            metadata::write_entry(
                &env,
                &CredentialDataKey::PendingAdmin,
                &metadata::PendingAdminTransfer {
                    new_admin: new_admin.clone(),
                    initiated_at: 0,
                },
            );
        });

        assert!(client.try_cancel_admin_transfer().is_err());
        assert_eq!(client.admin(), admin);
        let pending = client.pending_admin().expect("no pending transfer");
        assert_eq!(pending.new_admin, new_admin);
    }

    #[test]
    fn test_accept_admin_emits_accepted_event() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        env.mock_all_auths();
        client.set_admin_transfer_delay(&3_600);
        client.transfer_admin(&new_admin);
        env.ledger().with_mut(|l| l.timestamp += 3_600);
        client.accept_admin();

        let events = env.events().all();
        let (_, topics, data) = events.last().unwrap();
        assert_eq!(
            topics,
            (
                Symbol::new(&env, "admin_transfer_accepted"),
                new_admin.clone()
            )
                .into_val(&env)
        );
        let data_vec: soroban_sdk::Vec<soroban_sdk::Val> = data.into_val(&env);
        assert_eq!(data_vec.len(), 1);
        let data_admin: Address = data_vec.get(0).unwrap().into_val(&env);
        assert_eq!(data_admin, admin);
    }

    #[test]
    fn test_accept_admin_requires_pending_new_admin_auth() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        env.ledger().with_mut(|l| l.timestamp += 172_801);
        env.as_contract(&contract_id, || {
            metadata::write_entry(
                &env,
                &CredentialDataKey::PendingAdmin,
                &metadata::PendingAdminTransfer {
                    new_admin: new_admin.clone(),
                    initiated_at: 0,
                },
            );
        });

        assert!(client.try_accept_admin().is_err());
        assert_eq!(client.admin(), admin);
        let pending = client.pending_admin().expect("no pending transfer");
        assert_eq!(pending.new_admin, new_admin);
    }

    #[test]
    fn test_pending_admin_reports_the_full_lifecycle() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        assert!(client.pending_admin().is_none());

        env.mock_all_auths();
        client.set_admin_transfer_delay(&3_600);
        client.transfer_admin(&new_admin);

        let pending = client.pending_admin().expect("no pending transfer");
        assert_eq!(pending.new_admin, new_admin);
        assert_eq!(pending.initiated_at, env.ledger().timestamp());

        client.cancel_admin_transfer();
        assert!(client.pending_admin().is_none());

        client.transfer_admin(&new_admin);
        env.ledger().with_mut(|l| l.timestamp += 3_600);
        client.accept_admin();
        assert!(client.pending_admin().is_none());
        assert_eq!(client.admin(), new_admin);
    }

    #[test]
    fn test_double_transfer_overwrites_previous_pending() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let first = Address::generate(&env);
        let second = Address::generate(&env);

        env.mock_all_auths();
        client.set_admin_transfer_delay(&3_600);
        client.transfer_admin(&first);
        client.transfer_admin(&second);

        let pending = client.pending_admin().expect("no pending transfer");
        assert_eq!(pending.new_admin, second);

        // The first candidate can never accept the transfer anymore: only the
        // pending `new_admin` is allowed to sign the acceptance.
        env.ledger().with_mut(|l| l.timestamp += 3_600);
        env.mock_auths(&[MockAuth {
            address: &first,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "accept_admin",
                args: vec![&env],
                sub_invokes: &[],
            },
        }]);
        assert!(client.try_accept_admin().is_err());

        // The pending transfer is untouched and the admin is unchanged.
        let pending = client.pending_admin().expect("no pending transfer");
        assert_eq!(pending.new_admin, second);
        assert_eq!(client.admin(), _admin);
    }

    #[test]
    fn test_transfer_admin_requires_the_current_admin_signature() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        env.mock_all_auths();
        client.transfer_admin(&new_admin);

        // Drop the admin signature: only the current admin may start a
        // handover, so the second attempt must be rejected.
        env.set_auths(&[]);
        let other_admin = Address::generate(&env);
        assert!(client.try_transfer_admin(&other_admin).is_err());

        // The handover that was already initiated is left untouched.
        let pending = client.pending_admin().expect("no pending transfer");
        assert_eq!(pending.new_admin, new_admin);
    }

    #[test]
    fn test_pending_admin_alone_can_accept_the_handover() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        env.mock_all_auths();
        client.set_admin_transfer_delay(&3_600);
        client.transfer_admin(&new_admin);
        env.ledger().with_mut(|l| l.timestamp += 3_600);

        // The handover is completed by the incoming admin alone: the current
        // admin never has to sign the acceptance.
        env.mock_auths(&[MockAuth {
            address: &new_admin,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "accept_admin",
                args: vec![&env],
                sub_invokes: &[],
            },
        }]);
        client.accept_admin();

        assert_eq!(client.admin(), new_admin);
        assert!(client.pending_admin().is_none());
    }

    #[test]
    fn test_default_admin_transfer_delay_blocks_acceptance_within_48_hours() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        env.mock_all_auths();
        client.transfer_admin(&new_admin);

        // The default delay is 48 hours.
        assert_eq!(client.admin_transfer_delay(), 172_800);
        env.ledger().with_mut(|l| l.timestamp += 172_799);
        assert!(client.try_accept_admin().is_err());

        env.ledger().with_mut(|l| l.timestamp += 1);
        client.accept_admin();
        assert_eq!(client.admin(), new_admin);
    }

    #[test]
    fn test_admin_transfer_delay_is_never_below_the_minimum() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        env.mock_all_auths();
        // A delay below the one-hour minimum is rejected and leaves the
        // configured delay at its default value.
        let err = client
            .try_set_admin_transfer_delay(&0)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::AlreadyInitialized);
        assert_eq!(client.admin_transfer_delay(), 172_800);

        client.set_admin_transfer_delay(&3_600);
        assert_eq!(client.admin_transfer_delay(), 3_600);
    }

    #[test]
    fn test_set_admin_transfer_delay_accepts_larger_values() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        env.mock_all_auths();
        client.set_admin_transfer_delay(&86_400);
        assert_eq!(client.admin_transfer_delay(), 86_400);

        let new_admin = Address::generate(&env);
        client.transfer_admin(&new_admin);
        env.ledger().with_mut(|l| l.timestamp += 86_399);
        assert!(client.try_accept_admin().is_err());
        env.ledger().with_mut(|l| l.timestamp += 1);
        client.accept_admin();
        assert_eq!(client.admin(), new_admin);
    }

    // ── Issue #540: upgrade/wasm_hash/upgrade_version unit tests ────────────

    #[test]
    fn test_wasm_hash_returns_none_before_upgrade() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        assert!(client.wasm_hash().is_none());
    }

    #[test]
    fn test_upgrade_version_returns_zero_initially() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        assert_eq!(client.upgrade_version(), 0);
    }

    // ── Issue #540: upgrade / wasm_hash / upgrade_version unit tests ───────

    /// Upload a placeholder WASM module so `upgrade()` has a hash to point at.
    /// The module body is irrelevant for these tests: the contract only
    /// records the hash, bumps the version and emits the `upgraded` event.
    fn upload_placeholder_wasm(env: &Env) -> BytesN<32> {
        env.deployer()
            .upload_contract_wasm(Bytes::from_slice(env, &[]))
    }

    #[test]
    fn test_upgrade_requires_admin_auth() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        // No authorization is mocked, so the admin signature is missing.
        let fake_hash = BytesN::from_array(&env, &[7u8; 32]);
        assert!(client.try_upgrade(&admin, &fake_hash).is_err());
    }

    #[test]
    fn test_upgrade_rejects_non_admin_role_holder() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let not_admin = Address::generate(&env);
        let wasm_hash = upload_placeholder_wasm(&env);
        let err = client
            .try_upgrade(&not_admin, &wasm_hash)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::NotAuthorized);
    }

    #[test]
    fn test_upgrade_sets_wasm_hash_and_increments_version() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let wasm_hash = upload_placeholder_wasm(&env);
        client.upgrade(&admin, &wasm_hash);

        assert_eq!(client.wasm_hash(), Some(wasm_hash));
        assert_eq!(client.upgrade_version(), 1);
    }

    #[test]
    fn test_upgrade_emits_upgraded_event() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let wasm_hash = upload_placeholder_wasm(&env);
        client.upgrade(&admin, &wasm_hash);

        let events = env.events().all();
        let (_, topics, data) = events.last().unwrap();
        assert_eq!(topics, (Symbol::new(&env, "upgraded"),).into_val(&env));
        let data_vec: soroban_sdk::Vec<soroban_sdk::Val> = data.into_val(&env);
        assert_eq!(data_vec.len(), 2);
        let event_hash: BytesN<32> = data_vec.get(0).unwrap().into_val(&env);
        let event_version: u32 = data_vec.get(1).unwrap().into_val(&env);
        assert_eq!(event_hash, wasm_hash);
        assert_eq!(event_version, 1);
    }

    #[test]
    fn test_failed_upgrade_leaves_version_and_hash_untouched() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        // The hash was never uploaded, so the upgrade must not go through.
        let unknown_hash = BytesN::from_array(&env, &[9u8; 32]);
        assert!(client.try_upgrade(&admin, &unknown_hash).is_err());

        assert_eq!(client.wasm_hash(), None);
        assert_eq!(client.upgrade_version(), 0);
    }

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
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let metadata_uri = Symbol::new(&env, "ipfs_Qm123");
        enrolled_and_completed(&env, &tracker_id, &learner, &course_id);

        let cred_id = client.mint_credential(&admin, &learner, &course_id, &85, &metadata_uri);
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
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course1 = Symbol::new(&env, "rust_101");
        let course2 = Symbol::new(&env, "sol_201");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course1, 90);
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course2, 75);

        let cred1 = client.mint_credential(&admin, &learner, &course1, &90, &uri);
        let cred2 = client.mint_credential(&admin, &learner, &course2, &75, &uri);

        assert_eq!(client.get_credential_count(&learner), 2);

        let creds = client.get_credentials_for(&learner, &0, &10);
        assert_eq!(creds.len(), 2);
        assert_eq!(creds.get(0).unwrap(), cred1);
        assert_eq!(creds.get(1).unwrap(), cred2);
    }

    #[test]
    fn test_get_credentials_for_paginates() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
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

        let cred1 = client.mint_credential(&admin, &learner, &course1, &90, &uri);
        let cred2 = client.mint_credential(&admin, &learner, &course2, &75, &uri);
        let cred3 = client.mint_credential(&admin, &learner, &course3, &80, &uri);

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
    fn test_mint_rejects_inflated_score() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 50);
        let err = client
            .try_mint_credential(&admin, &learner, &course_id, &100, &uri)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::InvalidScore);
    }

    #[test]
    fn test_mint_rejects_understated_score() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 90);
        let err = client
            .try_mint_credential(&admin, &learner, &course_id, &80, &uri)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::InvalidScore);
    }

    #[test]
    fn test_mint_accepts_verified_score() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 72);

        let cred_id = client.mint_credential(&admin, &learner, &course_id, &72, &uri);
        assert_eq!(client.verify_credential(&cred_id).score, 72);
    }

    #[test]
    fn test_mint_verifies_against_average_of_all_quizzes() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
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

        let cred_id = client.mint_credential(&admin, &learner, &course_id, &85, &uri);
        assert_eq!(client.verify_credential(&cred_id).score, 85);
    }

    #[test]
    fn test_mint_rejects_score_from_a_different_learner() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let strong_learner = Address::generate(&env);
        let weak_learner = Address::generate(&env);
        env.mock_all_auths();
        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &strong_learner, &course_id, 95);
        complete_course_with_score(&env, &tracker_id, &weak_learner, &course_id, 60);
        let err = client
            .try_mint_credential(&admin, &weak_learner, &course_id, &95, &uri)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::InvalidScore);
    }

    // ── Issue #105: reverse lookup from course_id to credentials ──────────

    #[test]
    fn test_get_credentials_by_course_returns_minted() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
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

        let id1 = client.mint_credential(&admin, &learner1, &course, &80, &uri);
        let id2 = client.mint_credential(&admin, &learner2, &course, &90, &uri);
        client.mint_credential(&admin, &learner1, &other_course, &70, &uri);

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
        let (admin, contract_id, tracker_id) = setup_contract(&env);
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

        let id_a = client.mint_credential(&admin, &learner, &course_a, &85, &uri);
        let id_b = client.mint_credential(&admin, &learner, &course_b, &75, &uri);

        let a_creds = client.get_credentials_by_course(&course_a);
        assert_eq!(a_creds.len(), 1);
        assert_eq!(a_creds.get(0).unwrap(), id_a);

        let b_creds = client.get_credentials_by_course(&course_b);
        assert_eq!(b_creds.len(), 1);
        assert_eq!(b_creds.get(0).unwrap(), id_b);
    }

    #[test]
    fn test_mint_rejects_counter_overflow() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
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

        let err = client
            .try_mint_credential(&admin, &learner, &course_id, &90, &uri)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::NotFound);
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

        let cred_id = client.mint_credential(&_admin, &learner, &course_id, &85, &uri);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        // Learner and course_id are indexed topics (#548); the credential id,
        // score and metadata URI stay in the payload (#101).
        assert_eq!(
            vec![&env, last],
            vec![
                &env,
                (
                    contract_id.clone(),
                    (
                        Symbol::new(&env, "credential_minted"),
                        learner.clone(),
                        course_id.clone()
                    )
                        .into_val(&env),
                    (cred_id, 85u32, uri.clone()).into_val(&env),
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

        let cred_id = client.mint_credential(&admin, &learner, &course_id, &80, &uri);
        client.revoke_credential(&cred_id);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            vec![&env, last],
            vec![
                &env,
                (
                    contract_id.clone(),
                    (
                        Symbol::new(&env, "credential_revoked"),
                        learner.clone(),
                        course_id.clone()
                    )
                        .into_val(&env),
                    (cred_id, admin).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_mint_rejects_low_score() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed(&env, &tracker_id, &learner, &course_id);
        let err = client
            .try_mint_credential(&admin, &learner, &course_id, &40, &uri)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::InvalidScore);
    }

    #[test]
    fn test_mint_prevents_duplicates() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 90);
        client.mint_credential(&admin, &learner, &course_id, &90, &uri);
        let err = client
            .try_mint_credential(&admin, &learner, &course_id, &90, &uri)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::DuplicateCredential);
    }

    #[test]
    fn test_mint_rejects_unknown_course() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        // No course was ever created via `create_course` on the tracker.
        let course_id = Symbol::new(&env, "ghost_course");
        let uri = Symbol::new(&env, "ipfs_meta");

        let err = client
            .try_mint_credential(&admin, &learner, &course_id, &90, &uri)
            .err()
            .expect("should fail")
            .expect("expected typed error");
        assert_eq!(err, ContractError::NotFound);
    }

    #[test]
    fn test_mint_rejects_ineligible_learner() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let tracker_client = progress_tracker::ProgressTrackerClient::new(&env, &tracker_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");

        // Enrolled, but no modules completed and no quizzes submitted.
        create_course(&env, &tracker_id, &course_id);
        tracker_client.enroll(&learner, &course_id);

        let err = client
            .try_mint_credential(&admin, &learner, &course_id, &100, &uri)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::NotFound);
    }

    #[test]
    fn test_mint_rejects_incomplete_modules() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let tracker_client = progress_tracker::ProgressTrackerClient::new(&env, &tracker_id);
        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        create_course(&env, &tracker_id, &course_id);
        tracker_client.enroll(&learner, &course_id);
        tracker_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        tracker_client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &95);
        let err = client
            .try_mint_credential(&admin, &learner, &course_id, &95, &uri)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::NotFound);
    }

    #[test]
    fn test_mint_rejects_missing_quiz() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let tracker_client = progress_tracker::ProgressTrackerClient::new(&env, &tracker_id);
        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        create_course(&env, &tracker_id, &course_id);
        tracker_client.enroll(&learner, &course_id);
        tracker_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        tracker_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
        let err = client
            .try_mint_credential(&admin, &learner, &course_id, &90, &uri)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::NotFound);
    }

    #[test]
    fn test_revoke_credential() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 80);

        let cred_id = client.mint_credential(&admin, &learner, &course_id, &80, &uri);
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
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 80);

        let cred_id = client.mint_credential(&admin, &learner, &course_id, &80, &uri);
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
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        assert_eq!(client.get_total_credentials_count(), 0);

        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course = Symbol::new(&env, "rust_101");
        // Symbols cannot contain ':' or '/', so the fixture uses a symbol-safe
        // stand-in for the metadata URI.
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 80);

        client.mint_credential(&admin, &learner, &course, &80, &uri);
        assert_eq!(client.get_total_credentials_count(), 1);

        let learner2 = Address::generate(&env);
        complete_course_with_score(&env, &tracker_id, &learner2, &course, 90);
        client.mint_credential(&admin, &learner2, &course, &90, &uri);
        assert_eq!(client.get_total_credentials_count(), 2);
    }

    // ── Issue #104: learner credentials vec pruned on revoke ─────────────────

    #[test]
    fn test_revoke_prunes_learner_credentials_list() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course = Symbol::new(&env, "rust_101");
        // Symbols cannot contain ':' or '/', so the fixture uses a symbol-safe
        // stand-in for the metadata URI.
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 80);

        let cred_id = client.mint_credential(&admin, &learner, &course, &80, &uri);

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
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course = Symbol::new(&env, "rust_101");
        // Symbols cannot contain ':' or '/', so the fixture uses a symbol-safe
        // stand-in for the metadata URI.
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 80);

        let cred_id = client.mint_credential(&admin, &learner, &course, &80, &uri);

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
        let (admin, contract_id, tracker_id) = setup_contract(&env);
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
        let cred_id = client.mint_credential(&admin, &learner, &course, &85, &cert_uri);
        let info = client.verify_credential(&cred_id);
        assert_eq!(info.metadata_uri, cert_uri);
    }

    #[test]
    fn test_get_certificate_uri_is_none_before_generation() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course = Symbol::new(&env, "rust_101");

        // Completing the course does not by itself create a certificate URI;
        // only generate_certificate does (#545).
        enrolled_and_completed(&env, &tracker_id, &learner, &course);
        assert_eq!(client.get_certificate_uri(&learner, &course), None);
    }

    #[test]
    fn test_get_certificate_uri_returns_generated_uri() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course = Symbol::new(&env, "rust_101");

        enrolled_and_completed(&env, &tracker_id, &learner, &course);
        let cert_uri = client.generate_certificate(&learner, &course);

        // The stored URI is exactly what generate_certificate returned.
        assert_eq!(
            client.get_certificate_uri(&learner, &course),
            Some(cert_uri)
        );
    }

    #[test]
    fn test_get_certificate_uri_is_stable_across_repeated_generation() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course = Symbol::new(&env, "rust_101");

        enrolled_and_completed(&env, &tracker_id, &learner, &course);

        let first = client.generate_certificate(&learner, &course);
        assert_eq!(
            client.get_certificate_uri(&learner, &course),
            Some(first.clone())
        );

        // generate_certificate is idempotent: it returns the stored URI
        // instead of minting a new one, so get_certificate_uri keeps
        // reporting the same value.
        let second = client.generate_certificate(&learner, &course);
        assert_eq!(first, second);
        assert_eq!(client.get_certificate_uri(&learner, &course), Some(second));
    }

    #[test]
    fn test_get_certificate_uri_is_scoped_per_learner() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner1 = Address::generate(&env);
        let learner2 = Address::generate(&env);
        env.mock_all_auths();
        let course = Symbol::new(&env, "rust_101");

        create_course(&env, &tracker_id, &course);
        complete_course(&env, &tracker_id, &learner1, &course);
        complete_course(&env, &tracker_id, &learner2, &course);

        let uri1 = client.generate_certificate(&learner1, &course);
        // learner2 has not generated theirs yet, and learner1's URI must not
        // leak into learner2's slot.
        assert_eq!(client.get_certificate_uri(&learner2, &course), None);

        let uri2 = client.generate_certificate(&learner2, &course);
        assert_ne!(uri1, uri2);
        assert_eq!(client.get_certificate_uri(&learner1, &course), Some(uri1));
        assert_eq!(client.get_certificate_uri(&learner2, &course), Some(uri2));
    }

    #[test]
    fn test_get_certificate_uri_is_scoped_per_course() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course1 = Symbol::new(&env, "rust_101");
        let course2 = Symbol::new(&env, "sol_201");

        create_course(&env, &tracker_id, &course1);
        complete_course(&env, &tracker_id, &learner, &course1);
        create_course(&env, &tracker_id, &course2);
        complete_course(&env, &tracker_id, &learner, &course2);

        let uri1 = client.generate_certificate(&learner, &course1);
        // Generating a certificate for one course must not create one for
        // another course the same learner completed.
        assert_eq!(client.get_certificate_uri(&learner, &course2), None);

        let uri2 = client.generate_certificate(&learner, &course2);
        assert_ne!(uri1, uri2);
        assert_eq!(client.get_certificate_uri(&learner, &course1), Some(uri1));
        assert_eq!(client.get_certificate_uri(&learner, &course2), Some(uri2));
    }

    #[test]
    fn test_get_certificate_uri_is_none_for_unknown_learner_and_course() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        env.mock_all_auths();

        // Querying a pair that was never generated returns None rather than
        // panicking, so indexers can probe safely (#545).
        let learner = Address::generate(&env);
        let course = Symbol::new(&env, "rust_101");
        assert_eq!(client.get_certificate_uri(&learner, &course), None);
    }

    #[test]
    fn test_get_certificate_uri_uses_the_cert_prefix_format() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course = Symbol::new(&env, "rust_101");

        enrolled_and_completed(&env, &tracker_id, &learner, &course);
        let cert_uri = client.generate_certificate(&learner, &course);

        // The generated URI follows the documented on-chain format:
        // "cert_" + 24 lowercase hex characters of the learner/course hash.
        let symbol_str = SymbolStr::try_from_val(&env, &cert_uri.to_symbol_val())
            .expect("the URI is a well-formed symbol");
        let uri: &str = symbol_str.as_ref();
        assert_eq!(uri.len(), 29, "unexpected URI: {uri}");
        assert!(uri.starts_with("cert_"), "unexpected URI: {uri}");
        assert!(
            uri[5..]
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "suffix must be lowercase hex, got: {uri}"
        );

        let stored = client.get_certificate_uri(&learner, &course);
        assert_eq!(stored, Some(cert_uri));
    }

    #[test]
    fn test_mint_rejects_empty_metadata_uri() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 85);
        let empty_uri = Symbol::new(&env, "");
        let err = client
            .try_mint_credential(&admin, &learner, &course, &85, &empty_uri)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::InvalidMetadataUri);
    }

    #[test]
    fn test_mint_rejects_too_short_metadata_uri() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 85);
        let short_uri = Symbol::new(&env, "ipfs_1");
        let err = client
            .try_mint_credential(&admin, &learner, &course, &85, &short_uri)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::InvalidMetadataUri);
    }

    #[test]
    fn test_mint_rejects_malformed_metadata_uri() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 85);
        let invalid_uri = Symbol::new(&env, "ftp_metadata_hash");
        let err = client
            .try_mint_credential(&admin, &learner, &course, &85, &invalid_uri)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::InvalidMetadataUri);
    }

    #[test]
    fn test_mint_accepts_valid_schemes() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 85);

        let uri = Symbol::new(&env, "ipfs_hash12345");
        let id = client.mint_credential(&admin, &learner, &course, &85, &uri);
        assert_eq!(id, 1);
        let info = client.verify_credential(&id);
        assert_eq!(info.metadata_uri, uri);
    }

    // ── #227 fix: verify_credential_with_display's Vec-based optional ──────

    #[test]
    fn test_verify_credential_with_display_defaults_to_none_set() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 85);
        let id = client.mint_credential(
            &admin,
            &learner,
            &course,
            &85,
            &Symbol::new(&env, "ipfs_meta"),
        );

        // No display properties were ever set for this credential.
        let verification = client.verify_credential_with_display(&id);
        assert_eq!(verification.info, client.verify_credential(&id));
        assert!(verification.display.is_empty());
    }

    #[test]
    fn test_verify_credential_with_display_returns_set_properties() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 85);
        let id = client.mint_credential(
            &admin,
            &learner,
            &course,
            &85,
            &Symbol::new(&env, "ipfs_meta"),
        );

        let image_url = Some(Symbol::new(&env, "ipfs_img"));
        let description = Some(Symbol::new(&env, "rust_cert"));
        client.set_credential_display(&id, &image_url, &description, &None);

        let verification = client.verify_credential_with_display(&id);
        assert_eq!(verification.display.len(), 1);
        let display = verification.display.get(0).unwrap();
        assert_eq!(display.image_url, image_url);
        assert_eq!(display.description, description);
        assert!(display.issuer_name.is_none());
        // The credential's core info is unaffected by setting display data.
        assert_eq!(verification.info, client.verify_credential(&id));
    }

    // Issue #227 ("Add credential transfer rejection with reason") is a
    // content-duplicate of already-merged #242 (identical title/body); #242's
    // Soulbound-rejection behavior and its `require_auth()`-free design are
    // already covered by `test_transfer_always_returns_soulbound_error`,
    // `test_transfer_rejects_even_without_auth_or_existing_credential`, and
    // `test_transfer_does_not_mutate_credential_state` in
    // `tests/unit/credential_tests.rs`, so no new tests are added here.

    // ── Issue #551: RBAC tests ─────────────────────────────────────────────

    #[test]
    fn test_has_role_returns_false_before_grant() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let minter = Address::generate(&env);
        assert!(!client.has_role(&AdminRole::Minter, &minter));
    }

    #[test]
    fn test_grant_role_minter_enables_has_role() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let minter = Address::generate(&env);
        client.grant_role(&admin, &AdminRole::Minter, &minter);
        assert!(client.has_role(&AdminRole::Minter, &minter));
    }

    #[test]
    fn test_revoke_role_removes_has_role() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let minter = Address::generate(&env);
        client.grant_role(&admin, &AdminRole::Minter, &minter);
        assert!(client.has_role(&AdminRole::Minter, &minter));

        client.revoke_role(&admin, &AdminRole::Minter, &minter);
        assert!(!client.has_role(&AdminRole::Minter, &minter));
    }

    #[test]
    fn test_admin_always_has_all_roles() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        assert!(client.has_role(&AdminRole::Admin, &admin));
        assert!(client.has_role(&AdminRole::Minter, &admin));
        assert!(client.has_role(&AdminRole::Pauser, &admin));
    }

    #[test]
    fn test_admin_can_grant_minter_who_can_mint() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let minter = Address::generate(&env);
        let learner = Address::generate(&env);
        client.grant_role(&admin, &AdminRole::Minter, &minter);

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_Qm123");
        enrolled_and_completed(&env, &tracker_id, &learner, &course_id);
        let cred_id = client.mint_credential(&minter, &learner, &course_id, &85, &uri);
        assert_eq!(cred_id, 1);
    }

    #[test]
    fn test_admin_role_holder_can_grant_roles() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let sub_admin = Address::generate(&env);
        let other = Address::generate(&env);
        client.grant_role(&admin, &AdminRole::Admin, &sub_admin);
        // sub_admin can now grant Minter to another
        client.grant_role(&sub_admin, &AdminRole::Minter, &other);
        assert!(client.has_role(&AdminRole::Minter, &other));
    }

    #[test]
    fn test_mint_requires_minter_or_admin_role() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let nobody = Address::generate(&env);
        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_Qm123");
        enrolled_and_completed(&env, &tracker_id, &learner, &course_id);
        assert!(client
            .try_mint_credential(&nobody, &learner, &course_id, &85, &uri)
            .is_err());
    }

    #[test]
    fn test_admin_can_mint_without_explicit_minter_grant() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_Qm123");
        enrolled_and_completed(&env, &tracker_id, &learner, &course_id);
        let cred_id = client.mint_credential(&admin, &learner, &course_id, &85, &uri);
        assert_eq!(cred_id, 1);
    }

    #[test]
    fn test_pause_requires_pauser_role() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let nobody = Address::generate(&env);
        assert!(client.try_pause(&nobody).is_err());
    }

    #[test]
    fn test_unpause_requires_pauser_role() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        client.pause(&admin);
        let nobody = Address::generate(&env);
        assert!(client.try_unpause(&nobody).is_err());
    }

    #[test]
    fn test_pauser_can_pause_and_unpause() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let pauser = Address::generate(&env);
        client.grant_role(&admin, &AdminRole::Pauser, &pauser);

        client.pause(&pauser);
        env.as_contract(&contract_id, || {
            assert!(env
                .storage()
                .persistent()
                .get::<_, bool>(&metadata::CredentialDataKey::Paused)
                .unwrap_or(false));
        });

        client.unpause(&pauser);
        env.as_contract(&contract_id, || {
            assert!(!env
                .storage()
                .persistent()
                .get::<_, bool>(&metadata::CredentialDataKey::Paused)
                .unwrap_or(false));
        });
    }

    #[test]
    fn test_admin_can_pause_without_explicit_pauser_grant() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        client.pause(&admin);
        env.as_contract(&contract_id, || {
            assert!(env
                .storage()
                .persistent()
                .get::<_, bool>(&metadata::CredentialDataKey::Paused)
                .unwrap_or(false));
        });
    }

    #[test]
    fn test_upgrade_requires_admin_role() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let nobody = Address::generate(&env);
        let wasm_hash = BytesN::from_array(&env, &[0u8; 32]);
        assert!(client.try_upgrade(&nobody, &wasm_hash).is_err());
    }

    #[test]
    fn test_non_admin_cannot_grant_role() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let minter = Address::generate(&env);
        let other = Address::generate(&env);
        client.grant_role(&admin, &AdminRole::Minter, &minter);
        // minter cannot grant roles (not an Admin)
        assert!(client
            .try_grant_role(&minter, &AdminRole::Minter, &other)
            .is_err());
    }

    #[test]
    fn test_non_admin_cannot_revoke_role() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let minter = Address::generate(&env);
        client.grant_role(&admin, &AdminRole::Minter, &minter);
        let nobody = Address::generate(&env);
        // nobody cannot revoke roles
        assert!(client
            .try_revoke_role(&nobody, &AdminRole::Minter, &minter)
            .is_err());
    }
}
