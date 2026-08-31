//! Integration tests for multi-admin support (#279).
//!
//! Verifies that multiple admins with different roles can be added,
//! each role is enforced, revocation works, and events are emitted.

mod fixtures;
use fixtures::setup_chainlearn_env;

use learn_token::{AdminRole, LearnTokenClient, AdminInfo};
use soroban_sdk::{testutils::Address as _, testutils::Events as _, Address, Symbol};

/// Add multiple admins with Minter, Pauser, and Admin roles.
/// Verify each role is enforced, then revoke and verify access is lost.
#[test]
fn test_multiple_admins_distinct_roles_enforced_and_revoked() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    let primary_admin = &setup.admin;
    let recipient = Address::generate(env);
    env.mock_all_auths();

    let token_client = LearnTokenClient::new(env, &setup.token_contract_id);

    // ── Add multiple admins with distinct roles ──
    let minter_a = Address::generate(env);
    let minter_b = Address::generate(env);
    let pauser = Address::generate(env);
    let full_admin = Address::generate(env);

    token_client.add_admin(
        primary_admin,
        &AdminInfo {
            address: minter_a.clone(),
            role: AdminRole::Minter,
        },
    );
    token_client.add_admin(
        primary_admin,
        &AdminInfo {
            address: minter_b.clone(),
            role: AdminRole::Minter,
        },
    );
    token_client.add_admin(
        primary_admin,
        &AdminInfo {
            address: pauser.clone(),
            role: AdminRole::Pauser,
        },
    );
    token_client.add_admin(
        primary_admin,
        &AdminInfo {
            address: full_admin.clone(),
            role: AdminRole::Admin,
        },
    );

    // Verify admin list: 5 total (primary + minter_a + minter_b + pauser + full_admin)
    let admins = token_client.get_admins();
    assert_eq!(admins.len(), 5);

    // ── Verify role enforcement ──
    // Both minters can mint.
    token_client.mint(&minter_a, &recipient, &1000);
    assert_eq!(token_client.balance(&recipient), 1000);
    token_client.mint(&minter_b, &recipient, &2000);
    assert_eq!(token_client.balance(&recipient), 3000);

    // Minter A cannot pause.
    assert!(token_client.try_pause(&minter_a).is_err());
    // Minter B cannot pause.
    assert!(token_client.try_pause(&minter_b).is_err());

    // Pauser can pause and unpause.
    token_client.pause(&pauser);
    assert!(token_client.is_paused());
    token_client.unpause(&pauser);

    // Pauser cannot mint.
    assert!(token_client.try_mint(&pauser, &recipient, &500).is_err());

    // Full admin can do everything.
    token_client.mint(&full_admin, &recipient, &500);
    assert_eq!(token_client.balance(&recipient), 3500);
    token_client.pause(&full_admin);
    assert!(token_client.is_paused());
    token_client.unpause(&full_admin);

    // Full admin can also grant/revoke roles.
    let temp = Address::generate(env);
    token_client.grant_role(&full_admin, &temp, &AdminRole::Minter);
    assert!(token_client.has_role(&temp, &AdminRole::Minter));
    token_client.revoke_role(&full_admin, &temp, &AdminRole::Minter);
    assert!(!token_client.has_role(&temp, &AdminRole::Minter));

    // ── Events emitted for role grants ──
    // At least 4 role_granted events (one per add_admin above, minus the full_admin
    // which also has Admin role so add_admin fires role_granted for Admin).
    // The temp grant/revocation adds 2 more.
    let events = env.events().all();
    let mut role_granted_count = 0u32;
    let mut role_revoked_count = 0u32;
    for i in 0..events.len() {
        let (_, topics, _) = events.get(i).unwrap();
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        if topics.len() > 0 {
            let event_name: Symbol = topics.get(0).unwrap().into_val(env);
            if event_name == Symbol::new(env, "role_granted") {
                role_granted_count += 1;
            }
            if event_name == Symbol::new(env, "role_revoked") {
                role_revoked_count += 1;
            }
        }
    }
    assert!(role_granted_count >= 5); // 4 add_admin + 1 grant_role
    assert!(role_revoked_count >= 1); // 1 revoke_role

    // ── Revocation: remove all secondary admins ──
    token_client.remove_admin(
        primary_admin,
        &AdminInfo {
            address: minter_a.clone(),
            role: AdminRole::Minter,
        },
    );
    token_client.remove_admin(
        primary_admin,
        &AdminInfo {
            address: minter_b.clone(),
            role: AdminRole::Minter,
        },
    );
    token_client.remove_admin(
        primary_admin,
        &AdminInfo {
            address: pauser.clone(),
            role: AdminRole::Pauser,
        },
    );
    token_client.remove_admin(
        primary_admin,
        &AdminInfo {
            address: full_admin.clone(),
            role: AdminRole::Admin,
        },
    );

    // Verify revoked roles cannot perform their former actions.
    assert!(!token_client.has_role(&minter_a, &AdminRole::Minter));
    assert!(!token_client.has_role(&minter_b, &AdminRole::Minter));
    assert!(!token_client.has_role(&pauser, &AdminRole::Pauser));
    assert!(!token_client.has_role(&full_admin, &AdminRole::Admin));

    assert!(token_client.try_mint(&minter_a, &recipient, &100).is_err());
    assert!(token_client.try_mint(&minter_b, &recipient, &100).is_err());
    assert!(token_client.try_pause(&pauser).is_err());
    assert!(token_client.try_mint(&full_admin, &recipient, &100).is_err());

    // Only primary admin remains.
    let final_admins = token_client.get_admins();
    assert_eq!(final_admins.len(), 1);
    assert_eq!(final_admins.get(0).unwrap().address, *primary_admin);
}

