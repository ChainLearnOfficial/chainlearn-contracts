#![cfg(test)]

use credential_nft::{CredentialNft, CredentialNftClient};
use learn_token::{AdminRole, LearnTokenClient};
use progress_tracker::{ProgressTracker, ProgressTrackerClient};
use soroban_sdk::{
    testutils::Address as _, Address, Env, String as SorobanString, Symbol, BytesN, Vec,
};

fn setup_env(env: &Env) -> (Address, LearnTokenClient<'static>) {
    let admin = Address::generate(env);
    let pt_contract_id = env.register_contract(None, ProgressTracker);
    let contract_id = env.register_contract(None, learn_token::LearnToken);
    let client = LearnTokenClient::new(env, &contract_id);
    
    client.initialize(
        &admin,
        &SorobanString::from_str(env, "ChainLearn"),
        &SorobanString::from_str(env, "CLRN"),
        &7,
        &pt_contract_id,
        &1_000_000,
    );
    (admin, client)
}

fn setup_progress_tracker(env: &Env) -> (Address, Address, ProgressTrackerClient<'static>) {
    let admin = Address::generate(env);
    let contract_id = env.register_contract(None, ProgressTracker);
    let client = ProgressTrackerClient::new(env, &contract_id);
    client.initialize(&admin);
    (admin, contract_id, client)
}

fn setup_credential_nft(env: &Env) -> (Address, Address, CredentialNftClient<'static>) {
    let admin = Address::generate(env);
    let tracker_id = env.register_contract(None, ProgressTracker);
    let tracker_client = ProgressTrackerClient::new(env, &tracker_id);
    tracker_client.initialize(&admin);
    
    let contract_id = env.register_contract(None, CredentialNft);
    let client = CredentialNftClient::new(env, &contract_id);
    client.initialize(&admin, &tracker_id);
    
    (admin, contract_id, client)
}

fn create_test_course(env: &Env, client: &ProgressTrackerClient) -> Symbol {
    let course_id = Symbol::new(env, "rust_101");
    let mut module_ids = Vec::new(env);
    module_ids.push_back(Symbol::new(env, "mod_1"));
    module_ids.push_back(Symbol::new(env, "mod_2"));
    let mut quiz_ids = Vec::new(env);
    quiz_ids.push_back(Symbol::new(env, "quiz_1"));
    quiz_ids.push_back(Symbol::new(env, "quiz_2"));
    client.create_course(&course_id, &2, &2, &module_ids, &quiz_ids);
    course_id
}

#[test]
#[should_panic]
fn test_unauthorized_mint() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let recipient = Address::generate(&env);
    client.mint(&malicious, &recipient, &1000);
}

#[test]
#[should_panic]
fn test_unauthorized_pause() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    client.pause(&malicious);
}

#[test]
#[should_panic]
fn test_unauthorized_grant_role() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let new_admin = Address::generate(&env);
    client.grant_role(&malicious, &new_admin, &AdminRole::Admin);
}

#[test]
#[should_panic]
fn test_unauthorized_revoke_role() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let existing_admin = Address::generate(&env);
    client.revoke_role(&malicious, &existing_admin, &AdminRole::Admin);
}

#[test]
#[should_panic]
fn test_unauthorized_add_admin() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let admin_info = learn_token::AdminInfo {
        address: new_admin,
        role: AdminRole::Admin,
    };
    client.add_admin(&malicious, &admin_info);
}

#[test]
#[should_panic]
fn test_unauthorized_remove_admin() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let admin_to_remove = Address::generate(&env);
    let admin_info = learn_token::AdminInfo {
        address: admin_to_remove,
        role: AdminRole::Admin,
    };
    client.remove_admin(&malicious, &admin_info);
}

#[test]
#[should_panic]
fn test_unauthorized_execute_multisig() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let co_signer = Address::generate(&env);
    client.execute_multisig_op(&malicious, &co_signer, &Symbol::new(&env, "test"));
}

#[test]
#[should_panic]
fn test_unauthorized_upgrade_multisig() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let co_signer = Address::generate(&env);
    client.upgrade_multisig(&malicious, &co_signer, &BytesN::from_array(&env, &[0; 32]));
}

#[test]
#[should_panic]
fn test_unauthorized_pause_unpaused() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    client.unpause(&malicious);
}

#[test]
#[should_panic]
fn test_unauthorized_set_max_supply() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    client.set_max_supply(&10000);
}

#[test]
#[should_panic]
fn test_unauthorized_upgrade() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    client.upgrade(&BytesN::from_array(&env, &[0; 32]));
}

#[test]
#[should_panic]
fn test_unauthorized_transfer_admin() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let new_admin = Address::generate(&env);
    client.transfer_admin(&new_admin);
}

#[test]
#[should_panic]
fn test_unauthorized_cancel_admin_transfer() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    client.cancel_admin_transfer();
}

#[test]
#[should_panic]
fn test_unauthorized_set_admin_transfer_delay() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    client.set_admin_transfer_delay(&60);
}

