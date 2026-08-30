//! Unit tests for the credential-nft contract.

use credential_nft::{CredentialNft, CredentialNftClient};
use progress_tracker::{ProgressTracker, ProgressTrackerClient};
use soroban_sdk::{
    testutils::{Address as _, Events as _},
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
    #[should_panic(expected = "score 40 below minimum threshold 50")]
    fn test_mint_requires_passing_score() {
        let env = Env::default();
        let (_admin, contract_id, _tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let metadata_uri = Symbol::new(&env, "ipfs_Qm123");

        client.mint_credential(&learner, &course_id, &40, &metadata_uri);
    }

    #[test]
    fn test_mint_with_valid_score() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let metadata_uri = Symbol::new(&env, "ipfs_Qm123");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);

        let cred_id = client.mint_credential(&learner, &course_id, &85, &metadata_uri);
        assert_eq!(cred_id, 1);

        let info = client.verify_credential(&cred_id);
        assert_eq!(info.learner, learner);
        assert_eq!(info.course_id, course_id);
        assert_eq!(info.score, 85);
        assert!(!info.revoked);
    }

    #[test]
    #[should_panic(expected = "credential already exists for this learner and course")]
    fn test_prevent_duplicate_credentials() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_Qm123");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 90);

        client.mint_credential(&learner, &course_id, &90, &uri);
        client.mint_credential(&learner, &course_id, &90, &uri);
    }

    #[test]
    fn test_get_credentials_for_learner() {
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

        client.mint_credential(&learner, &course1, &90, &uri);
        client.mint_credential(&learner, &course2, &75, &uri);

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

        let cred_id = client.mint_credential(&learner, &course_id, &80, &uri);
        assert!(client.is_credential_valid(&cred_id));

        let event_count_before = env.events().all().len();
        client.revoke_credential(&cred_id);

        // Verify the revocation event includes the admin address.
        let events = env.events().all();
        assert_eq!(events.len(), event_count_before + 1);
        let last = events.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id.clone(),
                    (Symbol::new(&env, "credential_revoked"),).into_val(&env),
                    (learner, course_id, cred_id, admin).into_val(&env),
                )
            ]
        );

        assert!(!client.is_credential_valid(&cred_id));

        let info = client.verify_credential(&cred_id);
        assert!(info.revoked);
    }

    #[test]
    #[should_panic(expected = "credential already revoked")]
    fn test_double_revoke() {
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
        client.revoke_credential(&cred_id);
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
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner1 = Address::generate(&env);
        let learner2 = Address::generate(&env);
        env.mock_all_auths();

        let course = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");

        enrolled_and_completed_with_score(&env, &tracker_id, &learner1, &course, 80);
        complete_course_with_score(&env, &tracker_id, &learner2, &course, 90);

        let id1 = client.mint_credential(&learner1, &course, &80, &uri);
        let id2 = client.mint_credential(&learner2, &course, &90, &uri);
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
    #[should_panic(expected = "metadata_uri cannot be empty")]
    fn test_mint_rejects_empty_metadata_uri() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);

        let empty_uri = Symbol::new(&env, "");
        client.mint_credential(&learner, &course_id, &85, &empty_uri);
    }

    #[test]
    #[should_panic(expected = "metadata_uri too short: minimum length is 8")]
    fn test_mint_rejects_too_short_metadata_uri() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);

        let short_uri = Symbol::new(&env, "ipfs_1");
        client.mint_credential(&learner, &course_id, &85, &short_uri);
    }

    #[test]
    #[should_panic(expected = "metadata_uri is malformed: must start with a valid URI scheme")]
    fn test_mint_rejects_malformed_metadata_uri() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);

        let malformed_uri = Symbol::new(&env, "ftp_metadata_hash");
        client.mint_credential(&learner, &course_id, &85, &malformed_uri);
    }

    #[test]
    fn test_mint_accepts_valid_metadata_uri() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        enrolled_and_completed_with_score(&env, &tracker_id, &learner, &course_id, 85);

        let valid_uri = Symbol::new(&env, "ipfs_Qm123ValidURI");
        let cred_id = client.mint_credential(&learner, &course_id, &85, &valid_uri);
        assert_eq!(cred_id, 1);
        let info = client.verify_credential(&cred_id);
        assert_eq!(info.metadata_uri, valid_uri);
    }
}
