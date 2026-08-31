//! Unit tests for event emissions across all ChainLearn contracts (#286).
//!
//! Verifies that every event-emitting function produces events with the
//! correct topics (indexed fields) and data payload. Events are critical
//! for indexers — these tests ensure they are emitted correctly and are
//! queryable via topic filters.

use credential_nft::{CredentialNft, CredentialNftClient};
use learn_token::{AdminRole, LearnTokenClient};
use progress_tracker::{ProgressTracker, ProgressTrackerClient};
use soroban_sdk::{
    testutils::{Address as _, Events as _},
    Address, Env, IntoVal, String as SorobanString, Symbol, Vec,
};

// ── Progress Tracker Events ─────────────────────────────────────────────────

#[cfg(test)]
mod progress_tracker_events {
    use super::*;

    fn setup(env: &Env) -> (Address, Address) {
        let admin = Address::generate(env);
        let contract_id = env.register_contract(None, ProgressTracker);
        let client = ProgressTrackerClient::new(env, &contract_id);
        client.initialize(&admin);
        (admin, contract_id)
    }

    fn create_course(env: &Env, client: &ProgressTrackerClient) -> Symbol {
        let course_id = Symbol::new(env, "event_course");
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
    fn test_create_course_emits_course_created_event() {
        let env = Env::default();
        let (_admin, contract_id) = setup(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "new_course");
        let mut module_ids = Vec::new(&env);
        module_ids.push_back(Symbol::new(&env, "m1"));
        module_ids.push_back(Symbol::new(&env, "m2"));
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(Symbol::new(&env, "q1"));

        client.create_course(&course_id, &2, &1, &module_ids, &quiz_ids);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id.clone(),
                    (Symbol::new(&env, "course_created"),).into_val(&env),
                    (course_id.clone(), 2u32, 1u32, module_ids.clone()).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_enroll_emits_enrolled_event_with_timestamp() {
        let env = Env::default();
        let (_admin, contract_id) = setup(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);
        env.mock_all_auths();

        let course_id = create_course(&env, &client);
        let learner = Address::generate(&env);

        env.ledger().with_mut(|l| l.timestamp = 99999);
        client.enroll(&learner, &course_id);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (soroban_sdk::symbol_short!("enrolled"),).into_val(&env),
                    (learner, course_id, 99999u64).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_complete_module_emits_module_completed_event() {
        let env = Env::default();
        let (_admin, contract_id) = setup(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);
        env.mock_all_auths();

        let course_id = create_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        let (_, topics, data) = last;

        let topics_vec: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        assert_eq!(topics_vec.len(), 1);
        let event_name: Symbol = topics_vec.get(0).unwrap().into_val(&env);
        assert_eq!(event_name, Symbol::new(&env, "module_completed"));

        let data_vec: soroban_sdk::Vec<soroban_sdk::Val> = data.clone();
        assert_eq!(data_vec.len(), 4);
        let data_learner: Address = data_vec.get(0).unwrap().into_val(&env);
        let data_course: Symbol = data_vec.get(1).unwrap().into_val(&env);
        let data_module: Symbol = data_vec.get(2).unwrap().into_val(&env);
        assert_eq!(data_learner, learner);
        assert_eq!(data_course, course_id);
        assert_eq!(data_module, Symbol::new(&env, "mod_1"));
    }

    #[test]
    fn test_submit_quiz_score_emits_quiz_submitted_event() {
        let env = Env::default();
        let (_admin, contract_id) = setup(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);
        env.mock_all_auths();

        let course_id = create_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &85);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        let (_, topics, data) = last;

        let topics_vec: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        assert_eq!(topics_vec.len(), 1);
        let event_name: Symbol = topics_vec.get(0).unwrap().into_val(&env);
        assert_eq!(event_name, Symbol::new(&env, "quiz_submitted"));

        let data_vec: soroban_sdk::Vec<soroban_sdk::Val> = data.clone();
        assert_eq!(data_vec.len(), 4);
        let data_score: u32 = data_vec.get(3).unwrap().into_val(&env);
        assert_eq!(data_score, 85);
    }

    #[test]
    fn test_credential_eligible_event_emitted_on_eligibility_flip() {
        let env = Env::default();
        let (_admin, contract_id) = setup(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);
        env.mock_all_auths();

        let course_id = create_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);

        // Complete all requirements — the last event must be credential_eligible
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &70);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "credential_eligible"),).into_val(&env),
                    (learner, course_id).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_retake_quiz_emits_quiz_retaken_event() {
        let env = Env::default();
        let (_admin, contract_id) = setup(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);
        env.mock_all_auths();

        let course_id = create_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_id = Symbol::new(&env, "quiz_1");
        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &quiz_id, &40);
        client.retake_quiz(&learner, &course_id, &quiz_id, &90);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "quiz_retaken"),).into_val(&env),
                    (learner, course_id, quiz_id, 40u32, 90u32).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_archive_course_emits_course_archived_event() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);
        env.mock_all_auths();

        let course_id = create_course(&env, &client);
        client.archive_course(&course_id);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "course_archived"),).into_val(&env),
                    (course_id,).into_val(&env),
                )
            ]
        );
    }
}

