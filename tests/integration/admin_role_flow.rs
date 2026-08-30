//! Integration tests for admin role separation on the learn-token contract.
//!
//! Verifies that the Minter and Pauser roles are enforced independently: a
//! minter can mint but cannot pause, a pauser can pause but cannot mint,
//! role changes take effect immediately, role revocation removes access, and
//! role changes emit events.

mod fixtures;
use fixtures::setup_chainlearn_env;

use learn_token::{AdminRole, LearnTokenClient};
use soroban_sdk::{testutils::Address as _, testutils::Events as _, Address, IntoVal, Symbol};

#[test]
fn test_admin_role_separation() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    let admin = &setup.admin;
    env.mock_all_auths();

    let token_client = LearnTokenClient::new(env, &setup.token_contract_id);

    let minter = Address::generate(env);
    let pauser = Address::generate(env);
    let recipient = Address::generate(env);

    // Neither address starts with any role.
    assert!(!token_client.has_role(&minter, &AdminRole::Minter));
    assert!(!token_client.has_role(&pauser, &AdminRole::Pauser));

    // Grant the minter role.
    token_client.grant_role(admin, &minter, &AdminRole::Minter);
    assert!(token_client.has_role(&minter, &AdminRole::Minter));
    assert!(!token_client.has_role(&minter, &AdminRole::Pauser));
    {
        let all = env.events().all();
        let (_, topics, _) = all.last().expect("no events emitted");
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        let event_name: Symbol = topics.get(0).unwrap().into_val(env);
        let address_topic: Address = topics.get(1).unwrap().into_val(env);
        assert_eq!(event_name, Symbol::new(env, "role_granted"));
        assert_eq!(address_topic, minter);
    }

    // The minter can mint...
    token_client.mint(&minter, &recipient, &1000);
    assert_eq!(token_client.balance(&recipient), 1000);

    // ...but cannot pause the contract.
    assert!(token_client.try_pause(&minter).is_err());

    // Grant the pauser role.
    token_client.grant_role(admin, &pauser, &AdminRole::Pauser);
    assert!(token_client.has_role(&pauser, &AdminRole::Pauser));
    assert!(!token_client.has_role(&pauser, &AdminRole::Minter));
    {
        let all = env.events().all();
        let (_, topics, _) = all.last().expect("no events emitted");
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        let event_name: Symbol = topics.get(0).unwrap().into_val(env);
        let address_topic: Address = topics.get(1).unwrap().into_val(env);
        assert_eq!(event_name, Symbol::new(env, "role_granted"));
        assert_eq!(address_topic, pauser);
    }

    // The pauser can pause and unpause...
    token_client.pause(&pauser);
    assert!(token_client.is_paused());
    token_client.unpause(&pauser);
    assert!(!token_client.is_paused());

    // ...but cannot mint.
    assert!(token_client.try_mint(&pauser, &recipient, &1000).is_err());

    // Revoke both roles.
    token_client.revoke_role(admin, &minter, &AdminRole::Minter);
    {
        let all = env.events().all();
        let (_, topics, _) = all.last().expect("no events emitted");
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        let event_name: Symbol = topics.get(0).unwrap().into_val(env);
        let address_topic: Address = topics.get(1).unwrap().into_val(env);
        assert_eq!(event_name, Symbol::new(env, "role_revoked"));
        assert_eq!(address_topic, minter);
    }

    token_client.revoke_role(admin, &pauser, &AdminRole::Pauser);
    {
        let all = env.events().all();
        let (_, topics, _) = all.last().expect("no events emitted");
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = topics.clone();
        let event_name: Symbol = topics.get(0).unwrap().into_val(env);
        let address_topic: Address = topics.get(1).unwrap().into_val(env);
        assert_eq!(event_name, Symbol::new(env, "role_revoked"));
        assert_eq!(address_topic, pauser);
    }

    // Revocation removes access: neither address can perform its former
    // action anymore.
    assert!(!token_client.has_role(&minter, &AdminRole::Minter));
    assert!(!token_client.has_role(&pauser, &AdminRole::Pauser));
    assert!(token_client.try_mint(&minter, &recipient, &1000).is_err());
    assert!(token_client.try_pause(&pauser).is_err());
}

