//! Unit tests for the credential-nft contract.

use credential_nft::{ContractError, CredentialNft, CredentialNftClient};
use progress_tracker::{ProgressTracker, ProgressTrackerClient};
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    Address, Env, IntoVal, Symbol,
};

#[cfg(test)]
mod credential_unit_tests {
    use super::*;

    fn setup_contract(env: &Env) -> (Address, Address, Address) {
        let admin = Address::generate(env);

        let tracker_id = env.register_contract(None, ProgressTracker);
        let tracker_client = ProgressTrackerClient::new(env, &tracker_id);
        tracker_client.initialize(&admin);

        let contract_id = env.register_contract(None, CredentialNft);
        let client = CredentialNftClient::new(env, &contract_id);
        client.initialize(&admin, &tracker_id);

        (admin, contract_id, tracker_id)
    }

    fn create_course(env: &Env, tracker_id: &Address, course_id: &Symbol) {
        let tracker_client = ProgressTrackerClient::new(env, tracker_id);
        let mut module_ids = soroban_sdk::Vec::new(env);
        module_ids.push_back(Symbol::new(env, "mod_1"));
        module_ids.push_back(Symbol::new(env, "mod_2"));
        let mut quiz_ids = soroban_sdk::Vec::new(env);
        quiz_ids.push_back(Symbol::new(env, "quiz_1"));
        tracker_client.create_course(course_id, &2, &1, &module_ids, &quiz_ids);
    }

    fn complete_course_with_score(
        env: &Env,
        tracker_id: &Address,
        learner: &Address,
        course_id: &Symbol,
        score: u32,
    ) {
        let tracker_client = ProgressTrackerClient::new(env, tracker_id);
        tracker_client.enroll(learner, course_id);
        tracker_client.complete_module(learner, course_id, &Symbol::new(env, "mod_1"));
        tracker_client.complete_module(learner, course_id, &Symbol::new(env, "mod_2"));
        tracker_client.submit_quiz_score(learner, course_id, &Symbol::new(env, "quiz_1"), &score);
    }

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

    #[test]
    fn test_mint_requires_passing_score() {
        let env = Env::default();
        let (admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course_id = Symbol::new(&env, "rust_101");
        let metadata_uri = Symbol::new(&env, "ipfs_Qm123");
        let err = client
            .try_mint_credential(&admin, &learner, &course_id, &40, &metadata_uri)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::InvalidScore);
    }