/// Grant and revoke roles via the grant_role/revoke_role functions (not add_admin/remove_admin).
#[test]
fn test_grant_revoke_role_lifecycle() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    let admin = &setup.admin;
    let recipient = Address::generate(env);
    env.mock_all_auths();

    let token_client = LearnTokenClient::new(env, &setup.token_contract_id);

    let alice = Address::generate(env);
    let bob = Address::generate(env);

    // Initially no roles.
    assert!(!token_client.has_role(&alice, &AdminRole::Minter));
    assert!(!token_client.has_role(&bob, &AdminRole::Pauser));

    // Grant minter to Alice.
    token_client.grant_role(admin, &alice, &AdminRole::Minter);
    assert!(token_client.has_role(&alice, &AdminRole::Minter));
    assert!(!token_client.has_role(&alice, &AdminRole::Pauser));

    // Grant pauser to Bob.
    token_client.grant_role(admin, &bob, &AdminRole::Pauser);
    assert!(token_client.has_role(&bob, &AdminRole::Pauser));
    assert!(!token_client.has_role(&bob, &AdminRole::Minter));

    // Alice can mint, Bob can pause.
    token_client.mint(&alice, &recipient, &500);
    assert_eq!(token_client.balance(&recipient), 500);

    token_client.pause(&bob);
    assert!(token_client.is_paused());
    token_client.unpause(&bob);

    // Revoke both.
    token_client.revoke_role(admin, &alice, &AdminRole::Minter);
    token_client.revoke_role(admin, &bob, &AdminRole::Pauser);

    // Both lose access.
    assert!(!token_client.has_role(&alice, &AdminRole::Minter));
    assert!(!token_client.has_role(&bob, &AdminRole::Pauser));
    assert!(token_client.try_mint(&alice, &recipient, &100).is_err());
    assert!(token_client.try_pause(&bob).is_err());

    // Events: role_granted x2, role_revoked x2
    let events = env.events().all();
    let mut granted = 0u32;
    let mut revoked = 0u32;
    for i in 0..events.len() {
        let (_, topics, _) = events.get(i).unwrap();
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        if topics.len() > 0 {
            let event_name: Symbol = topics.get(0).unwrap().into_val(env);
            if event_name == Symbol::new(env, "role_granted") {
                granted += 1;
            }
            if event_name == Symbol::new(env, "role_revoked") {
                revoked += 1;
            }
        }
    }
    assert!(granted >= 2);
    assert!(revoked >= 2);
}
