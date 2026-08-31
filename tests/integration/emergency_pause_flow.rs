//! Integration tests for emergency pause across all three contracts (#281).
//!
//! Verifies that emergency pause works correctly on learn-token, progress-tracker,
//! and credential-nft: pause prevents state changes, unpause restores operations,
//! read-only functions still work while paused, and events are emitted.

mod fixtures;
use fixtures::setup_chainlearn_env;

use learn_token::{LearnTokenClient, AdminRole};
use progress_tracker::ProgressTrackerClient;
use credential_nft::CredentialNftClient;
use soroban_sdk::{testutils::Address as _, testutils::Events as _, Address, Symbol, Vec};

// ── Issue #281: learn-token emergency pause ──────────────────────────────

/// Pause prevents transfers, minting, burning, and claiming rewards.
/// Unpause restores all operations. Read-only functions still work.
/// Pause/unpause events are emitted.
#[test]
fn test_token_emergency_pause_prevents_state_changes() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    let admin = &setup.admin;
    let learner = &setup.learner;
    env.mock_all_auths();

    let token_client = LearnTokenClient::new(env, &setup.token_contract_id);
    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);

    // Mint tokens to the learner so we have something to transfer/burn.
    token_client.mint(admin, learner, &10_000);
    assert_eq!(token_client.balance(learner), 10_000);

    // ── Pause ──
    token_client.pause(admin);
    assert!(token_client.is_paused());

    // Events: paused event emitted
    {
        let all = env.events().all();
        let (_, topics, _) = all.last().expect("no events emitted");
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        let event_name: Symbol = topics.get(0).unwrap().into_val(env);
        assert_eq!(event_name, Symbol::new(env, "paused"));
    }

    // ── Transfers revert when paused ──
    let recipient = Address::generate(env);
    assert!(token_client.try_transfer(learner, &recipient, &1000).is_err());
    assert_eq!(token_client.balance(learner), 10_000); // unchanged

    // ── Minting reverts when paused ──
    assert!(token_client.try_mint(admin, learner, &5000).is_err());

    // ── Burning reverts when paused ──
    assert!(token_client.try_burn(learner, &1000).is_err());

    // ── Read-only functions still work ──
    assert_eq!(token_client.balance(learner), 10_000);
    assert_eq!(token_client.total_supply(), 10_000);
    assert_eq!(token_client.name(), soroban_sdk::String::from_str(env, "CLearn"));
    assert_eq!(token_client.symbol(), soroban_sdk::String::from_str(env, "CLRN"));
    assert_eq!(token_client.decimals(), 7);

    // ── Unpause ──
    token_client.unpause(admin);
    assert!(!token_client.is_paused());

    // Events: unpaused event emitted
    {
        let all = env.events().all();
        let (_, topics, _) = all.last().expect("no events emitted");
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        let event_name: Symbol = topics.get(0).unwrap().into_val(env);
        assert_eq!(event_name, Symbol::new(env, "unpaused"));
    }

    // ── Operations work again after unpause ──
    token_client.transfer(learner, &recipient, &1000);
    assert_eq!(token_client.balance(learner), 9_000);
    assert_eq!(token_client.balance(&recipient), 1_000);

    token_client.mint(admin, learner, &500);
    assert_eq!(token_client.balance(learner), 9_500);

    token_client.burn(learner, &500);
    assert_eq!(token_client.balance(learner), 9_000);
}

// ── Issue #281: progress-tracker emergency pause ─────────────────────────

