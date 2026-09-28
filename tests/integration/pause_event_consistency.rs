//! Cross-contract pause/unpause event consistency (#430).
//!
//! learn-token, progress-tracker and credential-nft must all emit
//! `paused` / `unpaused` events with data `(admin: Address, timestamp: u64)`.

mod fixtures;
use fixtures::setup_chainlearn_env;

use credential_nft::CredentialNftClient;
use learn_token::LearnTokenClient;
use progress_tracker::ProgressTrackerClient;
use soroban_sdk::{testutils::Events as _, Address, Symbol};

/// Returns the (topic, data) of the last event emitted by `contract`.
fn last_event(env: &soroban_sdk::Env, contract: &Address) -> (Symbol, (Address, u64)) {
    use soroban_sdk::TryFromVal;
    let (_, topics, data) = env
        .events()
        .all()
        .iter()
        .filter(|(c, _, _)| c == contract)
        .last()
        .expect("no event emitted");
    let topic = Symbol::try_from_val(env, &topics.get(0).unwrap()).unwrap();
    let data = <(Address, u64)>::try_from_val(env, &data).unwrap();
    (topic, data)
}

/// All three contracts emit `paused` / `unpaused` with data `(admin, timestamp)`.
#[test]
fn test_pause_events_consistent_across_contracts() {
    use soroban_sdk::testutils::Ledger as _;
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    let admin = &setup.admin;
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = 1_234);

    let token = LearnTokenClient::new(env, &setup.token_contract_id);
    let progress = ProgressTrackerClient::new(env, &setup.progress_contract_id);
    let credential = CredentialNftClient::new(env, &setup.credential_contract_id);
    let paused = Symbol::new(env, "paused");
    let unpaused = Symbol::new(env, "unpaused");
    let expected = (admin.clone(), 1_234u64);

    token.pause(admin);
    assert_eq!(
        last_event(env, &setup.token_contract_id),
        (paused.clone(), expected.clone())
    );
    progress.emergency_pause();
    assert_eq!(
        last_event(env, &setup.progress_contract_id),
        (paused.clone(), expected.clone())
    );
    credential.emergency_pause();
    assert_eq!(
        last_event(env, &setup.credential_contract_id),
        (paused, expected.clone())
    );

    token.unpause(admin);
    assert_eq!(
        last_event(env, &setup.token_contract_id),
        (unpaused.clone(), expected.clone())
    );
    progress.unpause();
    assert_eq!(
        last_event(env, &setup.progress_contract_id),
        (unpaused.clone(), expected.clone())
    );
    credential.unpause();
    assert_eq!(
        last_event(env, &setup.credential_contract_id),
        (unpaused, expected)
    );
}