#[test]
#[should_panic]
fn test_unauthorized_set_progress_tracker() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let malicious = Address::generate(&env);
    let new_tracker = Address::generate(&env);
    client.set_progress_tracker(&new_tracker);
}

// ── Progress Tracker Admin Function Auth Tests ──

#[test]
#[should_panic]
fn test_unauthorized_create_course() {
    let env = Env::default();
    let (_admin, _contract_id, client) = setup_progress_tracker(&env);
    let course_id = Symbol::new(&env, "new_course");
    let mut module_ids = Vec::new(&env);
    module_ids.push_back(Symbol::new(&env, "mod_1"));
    let mut quiz_ids = Vec::new(&env);
    quiz_ids.push_back(Symbol::new(&env, "quiz_1"));
    // Admin not authorized - should fail
    client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
}

#[test]
#[should_panic]
fn test_unauthorized_archive_course() {
    let env = Env::default();
    let (admin, _contract_id, client) = setup_progress_tracker(&env);
    // Setup course with admin auth
    env.mock_all_auths();
    let course_id = create_test_course(&env, &client);
    // Now test archive_course WITHOUT admin auth
    env.set_auths(&[]);
    env.mock_auths(&[]);
    client.archive_course(&course_id);
}

#[test]
#[should_panic]
fn test_unauthorized_set_course_content_hash() {
    let env = Env::default();
    let (admin, _contract_id, client) = setup_progress_tracker(&env);
    // Setup course with admin auth
    env.mock_all_auths();
    let course_id = create_test_course(&env, &client);
    // Now test set_course_content_hash WITHOUT admin auth
    env.set_auths(&[]);
    env.mock_auths(&[]);
    client.set_course_content_hash(&course_id, &Symbol::new(&env, "hash1"));
}

#[test]
#[should_panic]
fn test_unauthorized_set_course_difficulty() {
    let env = Env::default();
    let (admin, _contract_id, client) = setup_progress_tracker(&env);
    // Setup course with admin auth
    env.mock_all_auths();
    let course_id = create_test_course(&env, &client);
    // Now test set_course_difficulty WITHOUT admin auth
    env.set_auths(&[]);
    env.mock_auths(&[]);
    client.set_course_difficulty(&course_id, &0);
}

#[test]
#[should_panic]
fn test_unauthorized_set_course_tags() {
    let env = Env::default();
    let (admin, _contract_id, client) = setup_progress_tracker(&env);
    // Setup course with admin auth
    env.mock_all_auths();
    let course_id = create_test_course(&env, &client);
    let mut tags = Vec::new(&env);
    tags.push_back(Symbol::new(&env, "tag1"));
    // Now test set_course_tags WITHOUT admin auth
    env.set_auths(&[]);
    env.mock_auths(&[]);
    client.set_course_tags(&course_id, &tags);
}

#[test]
#[should_panic]
fn test_unauthorized_update_course_version() {
    let env = Env::default();
    let (admin, _contract_id, client) = setup_progress_tracker(&env);
    // Setup course with admin auth
    env.mock_all_auths();
    let course_id = create_test_course(&env, &client);
    // Now test update_course_version WITHOUT admin auth
    env.set_auths(&[]);
    env.mock_auths(&[]);
    client.update_course_version(&course_id, &2);
}

#[test]
#[should_panic]
fn test_unauthorized_set_prerequisites() {
    let env = Env::default();
    let (admin, _contract_id, client) = setup_progress_tracker(&env);
    // Setup course with admin auth
    env.mock_all_auths();
    let course_id = create_test_course(&env, &client);
    let mut prereqs = Vec::new(&env);
    prereqs.push_back(Symbol::new(&env, "prereq_1"));
    // Now test set_prerequisites WITHOUT admin auth
    client.set_prerequisites(&course_id, &prereqs);
}

#[test]
#[should_panic]
fn test_unauthorized_pause_progress_tracker() {
    let env = Env::default();
    let (_admin, _contract_id, client) = setup_progress_tracker(&env);
    // Admin not authorized - should fail
    let random = Address::generate(&env);
    client.pause(&random);
}

// ── Credential NFT Admin Function Auth Tests ──

#[test]
#[should_panic]
fn test_unauthorized_mint_credential() {
    let env = Env::default();
    let (_admin, _contract_id, client) = setup_credential_nft(&env);
    let learner = Address::generate(&env);
    let course_id = Symbol::new(&env, "rust_101");
    let metadata_uri = Symbol::new(&env, "ipfs_Qm123");
    
    // First set up a course and complete it with admin auth
    let (_pt_admin, _pt_contract_id, pt_client) = setup_progress_tracker(&env);
    env.mock_all_auths();
    let mut module_ids = Vec::new(&env);
    module_ids.push_back(Symbol::new(&env, "mod_1"));
    module_ids.push_back(Symbol::new(&env, "mod_2"));
    let mut quiz_ids = Vec::new(&env);
    quiz_ids.push_back(Symbol::new(&env, "quiz_1"));
    pt_client.create_course(&course_id, &2, &1, &module_ids, &quiz_ids);
    pt_client.enroll(&learner, &course_id);
    pt_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
    pt_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
    pt_client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &85);
    
    // Now try to mint WITHOUT admin authorization (no mock_all_auths)
    // The credential-nft mint_credential requires BOTH admin and learner auth
    let nobody = Address::generate(&env);
    client.mint_credential(&nobody, &learner, &course_id, &85, &metadata_uri);
}

