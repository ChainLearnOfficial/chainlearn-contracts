use credential_nft::{CredentialNft, CredentialNftClient};
use learn_token::{AdminRole, LearnTokenClient};
use progress_tracker::{ProgressTracker, ProgressTrackerClient};
use soroban_sdk::{
    testutils::Address as _, vec, Address, BytesN, Env, String as SorobanString, Symbol,
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
    client.set_max_supply(&10000);
}

#[test]
#[should_panic]
fn test_unauthorized_upgrade() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    client.upgrade(&BytesN::from_array(&env, &[0; 32]));
}

#[test]
#[should_panic]
fn test_unauthorized_transfer_admin() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let new_admin = Address::generate(&env);
    client.transfer_admin(&new_admin);
}

#[test]
#[should_panic]
fn test_unauthorized_cancel_admin_transfer() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    client.cancel_admin_transfer();
}

#[test]
#[should_panic]
fn test_unauthorized_set_admin_transfer_delay() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    client.set_admin_transfer_delay(&60);
}

#[test]
#[should_panic]
fn test_unauthorized_set_progress_tracker() {
    let env = Env::default();
    let (_, client) = setup_env(&env);
    let new_tracker = Address::generate(&env);
    client.set_progress_tracker(&new_tracker);
}

// ── Progress Tracker Admin Authorization Tests (#495) ─────────────────────

fn setup_progress_env(env: &Env) -> (Address, ProgressTrackerClient<'static>) {
    let admin = Address::generate(env);
    let contract_id = env.register_contract(None, ProgressTracker);
    let client = ProgressTrackerClient::new(env, &contract_id);
    client.initialize(&admin);
    (admin, client)
}

#[test]
#[should_panic]
fn test_unauthorized_progress_create_course() {
    let env = Env::default();
    let (_, client) = setup_progress_env(&env);
    let course_id = Symbol::new(&env, "rust_101");
    let module_ids = vec![&env, Symbol::new(&env, "mod_1")];
    let quiz_ids = vec![&env, Symbol::new(&env, "quiz_1")];
    client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
}

#[test]
#[should_panic]
fn test_unauthorized_progress_archive_course() {
    let env = Env::default();
    let (_, client) = setup_progress_env(&env);
    let course_id = Symbol::new(&env, "rust_101");
    let module_ids = vec![&env, Symbol::new(&env, "mod_1")];
    let quiz_ids = vec![&env, Symbol::new(&env, "quiz_1")];
    env.mock_all_auths();
    client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
    env.mock_auths(&[]);
    client.archive_course(&course_id);
}

#[test]
#[should_panic]
fn test_unauthorized_progress_set_course_content_hash() {
    let env = Env::default();
    let (_, client) = setup_progress_env(&env);
    let course_id = Symbol::new(&env, "rust_101");
    let module_ids = vec![&env, Symbol::new(&env, "mod_1")];
    let quiz_ids = vec![&env, Symbol::new(&env, "quiz_1")];
    env.mock_all_auths();
    client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
    env.mock_auths(&[]);
    client.set_course_content_hash(&course_id, &Symbol::new(&env, "hash123"));
}

#[test]
#[should_panic]
fn test_unauthorized_progress_set_course_difficulty() {
    let env = Env::default();
    let (_, client) = setup_progress_env(&env);
    let course_id = Symbol::new(&env, "rust_101");
    let module_ids = vec![&env, Symbol::new(&env, "mod_1")];
    let quiz_ids = vec![&env, Symbol::new(&env, "quiz_1")];
    env.mock_all_auths();
    client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
    env.mock_auths(&[]);
    client.set_course_difficulty(&course_id, &1);
}

#[test]
#[should_panic]
fn test_unauthorized_progress_set_course_tags() {
    let env = Env::default();
    let (_, client) = setup_progress_env(&env);
    let course_id = Symbol::new(&env, "rust_101");
    let module_ids = vec![&env, Symbol::new(&env, "mod_1")];
    let quiz_ids = vec![&env, Symbol::new(&env, "quiz_1")];
    env.mock_all_auths();
    client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
    env.mock_auths(&[]);
    let tags = vec![&env, Symbol::new(&env, "rust"), Symbol::new(&env, "smart_contracts")];
    client.set_course_tags(&course_id, &tags);
}

#[test]
#[should_panic]
fn test_unauthorized_progress_update_course_version() {
    let env = Env::default();
    let (_, client) = setup_progress_env(&env);
    let course_id = Symbol::new(&env, "rust_101");
    let module_ids = vec![&env, Symbol::new(&env, "mod_1")];
    let quiz_ids = vec![&env, Symbol::new(&env, "quiz_1")];
    env.mock_all_auths();
    client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
    env.mock_auths(&[]);
    client.update_course_version(&course_id, &2);
}

#[test]
#[should_panic]
fn test_unauthorized_progress_set_prerequisites() {
    let env = Env::default();
    let (_, client) = setup_progress_env(&env);
    let course_id = Symbol::new(&env, "rust_101");
    let prereq = Symbol::new(&env, "rust_100");
    let module_ids = vec![&env, Symbol::new(&env, "mod_1")];
    let quiz_ids = vec![&env, Symbol::new(&env, "quiz_1")];
    env.mock_all_auths();
    client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
    client.create_course(&prereq, &1, &1, &module_ids, &quiz_ids);
    env.mock_auths(&[]);
    let prereqs = vec![&env, prereq];
    client.set_prerequisites(&course_id, &prereqs);
}