#[test]
fn test_multi_admin_management_and_multisig_ops() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    let primary_admin = &setup.admin;
    env.mock_all_auths();

    let token_client = LearnTokenClient::new(env, &setup.token_contract_id);

    // Initial admin list includes the primary admin
    let admins = token_client.get_admins();
    assert_eq!(admins.len(), 1);
    assert_eq!(admins.get(0).unwrap().address, *primary_admin);
    assert_eq!(admins.get(0).unwrap().role, AdminRole::Admin);

    // Add secondary admin
    let secondary_admin = Address::generate(env);
    token_client.add_admin(
        primary_admin,
        &learn_token::AdminInfo {
            address: secondary_admin.clone(),
            role: AdminRole::Admin,
        },
    );

    let admins = token_client.get_admins();
    assert_eq!(admins.len(), 2);
    assert!(token_client.has_role(&secondary_admin, &AdminRole::Admin));

    // Add a minter admin
    let minter_admin = Address::generate(env);
    token_client.add_admin(
        primary_admin,
        &learn_token::AdminInfo {
            address: minter_admin.clone(),
            role: AdminRole::Minter,
        },
    );

    let admins = token_client.get_admins();
    assert_eq!(admins.len(), 3);
    assert!(token_client.has_role(&minter_admin, &AdminRole::Minter));

    // Execute multi-sig operation with primary and secondary admins
    token_client.execute_multisig_op(
        primary_admin,
        &secondary_admin,
        &Symbol::new(env, "critical_op"),
    );

    // Remove minter admin
    token_client.remove_admin(
        primary_admin,
        &learn_token::AdminInfo {
            address: minter_admin.clone(),
            role: AdminRole::Minter,
        },
    );

    let admins = token_client.get_admins();
    assert_eq!(admins.len(), 2);
    assert!(!token_client.has_role(&minter_admin, &AdminRole::Minter));
}

#[test]
fn test_multi_admin_role_enforcement_revocation_and_events() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    let primary_admin = &setup.admin;
    env.mock_all_auths();

    let token_client = LearnTokenClient::new(env, &setup.token_contract_id);

    let admin_minter = Address::generate(env);
    let admin_pauser = Address::generate(env);
    let recipient = Address::generate(env);

    // 1. Add multiple admins with distinct roles
    token_client.add_admin(
        primary_admin,
        &learn_token::AdminInfo {
            address: admin_minter.clone(),
            role: AdminRole::Minter,
        },
    );

    token_client.add_admin(
        primary_admin,
        &learn_token::AdminInfo {
            address: admin_pauser.clone(),
            role: AdminRole::Pauser,
        },
    );

    let admins = token_client.get_admins();
    assert_eq!(admins.len(), 3);

    // Verify role granted events were emitted
    let events = env.events().all();
    assert!(events.len() >= 2);

    // 2. Enforce roles
    assert!(token_client.has_role(&admin_minter, &AdminRole::Minter));
    assert!(!token_client.has_role(&admin_minter, &AdminRole::Pauser));
    assert!(token_client.has_role(&admin_pauser, &AdminRole::Pauser));
    assert!(!token_client.has_role(&admin_pauser, &AdminRole::Minter));

    // admin_minter can mint
    token_client.mint(&admin_minter, &recipient, &500);
    assert_eq!(token_client.balance(&recipient), 500);

    // admin_minter cannot pause
    assert!(token_client.try_pause(&admin_minter).is_err());

    // admin_pauser can pause
    token_client.pause(&admin_pauser);
    assert!(token_client.is_paused());
    token_client.unpause(&admin_pauser);

    // admin_pauser cannot mint
    assert!(token_client.try_mint(&admin_pauser, &recipient, &500).is_err());

    // 3. Revocation
    token_client.remove_admin(
        primary_admin,
        &learn_token::AdminInfo {
            address: admin_minter.clone(),
            role: AdminRole::Minter,
        },
    );

    token_client.remove_admin(
        primary_admin,
        &learn_token::AdminInfo {
            address: admin_pauser.clone(),
            role: AdminRole::Pauser,
        },
    );

    // Verify roles are revoked and actions fail
    assert!(!token_client.has_role(&admin_minter, &AdminRole::Minter));
    assert!(!token_client.has_role(&admin_pauser, &AdminRole::Pauser));
    assert!(token_client.try_mint(&admin_minter, &recipient, &100).is_err());
    assert!(token_client.try_pause(&admin_pauser).is_err());

    let final_admins = token_client.get_admins();
    assert_eq!(final_admins.len(), 1);
}

