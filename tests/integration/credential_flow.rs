//! Integration tests for the end-to-end credential flow.
//!
//! Tests the full journey: enroll -> complete -> quiz -> credential mint -> verify.

mod fixtures;

use credential_nft::CredentialNftClient;
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

#[test]
fn test_end_to_end_credential_flow() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let credential_client = CredentialNftClient::new(env, &setup.credential_contract_id);
    let progress_client =
        progress_tracker::ProgressTrackerClient::new(env, &setup.progress_contract_id);

    let course_id = fixtures::create_sample_course(env, &progress_client);

    // Enroll and complete all modules + quizzes
    fixtures::complete_full_course(env, learner, &course_id, &progress_client);

    // Mint credential (average score = (85+75)/2 = 80)
    let metadata_uri = Symbol::new(env, "ipfs://QmCredential123");
    let cred_id = credential_client.mint_credential(learner, &course_id, &80, &metadata_uri);

    // Verify
    let info = credential_client.verify_credential(&cred_id);
    assert_eq!(info.learner, *learner);
    assert_eq!(info.course_id, course_id);
    assert_eq!(info.score, 80);
    assert!(!info.revoked);
}

#[test]
fn test_public_verification() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let credential_client = CredentialNftClient::new(env, &setup.credential_contract_id);

    let course_id = Symbol::new(env, "rust_101");
    let uri = Symbol::new(env, "ipfs://Qm123");

    let cred_id = credential_client.mint_credential(learner, &course_id, &90, &uri);

    // Anyone can verify (no auth required for reads)
    let info = credential_client.verify_credential(&cred_id);
    assert_eq!(info.score, 90);
}

#[test]
fn test_multiple_course_credentials() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let credential_client = CredentialNftClient::new(env, &setup.credential_contract_id);

    let uri = Symbol::new(env, "ipfs://meta");

    let cred1 = credential_client.mint_credential(learner, &Symbol::new(env, "rust_101"), &85, &uri);
    let cred2 = credential_client.mint_credential(learner, &Symbol::new(env, "sol_201"), &90, &uri);

    let creds = credential_client.get_credentials_for(learner);
    assert_eq!(creds.len(), 2);
    assert!(creds.contains(&cred1));
    assert!(creds.contains(&cred2));
}

#[test]
fn test_revoked_credential_shows_revoked_status() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let credential_client = CredentialNftClient::new(env, &setup.credential_contract_id);

    let course_id = Symbol::new(env, "rust_101");
    let uri = Symbol::new(env, "ipfs://meta");

    let cred_id = credential_client.mint_credential(learner, &course_id, &80, &uri);
    assert!(credential_client.is_credential_valid(&cred_id));

    credential_client.revoke_credential(&cred_id);

    let info = credential_client.verify_credential(&cred_id);
    assert!(info.revoked);
    assert!(!credential_client.is_credential_valid(&cred_id));
}

#[test]
fn test_credential_metadata_uri() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let credential_client = CredentialNftClient::new(env, &setup.credential_contract_id);

    let course_id = Symbol::new(env, "rust_101");
    let expected_uri = Symbol::new(env, "ipfs://QmTestMetadata");

    let cred_id = credential_client.mint_credential(learner, &course_id, &75, &expected_uri);

    let info = credential_client.verify_credential(&cred_id);
    assert_eq!(info.metadata_uri, expected_uri);
}

#[test]
fn test_credential_has_issuance_timestamp() {
    let setup = fixtures::setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let credential_client = CredentialNftClient::new(env, &setup.credential_contract_id);

    let course_id = Symbol::new(env, "rust_101");
    let uri = Symbol::new(env, "ipfs://meta");

    let cred_id = credential_client.mint_credential(learner, &course_id, &80, &uri);

    let info = credential_client.verify_credential(&cred_id);
    assert!(info.issued_at > 0);
}
