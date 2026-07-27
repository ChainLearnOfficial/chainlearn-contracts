use chainlearn_shared::MAX_CREDENTIALS_PAGE_SIZE;
use soroban_sdk::{Address, Env, Symbol, Vec};

use crate::metadata::{CredentialInfo, DataKey};

/// Read the full list of credential IDs owned by a learner.
fn learner_credentials(env: &Env, learner: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::LearnerCredentials(learner.clone()))
        .unwrap_or(Vec::new(env))
}

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

/// Get a page of credential IDs belonging to a learner.
///
/// Responses are bounded: a learner with thousands of credentials is read one
/// page at a time instead of in a single unbounded response (#102). Use
/// [`get_credential_count`] to discover how many pages there are.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `learner` - The learner address
/// * `start` - Zero-based index of the first credential to return
/// * `limit` - Maximum number of credential IDs to return
///
/// # Returns
/// A vector of at most `limit` credential IDs, empty once `start` is past the
/// end of the learner's credential list.
///
/// # Panics
/// * If `limit` is zero
/// * If `limit` exceeds `MAX_CREDENTIALS_PAGE_SIZE`
pub fn get_credentials_for(env: &Env, learner: &Address, start: u32, limit: u32) -> Vec<u64> {
    if limit == 0 {
        panic!("limit must be greater than zero");
    }
    if limit > MAX_CREDENTIALS_PAGE_SIZE {
        panic!(
            "limit {} exceeds maximum page size {}",
            limit, MAX_CREDENTIALS_PAGE_SIZE
        );
    }

    let credentials = learner_credentials(env, learner);
    let total = credentials.len();
    if start >= total {
        return Vec::new(env);
    }

    // `start < total` here, so the end of the page is clamped to the list length.
    let end = start.saturating_add(limit).min(total);
    credentials.slice(start..end)
}

/// Count the credentials belonging to a learner.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `learner` - The learner address
///
/// # Returns
/// The total number of credentials the learner holds, revoked ones included.
pub fn get_credential_count(env: &Env, learner: &Address) -> u32 {
    learner_credentials(env, learner).len()
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

    // Emit the learner, course and revoking admin alongside the ID so revocations
    // can be audited without a follow-up state read (#100).
    env.events().publish(
        (Symbol::new(env, "credential_revoked"),),
        (info.learner, info.course_id, credential_id, admin),
    );
}
