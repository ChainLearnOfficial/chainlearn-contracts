use soroban_sdk::{Address, Env, Symbol, Vec};

use crate::metadata::{CredentialInfo, DataKey};

/// Verify a credential by its ID and return its full info.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `credential_id` - The unique credential identifier
///
/// # Returns
/// The `CredentialInfo` for the given credential.
///
/// # Panics
/// If the credential does not exist.
pub fn verify_credential(env: &Env, credential_id: u64) -> CredentialInfo {
    env.storage()
        .persistent()
        .get(&DataKey::Credential(credential_id))
        .expect("credential not found")
}

/// Get all credential IDs belonging to a learner.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `learner` - The learner address
///
/// # Returns
/// A vector of credential IDs.
pub fn get_credentials_for(env: &Env, learner: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::LearnerCredentials(learner.clone()))
        .unwrap_or(Vec::new(env))
}

/// Check whether a credential is valid (exists and not revoked).
///
/// # Arguments
/// * `env` - Soroban environment
/// * `credential_id` - The unique credential identifier
///
/// # Returns
/// `true` if the credential exists and is not revoked.
pub fn is_credential_valid(env: &Env, credential_id: u64) -> bool {
    match env
        .storage()
        .persistent()
        .get::<DataKey, CredentialInfo>(&DataKey::Credential(credential_id))
    {
        Some(info) => !info.revoked,
        None => false,
    }
}

/// Revoke a credential. Admin only.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `credential_id` - The credential to revoke
pub fn revoke_credential(env: &Env, credential_id: u64) {
    let admin: Address = env
        .storage()
        .persistent()
        .get(&DataKey::Admin)
        .expect("not initialized");
    admin.require_auth();

    let mut info: CredentialInfo = env
        .storage()
        .persistent()
        .get(&DataKey::Credential(credential_id))
        .expect("credential not found");

    if info.revoked {
        panic!("credential already revoked");
    }

    info.revoked = true;
    env.storage()
        .persistent()
        .set(&DataKey::Credential(credential_id), &info);

    env.events()
        .publish((Symbol::new(env, "credential_revoked"),), (credential_id,));
}
