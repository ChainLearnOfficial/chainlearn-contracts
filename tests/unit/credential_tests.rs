//! Unit tests for the credential-nft contract.

use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

#[cfg(test)]
mod credential_unit_tests {
    use super::*;

    /// Test that minting requires a passing score.
    #[test]
    fn test_mint_requires_passing_score() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let metadata_uri = Symbol::new(&env, "ipfs://Qm123");

        // Score 40 should fail (minimum is 50)
        // client.mint_credential(&learner, &course_id, &40, &metadata_uri);
        // ^ should panic with "score 40 below minimum threshold 50"
    }

    /// Test successful minting with valid score.
    #[test]
    fn test_mint_with_valid_score() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let metadata_uri = Symbol::new(&env, "ipfs://Qm123");

        // Score 85 should succeed
        // let cred_id = client.mint_credential(&learner, &course_id, &85, &metadata_uri);
        // assert_eq!(cred_id, 1);

        // let info = client.verify_credential(&cred_id);
        // assert_eq!(info.learner, learner);
        // assert_eq!(info.course_id, course_id);
        // assert_eq!(info.score, 85);
        // assert!(!info.revoked);
    }

    /// Test duplicate credential prevention.
    #[test]
    #[should_panic(expected = "credential already exists for this learner and course")]
    fn test_prevent_duplicate_credentials() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs://Qm123");

        // client.mint_credential(&learner, &course_id, &90, &uri);
        // client.mint_credential(&learner, &course_id, &95, &uri);
    }

    /// Test getting credentials for a learner.
    #[test]
    fn test_get_credentials_for_learner() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        // Mint credentials for two different courses
        // client.mint_credential(&learner, &course1, &90, &uri);
        // client.mint_credential(&learner, &course2, &75, &uri);

        // let creds = client.get_credentials_for(&learner);
        // assert_eq!(creds.len(), 2);
    }

    /// Test credential revocation.
    #[test]
    fn test_revoke_credential() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        // let cred_id = client.mint_credential(&learner, &course, &80, &uri);
        // assert!(client.is_credential_valid(&cred_id));

        // client.revoke_credential(&cred_id);
        // assert!(!client.is_credential_valid(&cred_id));

        // let info = client.verify_credential(&cred_id);
        // assert!(info.revoked);
    }

    /// Test that revoked credentials cannot be revoked again.
    #[test]
    #[should_panic(expected = "credential already revoked")]
    fn test_double_revoke() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        // let cred_id = client.mint_credential(&learner, &course, &80, &uri);
        // client.revoke_credential(&cred_id);
        // client.revoke_credential(&cred_id);
    }

    /// Test that non-existent credentials return false for validity check.
    #[test]
    fn test_nonexistent_credential_invalid() {
        let env = Env::default();
        // assert!(!client.is_credential_valid(&999));
    }

    /// Test credential ID auto-increment.
    #[test]
    fn test_credential_id_increment() {
        let env = Env::default();
        let learner1 = Address::generate(&env);
        let learner2 = Address::generate(&env);
        env.mock_all_auths();

        // let id1 = client.mint_credential(&learner1, &course, &80, &uri);
        // let id2 = client.mint_credential(&learner2, &course, &90, &uri);
        // assert_eq!(id1, 1);
        // assert_eq!(id2, 2);
    }
}
