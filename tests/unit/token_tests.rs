use soroban_sdk::{testutils::Address as _, vec, Address, Env, Symbol, Vec};
use std::collections::HashMap;

mod learn_token {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32-unknown-unknown/release/learn_token.wasm"
    );
}

fn create_learn_token(env: &Env, admin: &Address) -> learn_token::Client {
    let client = learn_token::Client::new(env, &env.register_contract_wasm(None));
    client.initialize(admin, &7, &1000000, &1000000000);
    client
}

#[test]
fn test_get_claim_history() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let client = create_learn_token(&env, &admin);

    // Initial history should be empty
    let history = client.get_claim_history(&user1);
    assert_eq!(history.len(), 0);

    // Mint some tokens to create claims
    client.mint(&user1, &100);
    client.mint(&user2, &200);
    client.mint(&user1, &50);

    // Verify claim history for user1
    let history = client.get_claim_history(&user1);
    assert_eq!(history.len(), 2);
    assert_eq!(history.get(0).unwrap().amount, 100);
    assert_eq!(history.get(1).unwrap().amount, 50);

    // Verify claim history for user2
    let history = client.get_claim_history(&user2);
    assert_eq!(history.len(), 1);
    assert_eq!(history.get(0).unwrap().amount, 200);

    // Verify empty history for new user
    let user3 = Address::generate(&env);
    let history = client.get_claim_history(&user3);
    assert_eq!(history.len(), 0);
}

#[test]
fn test_get_storage_size() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let client = create_learn_token(&env, &admin);

    // Initial storage size
    let initial_size = client.get_storage_size();
    assert!(initial_size > 0);

    // Perform operations that should increase storage
    client.mint(&user, &100);
    let size_after_mint = client.get_storage_size();
    assert!(size_after_mint > initial_size);

    // Add another user
    let user2 = Address::generate(&env);
    client.mint(&user2, &200);
    let size_after_second_mint = client.get_storage_size();
    assert!(size_after_second_mint > size_after_mint);
}

#[test]
fn test_total_minted_to() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let client = create_learn_token(&env, &admin);

    // Initial totals should be zero
    assert_eq!(client.total_minted_to(&user1), 0);
    assert_eq!(client.total_minted_to(&user2), 0);

    // Mint tokens
    client.mint(&user1, &100);
    client.mint(&user1, &50);
    client.mint(&user2, &200);

    // Verify totals
    assert_eq!(client.total_minted_to(&user1), 150);
    assert_eq!(client.total_minted_to(&user2), 200);

    // Verify non-existent user
    let user3 = Address::generate(&env);
    assert_eq!(client.total_minted_to(&user3), 0);
}

#[test]
fn test_get_vesting_schedule() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let client = create_learn_token(&env, &admin);

    // Create a vesting schedule
    let start_time = 1000u64;
    let end_time = 2000u64;
    let total_amount = 1000u64;
    client.create_vesting_schedule(&user, &start_time, &end_time, &total_amount);

    // Get the schedule
    let schedule = client.get_vesting_schedule(&user);
    assert!(schedule.is_some());
    let schedule = schedule.unwrap();
    assert_eq!(schedule.start_time, start_time);
    assert_eq!(schedule.end_time, end_time);
    assert_eq!(schedule.total_amount, total_amount);

    // Verify non-existent schedule
    let user2 = Address::generate(&env);
    let schedule = client.get_vesting_schedule(&user2);
    assert!(schedule.is_none());
}

#[test]
fn test_permit_nonce() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let client = create_learn_token(&env, &admin);

    // Initial nonce should be 0
    assert_eq!(client.permit_nonce(&user), 0);

    // First permit should increment nonce
    let domain = Symbol::new(&env, "test");
    let spender = Address::generate(&env);
    let amount = 100u64;
    let expiration_ledger = 1000u32;
    client.permit(&user, &domain, &spender, &amount, &expiration_ledger);
    assert_eq!(client.permit_nonce(&user), 1);

    // Second permit should increment again
    client.permit(&user, &domain, &spender, &amount, &expiration_ledger);
    assert_eq!(client.permit_nonce(&user), 2);

    // Different user should have separate nonce
    let user2 = Address::generate(&env);
    assert_eq!(client.permit_nonce(&user2), 0);
}

#[test]
fn test_get_admins() {
    let env = Env::default();
    let admin1 = Address::generate(&env);
    let client = create_learn_token(&env, &admin1);

    // Initial admin should be the one used in initialization
    let admins = client.get_admins();
    assert_eq!(admins.len(), 1);
    assert_eq!(admins.get(0).unwrap(), admin1);

    // Add another admin
    let admin2 = Address::generate(&env);
    client.add_admin(&admin2);
    let admins = client.get_admins();
    assert_eq!(admins.len(), 2);
    assert!(admins.contains(&admin1));
    assert!(admins.contains(&admin2));
}

#[test]
fn test_has_role() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let client = create_learn_token(&env, &admin);

    // Admin should have admin role
    assert!(client.has_role(&admin, &Symbol::new(&env, "admin")));

    // Regular user should not have admin role
    assert!(!client.has_role(&user, &Symbol::new(&env, "admin")));

    // Test with non-existent role
    assert!(!client.has_role(&admin, &Symbol::new(&env, "nonexistent")));
}

#[test]
fn test_prune_expired_allowance() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    let client = create_learn_token(&env, &admin);

    // Mint some tokens to owner
    client.mint(&owner, &1000);

    // Create allowance with expiration
    let amount = 100u64;
    let expiration_ledger = env.ledger().sequence() + 1; // Expires next ledger
    client.approve(&owner, &spender, &amount, &expiration_ledger);

    // Verify allowance exists
    let allowance = client.allowance(&owner, &spender);
    assert_eq!(allowance, amount);

    // Move past expiration
    env.ledger().set_sequence(expiration_ledger + 1);

    // Prune expired allowance
    client.prune_expired_allowance(&owner, &spender);

    // Verify allowance is removed
    let allowance = client.allowance(&owner, &spender);
    assert_eq!(allowance, 0);
}

#[test]
fn test_cleanup_expired_allowances() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let spender1 = Address::generate(&env);
    let spender2 = Address::generate(&env);
    let client = create_learn_token(&env, &admin);

    // Mint some tokens to owner
    client.mint(&owner, &1000);

    // Create allowances with different expirations
    let current_ledger = env.ledger().sequence();
    client.approve(&owner, &spender1, &100, &(current_ledger + 1)); // Expires soon
    client.approve(&owner, &spender2, &200, &(current_ledger + 100)); // Expires later

    // Move past first expiration
    env.ledger().set_sequence(current_ledger + 2);

    // Cleanup expired allowances
    client.cleanup_expired_allowances(&owner);

    // Verify first allowance is removed, second remains
    assert_eq!(client.allowance(&owner, &spender1), 0);
    assert_eq!(client.allowance(&owner, &spender2), 200);
}