#[test]
#[should_panic]
fn test_unauthorized_revoke_credential() {
    let env = Env::default();
    let (_admin, _contract_id, client) = setup_credential_nft(&env);
    
    // First mint a credential with the real admin (using mock_all_auths for setup)
    let learner = Address::generate(&env);
    let course_id = Symbol::new(&env, "rust_101");
    let metadata_uri = Symbol::new(&env, "ipfs_Qm123");
    
    let (_pt_admin, _pt_contract_id, pt_client) = setup_progress_tracker(&env);
    env.mock_all_auths();
    let mut module_ids = Vec::new(&env);
    module_ids.push_back(Symbol::new(&env, "mod_1"));
    module_ids.push_back(Symbol::new(&env, "mod_2"));
    let mut quiz_ids = Vec::new(&env);
    quiz_ids.push_back(Symbol::new(&env, "quiz_1"));
    pt_client.create_course(&course_id, &2, &1, &module_ids, &quiz_ids);
    pt_client.enroll(&learner, &course_id);
    pt_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
    pt_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
    pt_client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &85);
    
    let cred_id = client.mint_credential(&_admin, &learner, &course_id, &85, &metadata_uri);
    
    // Now try to revoke WITHOUT admin authorization (no mock_all_auths)
    client.revoke_credential(&cred_id);
}

#[test]
#[should_panic]
fn test_unauthorized_renew_credential() {
    let env = Env::default();
    let (_admin, _contract_id, client) = setup_credential_nft(&env);
    
    // First mint a credential with the real admin (using mock_all_auths for setup)
    let learner = Address::generate(&env);
    let course_id = Symbol::new(&env, "rust_101");
    let metadata_uri = Symbol::new(&env, "ipfs_Qm123");
    
    let (_pt_admin, _pt_contract_id, pt_client) = setup_progress_tracker(&env);
    env.mock_all_auths();
    let mut module_ids = Vec::new(&env);
    module_ids.push_back(Symbol::new(&env, "mod_1"));
    module_ids.push_back(Symbol::new(&env, "mod_2"));
    let mut quiz_ids = Vec::new(&env);
    quiz_ids.push_back(Symbol::new(&env, "quiz_1"));
    pt_client.create_course(&course_id, &2, &1, &module_ids, &quiz_ids);
    pt_client.enroll(&learner, &course_id);
    pt_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
    pt_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
    pt_client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &85);
    
    let cred_id = client.mint_credential(&_admin, &learner, &course_id, &85, &metadata_uri);
    
    // Now try to renew WITHOUT admin authorization (no mock_all_auths)
    client.renew_credential(&cred_id, &1000);
}

#[test]
#[should_panic]
fn test_unauthorized_set_credential_display() {
    let env = Env::default();
    let (_admin, _contract_id, client) = setup_credential_nft(&env);
    
    // First mint a credential with the real admin (using mock_all_auths for setup)
    let learner = Address::generate(&env);
    let course_id = Symbol::new(&env, "rust_101");
    let metadata_uri = Symbol::new(&env, "ipfs_Qm123");
    
    let (_pt_admin, _pt_contract_id, pt_client) = setup_progress_tracker(&env);
    env.mock_all_auths();
    let mut module_ids = Vec::new(&env);
    module_ids.push_back(Symbol::new(&env, "mod_1"));
    module_ids.push_back(Symbol::new(&env, "mod_2"));
    let mut quiz_ids = Vec::new(&env);
    quiz_ids.push_back(Symbol::new(&env, "quiz_1"));
    pt_client.create_course(&course_id, &2, &1, &module_ids, &quiz_ids);
    pt_client.enroll(&learner, &course_id);
    pt_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
    pt_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
    pt_client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &85);
    
    let cred_id = client.mint_credential(&_admin, &learner, &course_id, &85, &metadata_uri);
    
    // Now try to set display WITHOUT admin authorization (no mock_all_auths)
    client.set_credential_display(&cred_id, &None, &None, &None);
}

#[test]
#[should_panic]
fn test_unauthorized_transfer_admin_credential_nft() {
    let env = Env::default();
    let (_admin, _contract_id, client) = setup_credential_nft(&env);
    let new_admin = Address::generate(&env);
    // Admin not authorized - should fail
    client.transfer_admin(&new_admin);
}

#[test]
#[should_panic]
fn test_unauthorized_pause_credential_nft() {
    let env = Env::default();
    let (_admin, _contract_id, client) = setup_credential_nft(&env);
    // Admin not authorized - should fail
    let random = Address::generate(&env);
    client.pause(&random);
}