// ── Learn Token Events ──────────────────────────────────────────────────────

#[cfg(test)]
mod learn_token_events {
    use super::*;

    fn setup(env: &Env) -> (Address, Address, Address) {
        let admin = Address::generate(env);

        let pt_contract_id = env.register_contract(None, ProgressTracker);
        let pt_client = ProgressTrackerClient::new(env, &pt_contract_id);
        pt_client.initialize(&admin);

        let contract_id = env.register_contract(None, learn_token::LearnToken);
        let client = LearnTokenClient::new(env, &contract_id);
        client.initialize(
            &admin,
            &SorobanString::from_str(env, "CLearn"),
            &SorobanString::from_str(env, "CLRN"),
            &7,
            &pt_contract_id,
            &1_000_000_000_000_000,
        );

        (admin, contract_id, pt_contract_id)
    }

    #[test]
    fn test_mint_emits_mint_event() {
        let env = Env::default();
        let (admin, contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let recipient = Address::generate(&env);
        client.mint(&admin, &recipient, &5000);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "mint"), recipient.clone()).into_val(&env),
                    (5000i128,).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_transfer_emits_transfer_event() {
        let env = Env::default();
        let (admin, contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let from = Address::generate(&env);
        let to = Address::generate(&env);
        client.mint(&admin, &from, &10000);
        client.transfer(&from, &to, &3000);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "transfer"), from.clone(), to.clone()).into_val(&env),
                    (3000i128,).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_transfer_emits_indexed_topics() {
        let env = Env::default();
        let (admin, contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let from = Address::generate(&env);
        let to = Address::generate(&env);
        client.mint(&admin, &from, &10000);
        client.transfer(&from, &to, &3000);

        let all = env.events().all();
        let (_, topics, _) = all.last().expect("no events emitted");
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        assert_eq!(topics.len(), 3);
        let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
        let from_topic: Address = topics.get(1).unwrap().into_val(&env);
        let to_topic: Address = topics.get(2).unwrap().into_val(&env);
        assert_eq!(event_name, Symbol::new(&env, "transfer"));
        assert_eq!(from_topic, from);
        assert_eq!(to_topic, to);
    }

    #[test]
    fn test_burn_emits_burn_event() {
        let env = Env::default();
        let (admin, contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let user = Address::generate(&env);
        client.mint(&admin, &user, &10000);
        client.burn(&user, &2000);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "burn"), user.clone()).into_val(&env),
                    (2000i128,).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_approve_emits_approve_event() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        client.approve(&owner, &spender, &5000, &999999);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "approve"), owner.clone(), spender.clone()).into_val(&env),
                    (5000i128, 999999u32).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_claim_reward_emits_reward_event() {
        let env = Env::default();
        let (_admin, contract_id, pt_id) = setup(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        let pt_client = ProgressTrackerClient::new(&env, &pt_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "course_1");
        let quiz_id = Symbol::new(&env, "quiz_1");
        let mut module_ids = Vec::new(&env);
        module_ids.push_back(Symbol::new(&env, "mod_1"));
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(quiz_id.clone());
        pt_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
        pt_client.enroll(&learner, &course_id);
        pt_client.submit_quiz_score(&learner, &course_id, &quiz_id, &80);

        client.claim_reward(&learner, &course_id, &quiz_id);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "reward"), learner.clone(), course_id.clone()).into_val(&env),
                    (quiz_id, 80u32, 8000i128).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_role_granted_emits_event() {
        let env = Env::default();
        let (admin, contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let new_admin = Address::generate(&env);
        client.grant_role(&admin, &new_admin, &AdminRole::Minter);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "role_granted"), new_admin.clone()).into_val(&env),
                    (Symbol::new(&env, "Minter"),).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_role_revoked_emits_event() {
        let env = Env::default();
        let (admin, contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let user = Address::generate(&env);
        client.grant_role(&admin, &user, &AdminRole::Minter);
        client.revoke_role(&admin, &user, &AdminRole::Minter);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "role_revoked"), user.clone()).into_val(&env),
                    (Symbol::new(&env, "Minter"),).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_max_supply_updated_emits_event() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        client.set_max_supply(&8000);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "max_supply_updated"),).into_val(&env),
                    (1_000_000_000_000_000i128, 8000i128).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_vesting_created_emits_event() {
        let env = Env::default();
        let (_admin, contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &contract_id);
        env.mock_all_auths();

        let beneficiary = Address::generate(&env);
        client.create_vesting(&beneficiary, &10_000, &100, &1_000);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "vesting_created"), beneficiary.clone()).into_val(&env),
                    (10_000i128, 100u64, 1_000u64).into_val(&env),
                )
            ]
        );
    }
}

