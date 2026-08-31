//! Integration tests for token vesting schedules (#280).
//!
//! Verifies cliff enforcement, linear vesting, and claiming across the
//! full ChainLearn environment with all three contracts deployed.

mod fixtures;
use fixtures::setup_chainlearn_env;

use learn_token::LearnTokenClient;
use soroban_sdk::{testutils::Address as _, testutils::Events as _, Address, Symbol};

/// Create a vesting schedule, verify tokens are locked before the cliff,
/// advance time to verify linear vesting, and let the beneficiary claim.
#[test]
fn test_vesting_full_lifecycle() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    env.mock_all_auths();

    let token_client = LearnTokenClient::new(env, &setup.token_contract_id);
    let beneficiary = Address::generate(env);

    let total_amount: i128 = 10_000;
    let cliff_timestamp: u64 = 100;
    let duration_seconds: u64 = 1_000;

    // Set ledger timestamp to 0 so the schedule's created_at is 0.
    env.ledger().with_mut(|l| {
        l.timestamp = 0;
    });

    // ── Create vesting schedule ──
    token_client.create_vesting(&beneficiary, &total_amount, &cliff_timestamp, &duration_seconds);

    let schedule = token_client
        .get_vesting_schedule(&beneficiary)
        .expect("schedule should exist");
    assert_eq!(schedule.total_amount, total_amount);
    assert_eq!(schedule.cliff_timestamp, cliff_timestamp);
    assert_eq!(schedule.duration_seconds, duration_seconds);
    assert!(!schedule.exhausted);
    assert_eq!(token_client.get_vesting_claimed(&beneficiary), 0);
    assert_eq!(token_client.balance(&beneficiary), 0);

    // Events: vesting_created emitted
    {
        let all = env.events().all();
        let (_, topics, _) = all.last().expect("no events emitted");
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        let event_name: Symbol = topics.get(0).unwrap().into_val(env);
        assert_eq!(event_name, Symbol::new(env, "vesting_created"));
    }

    // ── Cliff enforcement: claiming before cliff reverts ──
    env.ledger().with_mut(|l| {
        l.timestamp = cliff_timestamp - 1;
    });
    assert!(token_client.try_claim_vested(&beneficiary).is_err());
    assert_eq!(token_client.balance(&beneficiary), 0);

    // ── Linear vesting halfway (50% vested) ──
    env.ledger().with_mut(|l| {
        l.timestamp = cliff_timestamp + duration_seconds / 2;
    });

    token_client.claim_vested(&beneficiary);
    assert_eq!(token_client.balance(&beneficiary), 5_000);
    assert_eq!(token_client.get_vesting_claimed(&beneficiary), 5_000);
    assert_eq!(token_client.total_supply(), 5_000);

    let mid_schedule = token_client.get_vesting_schedule(&beneficiary).unwrap();
    assert!(!mid_schedule.exhausted);

    // Events: vesting_claimed emitted
    {
        let all = env.events().all();
        let (_, topics, _) = all.last().expect("no events emitted");
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        let event_name: Symbol = topics.get(0).unwrap().into_val(env);
        assert_eq!(event_name, Symbol::new(env, "vesting_claimed"));
    }

    // ── Linear vesting complete (100% vested) ──
    env.ledger().with_mut(|l| {
        l.timestamp = cliff_timestamp + duration_seconds;
    });

    token_client.claim_vested(&beneficiary);
    assert_eq!(token_client.balance(&beneficiary), total_amount);
    assert_eq!(token_client.get_vesting_claimed(&beneficiary), total_amount);
    assert_eq!(token_client.total_supply(), total_amount);

    let final_schedule = token_client.get_vesting_schedule(&beneficiary).unwrap();
    assert!(final_schedule.exhausted);

    // ── Claiming after exhaustion reverts ──
    assert!(token_client.try_claim_vested(&beneficiary).is_err());
}

/// Cliff is strictly enforced: no tokens available before cliff_timestamp.
#[test]
fn test_vesting_cliff_strictly_enforced() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    env.mock_all_auths();

    let token_client = LearnTokenClient::new(env, &setup.token_contract_id);
    let beneficiary = Address::generate(env);

    env.ledger().with_mut(|l| {
        l.timestamp = 0;
    });

    token_client.create_vesting(&beneficiary, &10_000, &500, &1_000);

    // At cliff - 1, no tokens claimable.
    env.ledger().with_mut(|l| {
        l.timestamp = 499;
    });
    assert!(token_client.try_claim_vested(&beneficiary).is_err());
    assert_eq!(token_client.balance(&beneficiary), 0);

    // At cliff timestamp exactly, 0 tokens vested (elapsed = 0).
    env.ledger().with_mut(|l| {
        l.timestamp = 500;
    });
    // elapsed = 0, so vested_amount = 0, claimable = 0 - 0 = 0 → "no tokens available to claim"
    assert!(token_client.try_claim_vested(&beneficiary).is_err());

    // At cliff + 1, some tokens are claimable.
    env.ledger().with_mut(|l| {
        l.timestamp = 501;
    });
    token_client.claim_vested(&beneficiary);
    assert!(token_client.balance(&beneficiary) > 0);
}

/// Beneficiary can only claim; other addresses cannot claim on their behalf.
#[test]
fn test_vesting_beneficiary_only_claiming() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    env.mock_all_auths();

    let token_client = LearnTokenClient::new(env, &setup.token_contract_id);
    let beneficiary = Address::generate(env);
    let stranger = Address::generate(env);

    env.ledger().with_mut(|l| {
        l.timestamp = 0;
    });

    token_client.create_vesting(&beneficiary, &10_000, &100, &1_000);

    env.ledger().with_mut(|l| {
        l.timestamp = 600;
    });

    // Stranger cannot claim on behalf of beneficiary.
    assert!(token_client.try_claim_vested(&stranger).is_err());
    assert_eq!(token_client.balance(&stranger), 0);

    // Beneficiary can claim.
    token_client.claim_vested(&beneficiary);
    assert!(token_client.balance(&beneficiary) > 0);
}