#[test]
#[should_panic]
fn test_unauthorized_progress_emergency_pause() {
    let env = Env::default();
    let (_, client) = setup_progress_env(&env);
    client.emergency_pause();
}

#[test]
#[should_panic]
fn test_unauthorized_progress_unpause() {
    let env = Env::default();
    let (_, client) = setup_progress_env(&env);
    env.mock_all_auths();
    client.emergency_pause();
    env.mock_auths(&[]);
    client.unpause();
}

#[test]
#[should_panic]
fn test_unauthorized_progress_transfer_admin() {
    let env = Env::default();
    let (_, client) = setup_progress_env(&env);
    let new_admin = Address::generate(&env);
    client.transfer_admin(&new_admin);
}

#[test]
#[should_panic]
fn test_unauthorized_progress_cancel_admin_transfer() {
    let env = Env::default();
    let (_, client) = setup_progress_env(&env);
    let new_admin = Address::generate(&env);
    env.mock_all_auths();
    client.transfer_admin(&new_admin);
    env.mock_auths(&[]);
    client.cancel_admin_transfer();
}

#[test]
#[should_panic]
fn test_unauthorized_progress_set_admin_transfer_delay() {
    let env = Env::default();
    let (_, client) = setup_progress_env(&env);
    client.set_admin_transfer_delay(&3600);
}

// ── Credential NFT Admin Authorization Tests (#495) ───────────────────────

fn setup_credential_env(
    env: &Env,
) -> (
    Address,
    CredentialNftClient<'static>,
    ProgressTrackerClient<'static>,
) {
    let admin = Address::generate(env);
    let pt_contract_id = env.register_contract(None, ProgressTracker);
    let pt_client = ProgressTrackerClient::new(env, &pt_contract_id);
    pt_client.initialize(&admin);

    let cred_contract_id = env.register_contract(None, CredentialNft);
    let cred_client = CredentialNftClient::new(env, &cred_contract_id);
    cred_client.initialize(&admin, &pt_contract_id);

    (admin, cred_client, pt_client)
}

#[test]
#[should_panic]
fn test_unauthorized_credential_mint_credential() {
    let env = Env::default();
    let (_, cred_client, pt_client) = setup_credential_env(&env);
    let learner = Address::generate(&env);
    let course_id = Symbol::new(&env, "rust_101");
    let module_ids = vec![&env, Symbol::new(&env, "mod_1")];
    let quiz_ids = vec![&env, Symbol::new(&env, "quiz_1")];

    env.mock_all_auths();
    pt_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
    pt_client.enroll(&learner, &course_id);
    pt_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
    pt_client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &100);

    // Revoke mock_all_auths: calling without admin authorization must panic
    env.mock_auths(&[]);
    cred_client.mint_credential(
        &learner,
        &course_id,
        &100,
        &Symbol::new(&env, "cert123"),
    );
}

#[test]
#[should_panic]
fn test_unauthorized_credential_renew_credential() {
    let env = Env::default();
    let (_, cred_client, pt_client) = setup_credential_env(&env);
    let learner = Address::generate(&env);
    let course_id = Symbol::new(&env, "rust_101");
    let module_ids = vec![&env, Symbol::new(&env, "mod_1")];
    let quiz_ids = vec![&env, Symbol::new(&env, "quiz_1")];

    env.mock_all_auths();
    pt_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
    pt_client.enroll(&learner, &course_id);
    pt_client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
    pt_client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &100);
    cred_client.mint_credential(
        &learner,
        &course_id,
        &100,
        &Symbol::new(&env, "cert123"),
    );

    env.mock_auths(&[]);
    cred_client.renew_credential(&1u64, &99999u32);
}

#[test]
#[should_panic]
fn test_unauthorized_credential_set_credential_display() {
    let env = Env::default();
    let (_, cred_client, _) = setup_credential_env(&env);
    cred_client.set_credential_display(&1u64, &None, &None, &None);
}

#[test]
#[should_panic]
fn test_unauthorized_credential_transfer_admin() {
    let env = Env::default();
    let (_, cred_client, _) = setup_credential_env(&env);
    let new_admin = Address::generate(&env);
    cred_client.transfer_admin(&new_admin);
}

#[test]
#[should_panic]
fn test_unauthorized_credential_cancel_admin_transfer() {
    let env = Env::default();
    let (_, cred_client, _) = setup_credential_env(&env);
    let new_admin = Address::generate(&env);
    env.mock_all_auths();
    cred_client.transfer_admin(&new_admin);
    env.mock_auths(&[]);
    cred_client.cancel_admin_transfer();
}

#[test]
#[should_panic]
fn test_unauthorized_credential_set_admin_transfer_delay() {
    let env = Env::default();
    let (_, cred_client, _) = setup_credential_env(&env);
    cred_client.set_admin_transfer_delay(&3600);
}

#[test]
#[should_panic]
fn test_unauthorized_credential_emergency_pause() {
    let env = Env::default();
    let (_, cred_client, _) = setup_credential_env(&env);
    cred_client.emergency_pause();
}

#[test]
#[should_panic]
fn test_unauthorized_credential_unpause() {
    let env = Env::default();
    let (_, cred_client, _) = setup_credential_env(&env);
    env.mock_all_auths();
    cred_client.emergency_pause();
    env.mock_auths(&[]);
    cred_client.unpause();
}