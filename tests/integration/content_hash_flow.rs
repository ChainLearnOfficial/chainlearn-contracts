//! Integration tests for course content hash verification (#250).
//!
//! Verifies that courses have content hashes, hashes can be set and updated,
//! hashes are queryable, and hash verification is optional during enrollment.

mod fixtures;
use fixtures::setup_chainlearn_env;

use progress_tracker::ProgressTrackerClient;
use soroban_sdk::{testutils::Address as _, testutils::Events as _, Symbol};

/// Course content hash defaults to "none" (unset).
#[test]
fn test_content_hash_defaults_to_unset() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    env.mock_all_auths();

    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);
    let course_id = fixtures::create_sample_course(env, &progress_client);

    let hash = progress_client.get_course_content_hash(&course_id);
    assert_eq!(hash, Symbol::new(env, "none"));

    // Also queryable via get_course.
    let course = progress_client.get_course(&course_id);
    assert_eq!(course.content_hash, Symbol::new(env, "none"));
}

/// Admin can set and update the content hash.
#[test]
fn test_set_and_update_content_hash() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    let admin = &setup.admin;
    env.mock_all_auths();

    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);
    let course_id = fixtures::create_sample_course(env, &progress_client);

    // Set hash.
    let hash_v1 = Symbol::new(env, "QmHashV1");
    progress_client.set_course_content_hash(&course_id, &hash_v1);
    assert_eq!(progress_client.get_course_content_hash(&course_id), hash_v1);

    // Update hash.
    let hash_v2 = Symbol::new(env, "QmHashV2");
    progress_client.set_course_content_hash(&course_id, &hash_v2);
    assert_eq!(progress_client.get_course_content_hash(&course_id), hash_v2);

    // Unset hash by setting to "none".
    progress_client.set_course_content_hash(&course_id, &Symbol::new(env, "none"));
    assert_eq!(
        progress_client.get_course_content_hash(&course_id),
        Symbol::new(env, "none")
    );
}

/// Content hash set event is emitted.
#[test]
fn test_content_hash_set_emits_event() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    let admin = &setup.admin;
    env.mock_all_auths();

    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);
    let course_id = fixtures::create_sample_course(env, &progress_client);

    let hash = Symbol::new(env, "QmTest");
    progress_client.set_course_content_hash(&course_id, &hash);

    let all = env.events().all();
    let (_, topics, _) = all.last().expect("no events emitted");
    let topics: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
    let event_name: Symbol = topics.get(0).unwrap().into_val(env);
    assert_eq!(event_name, Symbol::new(env, "content_hash_set"));
}

/// Plain enroll skips hash verification (hash is optional).
#[test]
fn test_enroll_skips_hash_verification() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    env.mock_all_auths();

    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);
    let course_id = fixtures::create_sample_course(env, &progress_client);

    // Set a hash on the course.
    progress_client.set_course_content_hash(&course_id, &Symbol::new(env, "QmSecret"));

    // Plain enroll still succeeds — verification is optional.
    let learner = Address::generate(env);
    progress_client.enroll(&learner, &course_id);

    let progress = progress_client.get_progress(&learner, &course_id);
    assert_eq!(progress.overall_progress, 0);
}

/// enroll_checked accepts a matching hash.
#[test]
fn test_enroll_checked_accepts_matching_hash() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    env.mock_all_auths();

    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);
    let course_id = fixtures::create_sample_course(env, &progress_client);

    let hash = Symbol::new(env, "QmCorrect");
    progress_client.set_course_content_hash(&course_id, &hash);

    let learner = Address::generate(env);
    progress_client.enroll_checked(&learner, &course_id, &Some(hash));

    let progress = progress_client.get_progress(&learner, &course_id);
    assert_eq!(progress.overall_progress, 0);
}

/// enroll_checked rejects a mismatched hash.
#[test]
#[should_panic(expected = "course content hash mismatch")]
fn test_enroll_checked_rejects_mismatched_hash() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    env.mock_all_auths();

    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);
    let course_id = fixtures::create_sample_course(env, &progress_client);

    progress_client.set_course_content_hash(&course_id, &Symbol::new(env, "QmReal"));

    let learner = Address::generate(env);
    progress_client.enroll_checked(&learner, &course_id, &Some(Symbol::new(env, "QmWrong")));
}

/// enroll_checked skips verification when the course has no hash set.
#[test]
fn test_enroll_checked_skips_when_hash_unset() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    env.mock_all_auths();

    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);
    let course_id = fixtures::create_sample_course(env, &progress_client);

    // No hash set — verification is skipped even if the caller provides one.
    let learner = Address::generate(env);
    progress_client.enroll_checked(&learner, &course_id, &Some(Symbol::new(env, "anything")));

    let progress = progress_client.get_progress(&learner, &course_id);
    assert_eq!(progress.overall_progress, 0);
}

/// enroll_checked with None skips verification even when hash is set.
#[test]
fn test_enroll_checked_none_skips_verification() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    env.mock_all_auths();

    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);
    let course_id = fixtures::create_sample_course(env, &progress_client);

    progress_client.set_course_content_hash(&course_id, &Symbol::new(env, "QmSet"));

    let learner = Address::generate(env);
    progress_client.enroll_checked(&learner, &course_id, &None);

    let progress = progress_client.get_progress(&learner, &course_id);
    assert_eq!(progress.overall_progress, 0);
}