    #[test]
    fn test_mint_with_valid_score() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let metadata_uri = Symbol::new(&env, "ipfs_Qm123");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);

        let cred_id = client.mint_credential(&admin, &learner, &course_id, &85, &metadata_uri);
        assert_eq!(cred_id, 1);

        let info = client.verify_credential(&cred_id);
        assert_eq!(info.learner, learner);
        assert_eq!(info.course_id, course_id);
        assert_eq!(info.score, 85);
        assert!(!info.revoked);
    }

    #[test]
    fn test_prevent_duplicate_credentials() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_Qm123");
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
    fn test_get_credentials_for_learner() {
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

        client.mint_credential(&admin, &learner, &course1, &90, &uri);
        client.mint_credential(&admin, &learner, &course2, &75, &uri);

        let creds = client.get_credentials_for(&learner, &0, &10);
        assert_eq!(creds.len(), 2);
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

        let event_count_before = env.events().all().len();
        client.revoke_credential(&cred_id);

        // Verify the revocation event indexes the learner and course (#548)
        // and carries the credential id and revoking admin in its payload.
        let events = env.events().all();
        assert_eq!(events.len(), event_count_before + 1);
        let last = events.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
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

        assert!(!client.is_credential_valid(&cred_id));

        let info = client.verify_credential(&cred_id);
        assert!(info.revoked);
    }

    #[test]
    fn test_double_revoke() {
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
        let err = client
            .try_revoke_credential(&cred_id)
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::AlreadyInitialized);
    }

    #[test]
    fn test_nonexistent_credential_invalid() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        assert!(!client.is_credential_valid(&999));
    }

    #[test]
    fn test_credential_id_increment() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner1 = Address::generate(&env);
        let learner2 = Address::generate(&env);
        env.mock_all_auths();

        let course = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");

        enrolled_and_completed_with_score(&env, &tracker_id, &learner1, &course, 80);
        complete_course_with_score(&env, &tracker_id, &learner2, &course, 90);

        let id1 = client.mint_credential(&admin, &learner1, &course, &80, &uri);
        let id2 = client.mint_credential(&admin, &learner2, &course, &90, &uri);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    #[should_panic]
    fn test_revoke_credential_without_admin_auth_fails() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        // We do not mock auths, so revoke_credential must fail admin auth check
        client.revoke_credential(&1);
    }

    #[test]
    fn test_initialize_twice_returns_already_initialized_error() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let result = client.try_initialize(&admin, &tracker_id);

        assert!(result.is_err(), "second initialize call should fail");
        let contract_err = result
            .err()
            .expect("expected an error")
            .expect("expected a typed contract error, not a host trap");
        assert_eq!(
            contract_err,
            credential_nft::ContractError::AlreadyInitialized
        );
    }

    #[test]
    fn test_mint_rejects_empty_metadata_uri() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        let learner = Address::generate(&env);
        env.mock_all_auths();
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let err = client
            .try_mint_credential(&admin, &learner, &course_id, &85, &Symbol::new(&env, ""))
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
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let err = client
            .try_mint_credential(
                &admin,
                &learner,
                &course_id,
                &85,
                &Symbol::new(&env, "ipfs_1"),
            )
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
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let err = client
            .try_mint_credential(
                &admin,
                &learner,
                &course_id,
                &85,
                &Symbol::new(&env, "ftp_metadata_hash"),
            )
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::InvalidMetadataUri);
    }

    #[test]
    fn test_mint_accepts_valid_metadata_uri() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);

        let valid_uri = Symbol::new(&env, "ipfs_Qm123ValidURI");
        let cred_id = client.mint_credential(&admin, &learner, &course_id, &85, &valid_uri);
        assert_eq!(cred_id, 1);
        let info = client.verify_credential(&cred_id);
        assert_eq!(info.metadata_uri, valid_uri);
    }

    #[test]
    fn test_renew_credential_extends_expiry() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 90);
        let metadata_uri = Symbol::new(&env, "ipfs_Qm123ValidURI");
        let cred_id = client.mint_credential(&admin, &learner, &course_id, &90, &metadata_uri);

        let before = client.verify_credential(&cred_id);
        let new_expiry = before.expires_at + 10_000;

        client.renew_credential(&cred_id, &new_expiry);

        let after = client.verify_credential(&cred_id);
        assert_eq!(after.expires_at, new_expiry);
        assert!(after.expires_at > before.expires_at);
    }

    #[test]
    fn test_revoke_credential_with_reason_is_stored_and_queryable() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 90);
        let metadata_uri = Symbol::new(&env, "ipfs_Qm123ValidURI");
        let cred_id = client.mint_credential(&admin, &learner, &course_id, &90, &metadata_uri);

        let reason = Symbol::new(&env, "fraud_detected");
        client.revoke_credential_with_reason(&cred_id, &reason);

        let stored_reason = client.get_revocation_reason(&cred_id);
        assert_eq!(stored_reason, Some(reason));
    }

    // ── Issue #242: transfer is always rejected as Soulbound ────────────────

    #[test]
    fn test_transfer_always_returns_soulbound_error() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        let other = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let metadata_uri = Symbol::new(&env, "ipfs_Qm123");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(&admin, &learner, &course_id, &85, &metadata_uri);

        let result = client.try_transfer(&learner, &other, &cred_id);
        assert!(result.is_err(), "transfer must always be rejected");
        let contract_err = result
            .err()
            .expect("expected an error")
            .expect("expected a typed contract error, not a host trap");
        assert_eq!(contract_err, credential_nft::ContractError::Soulbound);
    }

    #[test]
    fn test_transfer_rejects_even_without_auth_or_existing_credential() {
        // The rejection is unconditional: it doesn't depend on auth, on
        // `from`/`to` being real accounts, or on `credential_id` existing.
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let from = Address::generate(&env);
        let to = Address::generate(&env);
        // Intentionally no `env.mock_all_auths()` and no minted credential.

        let result = client.try_transfer(&from, &to, &999);
        let contract_err = result
            .err()
            .expect("expected an error")
            .expect("expected a typed contract error, not a host trap");
        assert_eq!(contract_err, credential_nft::ContractError::Soulbound);
    }

    #[test]
    fn test_transfer_does_not_mutate_credential_state() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        let other = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let metadata_uri = Symbol::new(&env, "ipfs_Qm123");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(&admin, &learner, &course_id, &85, &metadata_uri);

        let before = client.verify_credential(&cred_id);
        let learner_credentials_before = client.get_credentials_for(&learner, &0, &50);
        let other_credentials_before = client.get_credentials_for(&other, &0, &50);

        let _ = client.try_transfer(&learner, &other, &cred_id);

        let after = client.verify_credential(&cred_id);
        assert_eq!(before, after, "credential record must be unchanged");
        assert_eq!(
            client.get_credentials_for(&learner, &0, &50),
            learner_credentials_before,
            "original holder's credential list must be unchanged"
        );
        assert_eq!(
            client.get_credentials_for(&other, &0, &50),
            other_credentials_before,
            "intended recipient must not gain the credential"
        );
    }

    // ── Issue #240: contract initialization verification ────────────────────

    #[test]
    fn test_is_initialized_false_before_initialize() {
        let env = Env::default();
        let contract_id = env.register_contract(None, CredentialNft);
        let client = CredentialNftClient::new(&env, &contract_id);

        assert!(!client.is_initialized());
    }

    #[test]
    fn test_is_initialized_true_after_initialize() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        assert!(client.is_initialized());
    }

    #[test]
    fn test_is_initialized_does_not_mutate_state() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let before = client.contract_metadata();
        let _ = client.is_initialized();
        let _ = client.is_initialized();
        let after = client.contract_metadata();

        assert_eq!(before, after, "is_initialized must be read-only");
    }

    // ── Issue #239: storage size tracking ────────────────────────────────────

    #[test]
    fn test_storage_size_zero_before_initialize() {
        let env = Env::default();
        let contract_id = env.register_contract(None, CredentialNft);
        let client = CredentialNftClient::new(&env, &contract_id);

        assert_eq!(client.get_storage_size(), 0);
    }

    #[test]
    fn test_storage_size_increases_after_initialize() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        // initialize() writes Admin, ProgressTracker, CredentialCounter,
        // Metadata and the initial Admin role grant -- 5 distinct new keys.
        assert_eq!(client.get_storage_size(), 5);
    }

    #[test]
    fn test_storage_size_increases_on_mint() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);

        let before = client.get_storage_size();
        client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );

        // A first-time mint for a fresh learner+course creates 4 new keys:
        // Credential(id), LearnerCredentials(learner), the
        // CourseCredential(learner, course) dedup key, and
        // CourseCredentials(course). CredentialCounter already existed.
        assert_eq!(client.get_storage_size(), before + 4);
    }

    #[test]
    fn test_storage_size_unchanged_on_overwrite() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );

        let before = client.get_storage_size();
        // Revoking overwrites the existing Credential(id) entry and creates
        // exactly one new entry: Revoked(id).
        client.revoke_credential(&cred_id);
        assert_eq!(client.get_storage_size(), before + 1);
    }

    #[test]
    fn test_storage_size_does_not_change_on_read_only_calls() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );

        let before = client.get_storage_size();
        let _ = client.verify_credential(&cred_id);
        let _ = client.is_credential_valid(&cred_id);
        let _ = client.get_credentials_for(&learner, &0, &10);
        let _ = client.try_transfer(&learner, &Address::generate(&env), &cred_id);

        assert_eq!(
            client.get_storage_size(),
            before,
            "read-only calls (including the rejected transfer) must not change storage size"
        );
    }

    #[test]
    fn test_storage_size_tracks_admin_transfer_and_metadata_update() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let initial_size = client.get_storage_size();
        let new_admin = Address::generate(&env);

        client.set_admin_transfer_delay(&3_600);
        assert_eq!(client.get_storage_size(), initial_size + 1);

        let before_transfer = client.get_storage_size();
        client.transfer_admin(&new_admin);
        assert_eq!(client.get_storage_size(), before_transfer + 1);

        // Advance past the configured one-hour minimum delay.
        env.ledger().with_mut(|l| l.timestamp += 3_600);
        client.accept_admin();
        assert_eq!(client.get_storage_size(), before_transfer);

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );

        let before_update = client.get_storage_size();
        client.update_credential_metadata(&cred_id, &Symbol::new(&env, "ipfs_Qm456"));
        assert_eq!(client.get_storage_size(), before_update);
    }

    #[test]
    fn test_generate_certificate_deterministic_and_unique() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner1 = Address::generate(&env);
        let learner2 = Address::generate(&env);
        let course1 = Symbol::new(&env, "rust_101");
        let course2 = Symbol::new(&env, "solidity_101");

        create_course(&env, &tracker_id, &course1);
        complete_course_with_score(&env, &tracker_id, &learner1, &course1, 90);
        complete_course_with_score(&env, &tracker_id, &learner2, &course1, 90);

        create_course(&env, &tracker_id, &course2);
        complete_course_with_score(&env, &tracker_id, &learner1, &course2, 90);

        let cert1 = client.generate_certificate(&learner1, &course1);
        let cert1_again = client.generate_certificate(&learner1, &course1);
        let cert2 = client.generate_certificate(&learner2, &course1);
        let cert3 = client.generate_certificate(&learner1, &course2);

        // Deterministic: same inputs give same output
        assert_eq!(cert1, cert1_again);

        // Unique: different learner gives different output
        assert_ne!(cert1, cert2);

        // Unique: different course gives different output
        assert_ne!(cert1, cert3);

        // Valid credential metadata URI format
        assert!(client
            .try_mint_credential(&admin, &learner1, &course1, &90, &cert1)
            .is_ok());
    }

    // ── Issue #542: update_credential_metadata ───────────────────────────────

    #[test]
    fn test_update_credential_metadata_replaces_uri() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );
        assert_eq!(
            client.verify_credential(&cred_id).metadata_uri,
            Symbol::new(&env, "ipfs_Qm123")
        );

        let new_uri = Symbol::new(&env, "ipfs_QmReplaced");
        client.update_credential_metadata(&cred_id, &new_uri);

        assert_eq!(client.verify_credential(&cred_id).metadata_uri, new_uri);
    }

    #[test]
    fn test_update_credential_metadata_leaves_every_other_field_untouched() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 77);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &77,
            &Symbol::new(&env, "ipfs_Qm123"),
        );

        let before = client.verify_credential(&cred_id);
        let new_uri = Symbol::new(&env, "ipfs_QmReplaced");
        client.update_credential_metadata(&cred_id, &new_uri);
        let after = client.verify_credential(&cred_id);

        // Only `metadata_uri` may change; a metadata refresh must never
        // rewrite who earned the credential, for which course, with what
        // score, or when it was issued.
        assert_eq!(after.metadata_uri, new_uri);
        assert_eq!(after.learner, before.learner);
        assert_eq!(after.course_id, before.course_id);
        assert_eq!(after.score, before.score);
        assert_eq!(after.issued_at, before.issued_at);
        assert_eq!(after.revoked, before.revoked);
        assert_eq!(after.expires_at, before.expires_at);
    }

    #[test]
    fn test_update_credential_metadata_emits_indexed_event() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );

        let new_uri = Symbol::new(&env, "ipfs_QmReplaced");
        client.update_credential_metadata(&cred_id, &new_uri);

        let all = env.events().all();
        let (_, topics, data) = all.last().expect("no event emitted");
        // The credential id is a topic so "everything that happened to
        // credential N" is a server-side filter (#548); the new URI is the
        // payload.
        let topics_vec: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        assert_eq!(topics_vec.len(), 2);
        let event_name: Symbol = topics_vec.get(0).unwrap().into_val(&env);
        let topic_cred_id: u64 = topics_vec.get(1).unwrap().into_val(&env);
        assert_eq!(event_name, Symbol::new(&env, "credential_metadata_updated"));
        assert_eq!(topic_cred_id, cred_id);

        let data_vec: soroban_sdk::Vec<soroban_sdk::Val> = data.into_val(&env);
        assert_eq!(data_vec.len(), 1);
        let data_uri: Symbol = data_vec.get(0).unwrap().into_val(&env);
        assert_eq!(data_uri, new_uri);
    }

    #[test]
    fn test_update_credential_metadata_repeated_updates_use_the_latest_uri() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );

        let first_update = Symbol::new(&env, "ipfs_QmFirst");
        let second_update = Symbol::new(&env, "ipfs_QmSecond");
        client.update_credential_metadata(&cred_id, &first_update);
        client.update_credential_metadata(&cred_id, &second_update);

        // Overwriting the same storage entry must always expose the most
        // recent URI, never a stale one.
        assert_eq!(
            client.verify_credential(&cred_id).metadata_uri,
            second_update
        );
    }

    #[test]
    fn test_update_credential_metadata_rejects_unknown_credential() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();
        let err = client
            .try_update_credential_metadata(&1, &Symbol::new(&env, "ipfs_QmReplaced"))
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::NotFound);
    }

    #[test]
    fn test_update_credential_metadata_rejects_short_uri() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();
        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );
        let err = client
            .try_update_credential_metadata(&cred_id, &Symbol::new(&env, "ipfs_1"))
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::InvalidMetadataUri);
    }

    #[test]
    fn test_update_credential_metadata_rejects_malformed_uri() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();
        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );
        let err = client
            .try_update_credential_metadata(&cred_id, &Symbol::new(&env, "ftp_metadata_hash"))
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::InvalidMetadataUri);
    }

    #[test]
    fn test_update_credential_metadata_rejects_empty_uri() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();
        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );
        let err = client
            .try_update_credential_metadata(&cred_id, &Symbol::new(&env, ""))
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::InvalidMetadataUri);
    }

    #[test]
    #[should_panic]
    fn test_update_credential_metadata_requires_admin_auth() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );

        // Disable auth mocking: the credential exists, so the only reason
        // this call may fail is the missing admin authorization.
        env.set_auths(&[]);
        client.update_credential_metadata(&cred_id, &Symbol::new(&env, "ipfs_QmReplaced"));
    }

    #[test]
    fn test_update_credential_metadata_rejected_while_paused() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();
        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );
        client.pause(&admin);
        let err = client
            .try_update_credential_metadata(&cred_id, &Symbol::new(&env, "ipfs_QmReplaced"))
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::ContractPaused);
    }

    // ── Issue #543: verify_credential_with_display ───────────────────────────

    #[test]
    fn test_verify_credential_with_display_returns_empty_display_when_unset() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );

        let verification = client.verify_credential_with_display(&cred_id);

        // Nothing was ever set, so the optional display wrapper is empty
        // rather than a `None`-valued struct (see metadata.rs for why a
        // 0-or-1 Vec stands in for the option).
        assert!(verification.display.is_empty());
        assert_eq!(verification.info, client.verify_credential(&cred_id));
        assert_eq!(verification.info.learner, learner);
        assert_eq!(verification.info.course_id, course_id);
        assert!(!verification.info.revoked);
    }

    #[test]
    fn test_verify_credential_with_display_returns_all_three_fields() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );

        let image_url = Some(Symbol::new(&env, "ipfs_QmImage"));
        let description = Some(Symbol::new(&env, "rust_course_cert"));
        let issuer_name = Some(Symbol::new(&env, "chainlearn"));
        client.set_credential_display(&cred_id, &image_url, &description, &issuer_name);

        let verification = client.verify_credential_with_display(&cred_id);
        assert_eq!(verification.display.len(), 1);
        let display = verification.display.get(0).unwrap();
        assert_eq!(display.image_url, image_url);
        assert_eq!(display.description, description);
        assert_eq!(display.issuer_name, issuer_name);
    }

    #[test]
    fn test_verify_credential_with_display_supports_partially_set_fields() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );

        // Only the issuer is known; image and description stay unset.
        client.set_credential_display(
            &cred_id,
            &None,
            &None,
            &Some(Symbol::new(&env, "chainlearn")),
        );

        let verification = client.verify_credential_with_display(&cred_id);
        assert_eq!(verification.display.len(), 1);
        let display = verification.display.get(0).unwrap();
        assert!(display.image_url.is_none());
        assert!(display.description.is_none());
        assert_eq!(display.issuer_name, Some(Symbol::new(&env, "chainlearn")));
    }

    #[test]
    fn test_verify_credential_with_display_shows_latest_values_after_overwrite() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );

        client.set_credential_display(
            &cred_id,
            &Some(Symbol::new(&env, "ipfs_old_image")),
            &Some(Symbol::new(&env, "old_description")),
            &None,
        );
        client.set_credential_display(
            &cred_id,
            &Some(Symbol::new(&env, "ipfs_new_image")),
            &None,
            &None,
        );

        // A second set call overwrites the stored struct: exactly one display
        // entry is returned, holding the latest values.
        let verification = client.verify_credential_with_display(&cred_id);
        assert_eq!(verification.display.len(), 1);
        let display = verification.display.get(0).unwrap();
        assert_eq!(display.image_url, Some(Symbol::new(&env, "ipfs_new_image")));
        assert!(display.description.is_none());
        assert!(display.issuer_name.is_none());
    }

    #[test]
    fn test_verify_credential_with_display_is_scoped_to_one_credential() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_a = Symbol::new(&env, "rust_101");
        let course_b = Symbol::new(&env, "sol_201");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_a, 85);
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_b, 90);
        let id_a = client.mint_credential(
            &admin,
            &learner,
            &course_a,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );
        let id_b = client.mint_credential(
            &admin,
            &learner,
            &course_b,
            &90,
            &Symbol::new(&env, "ipfs_Qm456"),
        );

        client.set_credential_display(&id_a, &Some(Symbol::new(&env, "ipfs_img_a")), &None, &None);

        // Display data is keyed per credential: setting it for A must not
        // leak into B's verification result.
        let verification_a = client.verify_credential_with_display(&id_a);
        assert_eq!(verification_a.display.len(), 1);
        let verification_b = client.verify_credential_with_display(&id_b);
        assert!(verification_b.display.is_empty());
        assert_eq!(verification_b.info.course_id, course_b);
    }

    #[test]
    fn test_verify_credential_with_display_survives_revocation() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );
        let issuer_name = Some(Symbol::new(&env, "chainlearn"));
        client.set_credential_display(&cred_id, &None, &None, &issuer_name);

        client.revoke_credential(&cred_id);

        // Revocation flips the info's `revoked` flag but must not drop the
        // display properties a verifier still needs to render the credential.
        let verification = client.verify_credential_with_display(&cred_id);
        assert!(verification.info.revoked);
        assert_eq!(verification.display.len(), 1);
        assert_eq!(
            verification.display.get(0).unwrap().issuer_name,
            issuer_name
        );
    }

    #[test]
    fn test_verify_credential_with_display_panics_for_unknown_credential() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        // verify_credential_with_display still panics (non-Result fn) — left as-is
        assert!(!client.is_credential_valid(&999));
    }

    #[test]
    fn test_verify_credential_with_display_does_not_mutate_storage() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);
        let cred_id = client.mint_credential(
            &admin,
            &learner,
            &course_id,
            &85,
            &Symbol::new(&env, "ipfs_Qm123"),
        );
        client.set_credential_display(&cred_id, &Some(Symbol::new(&env, "ipfs_img")), &None, &None);

        let before = client.get_storage_size();
        let _ = client.verify_credential_with_display(&cred_id);
        let _ = client.verify_credential_with_display(&cred_id);
        assert_eq!(
            client.get_storage_size(),
            before,
            "verification must be read-only"
        );
    }

    // ── Issue #544: get_credentials_by_course ────────────────────────────────

    #[test]
    fn test_get_credentials_by_course_returns_ids_in_mint_order() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let course = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        let learner1 = Address::generate(&env);
        let learner2 = Address::generate(&env);
        let learner3 = Address::generate(&env);
        enrolled_and_completed_with_score(&env, &tracker_id, &learner1, &course, 80);
        complete_course_with_score(&env, &tracker_id, &learner2, &course, 85);
        complete_course_with_score(&env, &tracker_id, &learner3, &course, 90);

        let id1 = client.mint_credential(&admin, &learner1, &course, &80, &uri);
        let id2 = client.mint_credential(&admin, &learner2, &course, &85, &uri);
        let id3 = client.mint_credential(&admin, &learner3, &course, &90, &uri);

        let creds = client.get_credentials_by_course(&course);
        assert_eq!(creds.len(), 3);
        // The reverse lookup mirrors mint order, so callers can rely on a
        // stable, deterministic listing.
        assert_eq!(creds.get(0).unwrap(), id1);
        assert_eq!(creds.get(1).unwrap(), id2);
        assert_eq!(creds.get(2).unwrap(), id3);
    }

    #[test]
    fn test_get_credentials_by_course_excludes_other_courses() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let other_learner = Address::generate(&env);
        let course_a = Symbol::new(&env, "rust_101");
        let course_b = Symbol::new(&env, "sol_201");
        let uri = Symbol::new(&env, "ipfs_meta");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_a, 85);
        complete_course_with_score(&env, &tracker_id, &other_learner, &course_a, 90);
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_b, 75);

        let id_a1 = client.mint_credential(&admin, &learner, &course_a, &85, &uri);
        let id_a2 = client.mint_credential(&admin, &other_learner, &course_a, &90, &uri);
        let id_b = client.mint_credential(&admin, &learner, &course_b, &75, &uri);

        let a_creds = client.get_credentials_by_course(&course_a);
        assert_eq!(a_creds.len(), 2);
        assert!(a_creds.contains(id_a1));
        assert!(a_creds.contains(id_a2));
        assert!(!a_creds.contains(id_b));

        let b_creds = client.get_credentials_by_course(&course_b);
        assert_eq!(b_creds.len(), 1);
        assert_eq!(b_creds.get(0).unwrap(), id_b);
    }

    #[test]
    fn test_get_credentials_by_course_empty_for_course_with_no_credentials() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        // The course exists in the tracker (and another learner already holds
        // a credential for a different course), but nobody has minted for
        // `empty_course` yet.
        let other_learner = Address::generate(&env);
        let other_course = Symbol::new(&env, "rust_101");
        let empty_course = Symbol::new(&env, "sol_201");
        enrolled_and_completed_with_score(&env, &tracker_id, &other_learner, &other_course, 85);
        client.mint_credential(
            &admin,
            &other_learner,
            &other_course,
            &85,
            &Symbol::new(&env, "ipfs_meta"),
        );
        create_course(&env, &tracker_id, &empty_course);

        assert_eq!(client.get_credentials_by_course(&empty_course).len(), 0);
        // The unrelated course's credential is unaffected by the empty query.
        assert_eq!(client.get_credentials_by_course(&other_course).len(), 1);
    }

    #[test]
    fn test_get_credentials_by_course_prunes_only_the_revoked_credential() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let course = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        let learner1 = Address::generate(&env);
        let learner2 = Address::generate(&env);
        let learner3 = Address::generate(&env);
        enrolled_and_completed_with_score(&env, &tracker_id, &learner1, &course, 80);
        complete_course_with_score(&env, &tracker_id, &learner2, &course, 85);
        complete_course_with_score(&env, &tracker_id, &learner3, &course, 90);

        let id1 = client.mint_credential(&admin, &learner1, &course, &80, &uri);
        let id2 = client.mint_credential(&admin, &learner2, &course, &85, &uri);
        let id3 = client.mint_credential(&admin, &learner3, &course, &90, &uri);

        client.revoke_credential(&id2);

        // Revoking one credential prunes exactly that id from the course
        // index (#104); the surviving credentials stay listed.
        let creds = client.get_credentials_by_course(&course);
        assert_eq!(creds.len(), 2);
        assert!(creds.contains(id1));
        assert!(!creds.contains(id2));
        assert!(creds.contains(id3));
    }

    #[test]
    fn test_get_credentials_by_course_empty_after_all_credentials_revoked() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let course = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        let learner1 = Address::generate(&env);
        let learner2 = Address::generate(&env);
        enrolled_and_completed_with_score(&env, &tracker_id, &learner1, &course, 80);
        complete_course_with_score(&env, &tracker_id, &learner2, &course, 85);

        let id1 = client.mint_credential(&admin, &learner1, &course, &80, &uri);
        let id2 = client.mint_credential(&admin, &learner2, &course, &85, &uri);
        assert_eq!(client.get_credentials_by_course(&course).len(), 2);

        client.revoke_credential(&id1);
        client.revoke_credential(&id2);

        // Every id pruned leaves the index empty rather than a list of stale
        // revoked entries.
        assert_eq!(client.get_credentials_by_course(&course).len(), 0);
        // The global issued counter is untouched by revocation.
        assert_eq!(client.get_total_credentials_count(), 2);
    }

    // ── Issue #541: set/get_credential_display unit tests ──────────────────

    /// Mint a credential for `learner` in `course_id` and return its id.
    fn mint_credential(
        env: &Env,
        client: &CredentialNftClient,
        admin: &Address,
        tracker_id: &Address,
        learner: &Address,
        course_id: &Symbol,
    ) -> u64 {
        enrolled_and_completed_with_score(env, tracker_id, learner, course_id, 85);
        client.mint_credential(
            admin,
            learner,
            course_id,
            &85,
            &Symbol::new(env, "ipfs_Qm123"),
        )
    }

    #[test]
    fn test_get_credential_display_returns_none_for_unset() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        let cred_id = mint_credential(&env, &client, &admin, &tracker_id, &learner, &course_id);

        assert!(client.get_credential_display(&cred_id).is_none());
    }

    #[test]
    fn test_get_credential_display_returns_none_for_unknown_credential() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        assert!(client.get_credential_display(&999).is_none());
    }

    #[test]
    fn test_set_credential_display_all_fields() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        let cred_id = mint_credential(&env, &client, &admin, &tracker_id, &learner, &course_id);

        let image_url = Some(Symbol::new(&env, "ipfs_QmImage"));
        let description = Some(Symbol::new(&env, "rust_course_cert"));
        let issuer_name = Some(Symbol::new(&env, "chainlearn"));
        client.set_credential_display(&cred_id, &image_url, &description, &issuer_name);

        let display = client.get_credential_display(&cred_id).unwrap();
        assert_eq!(display.image_url, image_url);
        assert_eq!(display.description, description);
        assert_eq!(display.issuer_name, issuer_name);
    }

    #[test]
    fn test_set_credential_display_partial_fields() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        let cred_id = mint_credential(&env, &client, &admin, &tracker_id, &learner, &course_id);

        client.set_credential_display(
            &cred_id,
            &None,
            &None,
            &Some(Symbol::new(&env, "chainlearn")),
        );

        let display = client.get_credential_display(&cred_id).unwrap();
        assert!(display.image_url.is_none());
        assert!(display.description.is_none());
        assert_eq!(display.issuer_name, Some(Symbol::new(&env, "chainlearn")));
    }

    #[test]
    fn test_set_credential_display_updates_overwrites_previous_values() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        let cred_id = mint_credential(&env, &client, &admin, &tracker_id, &learner, &course_id);

        client.set_credential_display(
            &cred_id,
            &Some(Symbol::new(&env, "ipfs_old_image")),
            &Some(Symbol::new(&env, "old_description")),
            &Some(Symbol::new(&env, "old_issuer")),
        );
        client.set_credential_display(
            &cred_id,
            &Some(Symbol::new(&env, "ipfs_new_image")),
            &None,
            &None,
        );

        let display = client.get_credential_display(&cred_id).unwrap();
        assert_eq!(display.image_url, Some(Symbol::new(&env, "ipfs_new_image")));
        assert!(display.description.is_none());
        assert!(display.issuer_name.is_none());
    }

    #[test]
    fn test_set_credential_display_emits_event() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        let cred_id = mint_credential(&env, &client, &admin, &tracker_id, &learner, &course_id);

        let image_url = Some(Symbol::new(&env, "ipfs_QmImage"));
        let description = Some(Symbol::new(&env, "rust_course_cert"));
        let issuer_name = Some(Symbol::new(&env, "chainlearn"));
        client.set_credential_display(&cred_id, &image_url, &description, &issuer_name);

        let events = env.events().all();
        let (_, topics, data) = events.last().unwrap();
        assert_eq!(
            topics,
            (Symbol::new(&env, "credential_display_set"), cred_id).into_val(&env)
        );
        let data_vec: soroban_sdk::Vec<soroban_sdk::Val> = data.into_val(&env);
        assert_eq!(data_vec.len(), 3);
        let ev_image: Option<Symbol> = data_vec.get(0).unwrap().into_val(&env);
        let ev_desc: Option<Symbol> = data_vec.get(1).unwrap().into_val(&env);
        let ev_issuer: Option<Symbol> = data_vec.get(2).unwrap().into_val(&env);
        assert_eq!(ev_image, image_url);
        assert_eq!(ev_desc, description);
        assert_eq!(ev_issuer, issuer_name);
    }

    #[test]
    fn test_set_credential_display_requires_admin_auth() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        let cred_id = mint_credential(&env, &client, &admin, &tracker_id, &learner, &course_id);

        // Disable auth mocking: the credential exists, so the only reason this
        // call may fail is the missing admin authorization.
        env.set_auths(&[]);
        assert!(client
            .try_set_credential_display(
                &cred_id,
                &None,
                &None,
                &Some(Symbol::new(&env, "chainlearn"))
            )
            .is_err());

        // Nothing was written: the display is still unset.
        env.mock_all_auths();
        assert!(client.get_credential_display(&cred_id).is_none());
    }

    #[test]
    fn test_set_credential_display_rejects_unknown_credential() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let err = client
            .try_set_credential_display(&999, &None, &None, &Some(Symbol::new(&env, "x")))
            .err()
            .expect("should fail")
            .expect("typed error");
        assert_eq!(err, ContractError::NotFound);
    }

    #[test]
    fn test_set_credential_display_is_scoped_to_a_single_credential() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        let cred_id = mint_credential(&env, &client, &admin, &tracker_id, &learner, &course_id);

        let other_course = Symbol::new(&env, "sol_201");
        let other_cred_id =
            mint_credential(&env, &client, &admin, &tracker_id, &learner, &other_course);
        assert_ne!(cred_id, other_cred_id);

        client.set_credential_display(
            &cred_id,
            &Some(Symbol::new(&env, "ipfs_QmImage")),
            &None,
            &None,
        );

        assert!(client.get_credential_display(&cred_id).is_some());
        assert!(client.get_credential_display(&other_cred_id).is_none());
    }

    #[test]
    fn test_set_credential_display_survives_revocation() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        let cred_id = mint_credential(&env, &client, &admin, &tracker_id, &learner, &course_id);

        let issuer_name = Some(Symbol::new(&env, "chainlearn"));
        client.set_credential_display(&cred_id, &None, &None, &issuer_name);
        client.revoke_credential(&cred_id);

        let display = client.get_credential_display(&cred_id).unwrap();
        assert_eq!(display.issuer_name, issuer_name);
    }

    #[test]
    fn test_get_credentials_by_course_is_read_only() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course, 85);
        client.mint_credential(
            &admin,
            &learner,
            &course,
            &85,
            &Symbol::new(&env, "ipfs_meta"),
        );

        let before = client.get_storage_size();
        let _ = client.get_credentials_by_course(&course);
        let _ = client.get_credentials_by_course(&Symbol::new(&env, "unknown_course"));
        assert_eq!(
            client.get_storage_size(),
            before,
            "the reverse lookup must not write storage"
        );
    }
}