// ── Credential NFT Events ───────────────────────────────────────────────────

#[cfg(test)]
mod credential_nft_events {
    use super::*;

    fn setup(env: &Env) -> (Address, Address, Address) {
        let admin = Address::generate(env);

        let tracker_id = env.register_contract(None, ProgressTracker);
        let tracker_client = ProgressTrackerClient::new(env, &tracker_id);
        tracker_client.initialize(&admin);

        let contract_id = env.register_contract(None, CredentialNft);
        let client = CredentialNftClient::new(env, &contract_id);
        client.initialize(&admin, &tracker_id);

        (admin, contract_id, tracker_id)
    }

    fn complete_course(
        env: &Env,
        tracker_id: &Address,
        learner: &Address,
        course_id: &Symbol,
        score: u32,
    ) {
        let tracker_client = ProgressTrackerClient::new(env, tracker_id);
        let mut module_ids = Vec::new(env);
        module_ids.push_back(Symbol::new(env, "mod_1"));
        let mut quiz_ids = Vec::new(env);
        quiz_ids.push_back(Symbol::new(env, "quiz_1"));
        tracker_client.create_course(course_id, &1, &1, &module_ids, &quiz_ids);
        tracker_client.enroll(learner, course_id);
        tracker_client.complete_module(learner, course_id, &Symbol::new(&env, "mod_1"));
        tracker_client.submit_quiz_score(learner, course_id, &Symbol::new(&env, "quiz_1"), &score);
    }

    #[test]
    fn test_mint_credential_emits_credential_minted_event() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_Qm123");
        complete_course(&env, &tracker_id, &learner, &course_id, 85);

        let cred_id = client.mint_credential(&learner, &course_id, &85, &uri);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
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
    fn test_revoke_credential_emits_credential_revoked_event() {
        let env = Env::default();
        let (admin, contract_id, tracker_id) = setup(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        complete_course(&env, &tracker_id, &learner, &course_id, 80);

        let cred_id = client.mint_credential(&learner, &course_id, &80, &uri);
        client.revoke_credential(&cred_id);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
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
    }

    #[test]
    fn test_credential_revoked_event_is_indexed() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_meta");
        complete_course(&env, &tracker_id, &learner, &course_id, 80);

        let cred_id = client.mint_credential(&learner, &course_id, &80, &uri);
        client.revoke_credential(&cred_id);

        let all = env.events().all();
        let (_, topics, _) = all.last().expect("no events emitted");
        let topics_vec: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        assert_eq!(topics_vec.len(), 1);
        let event_name: Symbol = topics_vec.get(0).unwrap().into_val(&env);
        assert_eq!(event_name, Symbol::new(&env, "credential_revoked"));
    }

    #[test]
    fn test_credential_minted_event_data_contains_metadata_uri() {
        let env = Env::default();
        let (_admin, contract_id, tracker_id) = setup(&env);
        let client = CredentialNftClient::new(&env, &contract_id);
        env.mock_all_auths();

        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "rust_101");
        let uri = Symbol::new(&env, "ipfs_QmUniqueURI");
        complete_course(&env, &tracker_id, &learner, &course_id, 90);

        let cred_id = client.mint_credential(&learner, &course_id, &90, &uri);

        let all = env.events().all();
        let (_, _, data) = all.last().expect("no events emitted");
        let data_vec: soroban_sdk::Vec<soroban_sdk::Val> = data.clone();
        assert_eq!(data_vec.len(), 5);
        let data_uri: Symbol = data_vec.get(4).unwrap().into_val(&env);
        assert_eq!(data_uri, uri);

        let data_cred_id: u64 = data_vec.get(2).unwrap().into_val(&env);
        assert_eq!(data_cred_id, cred_id);
    }
}
