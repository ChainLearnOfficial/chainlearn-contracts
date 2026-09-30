//! Authorization tests for progress-tracker admin functions
//! These tests verify that non-admin callers are properly rejected

#![cfg(test)]
use soroban_sdk::{testutils::Address as _, Address, Env};
use crate::progress_tracker::{ProgressTracker, ProgressTrackerClient};

fn create_progress_tracker(e: &Env) -> ProgressTrackerClient {
    ProgressTrackerClient::new(e, &e.register_contract(None, ProgressTracker {}))
}

#[test]
fn test_create_course_non_admin_fails() {
    let e = Env::default();
    let contract = create_progress_tracker(&e);
    let non_admin = Address::generate(&e);
    
    // Ensure non_admin is not the admin
    let admin = contract.get_admin();
    assert_ne!(admin, non_admin);
    
    // This should panic with authorization error
    let result = std::panic::catch_unwind(|| {
        contract
            .with_source_account(&non_admin)
            .create_course(&"course1".into(), &1, &vec![&e, 1u32].into(), &"ipfs_hash".into());
    });
    assert!(result.is_err());
}

#[test]
fn test_archive_course_non_admin_fails() {
    let e = Env::default();
    let contract = create_progress_tracker(&e);
    let admin = contract.get_admin();
    let non_admin = Address::generate(&e);
    
    // Setup: create a course as admin
    contract
        .with_source_account(&admin)
        .create_course(&"course1".into(), &1, &vec![&e, 1u32].into(), &"ipfs_hash".into());
    
    // Non-admin tries to archive
    let result = std::panic::catch_unwind(|| {
        contract
            .with_source_account(&non_admin)
            .archive_course(&1);
    });
    assert!(result.is_err());
}

#[test]
fn test_set_course_content_hash_non_admin_fails() {
    let e = Env::default();
    let contract = create_progress_tracker(&e);
    let admin = contract.get_admin();
    let non_admin = Address::generate(&e);
    
    // Setup
    contract
        .with_source_account(&admin)
        .create_course(&"course1".into(), &1, &vec![&e, 1u32].into(), &"ipfs_hash".into());
    
    let result = std::panic::catch_unwind(|| {
        contract
            .with_source_account(&non_admin)
            .set_course_content_hash(&1, &"new_hash".into());
    });
    assert!(result.is_err());
}

#[test]
fn test_set_course_difficulty_non_admin_fails() {
    let e = Env::default();
    let contract = create_progress_tracker(&e);
    let admin = contract.get_admin();
    let non_admin = Address::generate(&e);
    
    // Setup
    contract
        .with_source_account(&admin)
        .create_course(&"course1".into(), &1, &vec![&e, 1u32].into(), &"ipfs_hash".into());
    
    let result = std::panic::catch_unwind(|| {
        contract
            .with_source_account(&non_admin)
            .set_course_difficulty(&1, &2);
    });
    assert!(result.is_err());
}

#[test]
fn test_set_course_tags_non_admin_fails() {
    let e = Env::default();
    let contract = create_progress_tracker(&e);
    let admin = contract.get_admin();
    let non_admin = Address::generate(&e);
    
    // Setup
    contract
        .with_source_account(&admin)
        .create_course(&"course1".into(), &1, &vec![&e, 1u32].into(), &"ipfs_hash".into());
    
    let result = std::panic::catch_unwind(|| {
        contract
            .with_source_account(&non_admin)
            .set_course_tags(&1, &vec![&e, "tag1".into()]);
    });
    assert!(result.is_err());
}

#[test]
fn test_update_course_version_non_admin_fails() {
    let e = Env::default();
    let contract = create_progress_tracker(&e);
    let admin = contract.get_admin();
    let non_admin = Address::generate(&e);
    
    // Setup
    contract
        .with_source_account(&admin)
        .create_course(&"course1".into(), &1, &vec![&e, 1u32].into(), &"ipfs_hash".into());
    
    let result = std::panic::catch_unwind(|| {
        contract
            .with_source_account(&non_admin)
            .update_course_version(&1, &2);
    });
    assert!(result.is_err());
}

#[test]
fn test_set_prerequisites_non_admin_fails() {
    let e = Env::default();
    let contract = create_progress_tracker(&e);
    let admin = contract.get_admin();
    let non_admin = Address::generate(&e);
    
    // Setup
    contract
        .with_source_account(&admin)
        .create_course(&"course1".into(), &1, &vec![&e, 1u32].into(), &"ipfs_hash".into());
    contract
        .with_source_account(&admin)
        .create_course(&"course2".into(), &2, &vec![&e, 1u32].into(), &"ipfs_hash2".into());
    
    let result = std::panic::catch_unwind(|| {
        contract
            .with_source_account(&non_admin)
            .set_prerequisites(&2, &vec![&e, 1u32]);
    });
    assert!(result.is_err());
}

#[test]
fn test_emergency_pause_non_admin_fails() {
    let e = Env::default();
    let contract = create_progress_tracker(&e);
    let non_admin = Address::generate(&e);
    
    let result = std::panic::catch_unwind(|| {
        contract
            .with_source_account(&non_admin)
            .emergency_pause(&true);
    });
    assert!(result.is_err());
}