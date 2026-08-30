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