/// Pause prevents enrollment, module completion, and quiz submissions.
/// Unpause restores operations. Read-only functions still work.
#[test]
fn test_progress_tracker_emergency_pause() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);

    // Create a course before pausing.
    let course_id = fixtures::create_sample_course(env, &progress_client);

    // ── Pause ──
    progress_client.emergency_pause();

    // ── Enrollment reverts when paused ──
    let new_learner = Address::generate(env);
    assert!(progress_client.try_enroll(&new_learner, &course_id).is_err());

    // ── Module completion reverts when paused ──
    // Enroll a learner before pausing so we can test complete_module.
    progress_client.unpause();
    progress_client.enroll(learner, &course_id);
    progress_client.emergency_pause();
    assert!(
        progress_client
            .try_complete_module(learner, &course_id, &Symbol::new(env, "mod_basics"))
            .is_err()
    );

    // ── Quiz submission reverts when paused ──
    assert!(
        progress_client
            .try_submit_quiz_score(learner, &course_id, &Symbol::new(env, "quiz_midterm"), &80)
            .is_err()
    );

    // ── Read-only functions still work ──
    let course = progress_client.get_course(&course_id);
    assert_eq!(course.course_id, course_id);
    assert_eq!(course.total_modules, 3);

    let progress = progress_client.get_progress(learner, &course_id);
    assert_eq!(progress.overall_progress, 0);

    // ── Unpause ──
    progress_client.unpause();

    // ── Operations resume ──
    progress_client.complete_module(learner, &course_id, &Symbol::new(env, "mod_basics"));
    let progress = progress_client.get_progress(learner, &course_id);
    assert!(progress.overall_progress > 0);
}

// ── Issue #281: credential-nft emergency pause ───────────────────────────

/// Pause prevents minting, revoking, and renewing credentials.
/// Unpause restores operations. Read-only functions still work.
#[test]
fn test_credential_nft_emergency_pause() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let credential_client = CredentialNftClient::new(env, &setup.credential_contract_id);
    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);

    // Complete a course so we can attempt to mint credentials.
    let course_id = Symbol::new(env, "rust_101");
    let mut module_ids = Vec::new(env);
    module_ids.push_back(Symbol::new(env, "mod_1"));
    module_ids.push_back(Symbol::new(env, "mod_2"));
    let mut quiz_ids = Vec::new(env);
    quiz_ids.push_back(Symbol::new(env, "quiz_1"));
    progress_client.create_course(&course_id, &2, &1, &module_ids, &quiz_ids);
    progress_client.enroll(learner, &course_id);
    progress_client.complete_module(learner, &course_id, &Symbol::new(env, "mod_1"));
    progress_client.complete_module(learner, &course_id, &Symbol::new(env, "mod_2"));
    progress_client.submit_quiz_score(learner, &course_id, &Symbol::new(env, "quiz_1"), &85);

    // Mint a credential before pausing so we can test revoke.
    let uri = Symbol::new(env, "ipfs_meta");
    let cred_id = credential_client.mint_credential(learner, &course_id, &85, &uri);

    // ── Pause ──
    credential_client.emergency_pause();

    // ── Minting reverts when paused ──
    let new_learner = Address::generate(env);
    assert!(
        credential_client
            .try_mint_credential(&new_learner, &course_id, &85, &uri)
            .is_err()
    );

    // ── Revoking reverts when paused ──
    assert!(credential_client.try_revoke_credential(&cred_id).is_err());

    // ── Read-only functions still work ──
    let info = credential_client.verify_credential(&cred_id);
    assert_eq!(info.learner, *learner);
    assert_eq!(info.course_id, course_id);
    assert!(!info.revoked);

    assert!(credential_client.is_credential_valid(&cred_id));
    assert_eq!(credential_client.get_credential_count(learner), 1);

    // ── Unpause ──
    credential_client.unpause();

    // ── Operations resume ──
    let new_cred_id = credential_client.mint_credential(learner, &course_id, &85, &uri);
    assert_eq!(new_cred_id, cred_id + 1);
}

// ── Issue #281: Pauser role enforcement ──────────────────────────────────

/// A Minter cannot pause, a Pauser cannot mint.
#[test]
fn test_pause_role_enforcement() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    let admin = &setup.admin;
    let learner = &setup.learner;
    env.mock_all_auths();

    let token_client = LearnTokenClient::new(env, &setup.token_contract_id);

    let minter = Address::generate(env);
    let pauser = Address::generate(env);

    token_client.grant_role(admin, &minter, &AdminRole::Minter);
    token_client.grant_role(admin, &pauser, &AdminRole::Pauser);

    // Minter cannot pause.
    assert!(token_client.try_pause(&minter).is_err());

    // Pauser can pause and unpause.
    token_client.pause(&pauser);
    assert!(token_client.is_paused());
    token_client.unpause(&pauser);

    // Pauser cannot mint.
    assert!(token_client.try_mint(&pauser, learner, &1000).is_err());
}
