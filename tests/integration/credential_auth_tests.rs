//! Authorization tests for credential-nft admin functions
//! These tests verify that non-admin callers are properly rejected

#![cfg(test)]
use soroban_sdk::{testutils::Address as _, Address, Env};
use crate::credential_nft::{CredentialNFT, CredentialNFTClient};

fn create_credential_nft(e: &Env) -> CredentialNFTClient {
    CredentialNFTClient::new(e, &e.register_contract(None, CredentialNFT {}))
}

#[test]
fn test_mint_credential_non_admin_fails() {
    let e = Env::default();
    let contract = create_credential_nft(&e);
    let non_admin = Address::generate(&e);
    let recipient = Address::generate(&e);
    
    // Ensure non_admin is not the admin
    let admin = contract.get_admin();
    assert_ne!(admin, non_admin);
    
    let result = std::panic::catch_unwind(|| {
        contract
            .with_source_account(&non_admin)
            .mint_credential(&recipient, &1, &"metadata".into());
    });
    assert!(result.is_err());
}

#[test]
fn test_renew_credential_non_admin_fails() {
    let e = Env::default();
    let contract = create_credential_nft(&e);
    let admin = contract.get_admin();
    let non_admin = Address::generate(&e);
    let recipient = Address::generate(&e);
    
    // Setup: mint a credential as admin
    contract
        .with_source_account(&admin)
        .mint_credential(&recipient, &1, &"metadata".into());
    
    let result = std::panic::catch_unwind(|| {
        contract
            .with_source_account(&non_admin)
            .renew_credential(&1);
    });
    assert!(result.is_err());
}

#[test]
fn test_set_credential_display_non_admin_fails() {
    let e = Env::default();
    let contract = create_credential_nft(&e);
    let admin = contract.get_admin();
    let non_admin = Address::generate(&e);
    let recipient = Address::generate(&e);
    
    // Setup
    contract
        .with_source_account(&admin)
        .mint_credential(&recipient, &1, &"metadata".into());
    
    let result = std::panic::catch_unwind(|| {
        contract
            .with_source_account(&non_admin)
            .set_credential_display(&1, &"new_display".into());
    });
    assert!(result.is_err());
}

#[test]
fn test_transfer_admin_non_admin_fails() {
    let e = Env::default();
    let contract = create_credential_nft(&e);
    let non_admin = Address::generate(&e);
    let new_admin = Address::generate(&e);
    
    let result = std::panic::catch_unwind(|| {
        contract
            .with_source_account(&non_admin)
            .transfer_admin(&new_admin);
    });
    assert!(result.is_err());
}