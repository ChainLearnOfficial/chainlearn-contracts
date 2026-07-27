//! Unit tests for the credential-nft contract.

use credential_nft::{CredentialNft, CredentialNftClient};
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

#[cfg(test)]
mod credential_unit_tests {
    use super::*;

    fn setup_contract(env: &Env) -> (Address, Address) {
        let admin = Address::generate(env);
        let contract_id = env.register(CredentialNft, ());
        let client = CredentialNftClient::new(env, &contract_id);
        client.initialize(&admin);
        (admin, contract_id)
    }

    #[test]
    #[should_panic(expected = "score 40 below minimum threshold 50")]
    fn test_mint_requires_passing_score() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let metadata_uri = Symbol::new(&env, "ipfs://Qm123");

        client.mint_credential(&learner, &course_id, &40, &metadata_uri);
    }

    #[test]
    fn test_mint_with_valid_score() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let metadata_uri = Symbol::new(&env, "ipfs://Qm123");

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
        let (_admin, contract_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs://Qm123");

        client.mint_credential(&learner, &course_id, &90, &uri);
        client.mint_credential(&learner, &course_id, &95, &uri);
    }

    #[test]
    fn test_get_credentials_for_learner() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course1 = Symbol::new(&env, "rust_101");
        let course2 = Symbol::new(&env, "sol_201");
        let uri = Symbol::new(&env, "ipfs://meta");

        client.mint_credential(&learner, &course1, &90, &uri);
        client.mint_credential(&learner, &course2, &75, &uri);

        let creds = client.get_credentials_for(&learner);
        assert_eq!(creds.len(), 2);
    }

    #[test]
    fn test_revoke_credential() {
        let env = Env::default();
        let (admin, contract_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs://meta");

        let cred_id = client.mint_credential(&learner, &course_id, &80, &uri);
        assert!(client.is_credential_valid(&cred_id));

        let event_count_before = env.events().all().len();
        client.revoke_credential(&cred_id);

        // Verify the revocation event includes the admin address.
        let events = env.events().all();
        assert_eq!(events.len(), event_count_before + 1);
        let (_, topics, data) = events.get(events.len() - 1).unwrap();
        let event_sym: Symbol = topics.get(0).unwrap();
        assert_eq!(event_sym, Symbol::new(&env, "credential_revoked"));
        // data: (learner, course_id, credential_id, admin)
        let event_learner: Address = data.get(0).unwrap();
        let event_admin: Address = data.get(3).unwrap();
        assert_eq!(event_learner, learner);
        assert_eq!(event_admin, admin);

        assert!(!client.is_credential_valid(&cred_id));

        let info = client.verify_credential(&cred_id);
        assert!(info.revoked);
    }

    #[test]
    #[should_panic(expected = "credential already revoked")]
    fn test_double_revoke() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs://meta");

        let cred_id = client.mint_credential(&learner, &course_id, &80, &uri);
        client.revoke_credential(&cred_id);
        client.revoke_credential(&cred_id);
    }

    #[test]
    fn test_nonexistent_credential_invalid() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        assert!(!client.is_credential_valid(&999));
    }

    #[test]
    fn test_credential_id_increment() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = CredentialNftClient::new(&env, &contract_id);

        let learner1 = Address::generate(&env);
        let learner2 = Address::generate(&env);
        env.mock_all_auths();

        let course = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs://meta");

        let id1 = client.mint_credential(&learner1, &course, &80, &uri);
        let id2 = client.mint_credential(&learner2, &course, &90, &uri);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }
}
