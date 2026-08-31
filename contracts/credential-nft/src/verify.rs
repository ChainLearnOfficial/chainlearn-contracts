use chainlearn_shared::MAX_CREDENTIALS_PAGE_SIZE;
use soroban_sdk::{Address, Env, Symbol, Vec};

use crate::metadata::{
    no_display, one_display, CredentialDataKey, CredentialDisplay, CredentialInfo,
    CredentialVerification,
};

/// Read the full list of credential IDs owned by a learner.
fn learner_credentials(env: &Env, learner: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&CredentialDataKey::LearnerCredentials(learner.clone()))
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
        .get(&CredentialDataKey::Credential(credential_id))
        .expect("credential not found")
}

/// Verify a credential and return its full info along with optional display properties (#244).
///
/// # Arguments
/// * `env` - Soroban environment
/// * `credential_id` - The unique credential identifier
///
/// # Returns
/// A `CredentialVerification` containing the credential info and optional display properties.
///
/// # Panics
/// If the credential does not exist.
pub fn verify_credential_with_display(env: &Env, credential_id: u64) -> CredentialVerification {
    let info: CredentialInfo = env
        .storage()
        .persistent()
        .get(&CredentialDataKey::Credential(credential_id))
        .expect("credential not found");
    let stored: Option<CredentialDisplay> = env
        .storage()
        .persistent()
        .get(&CredentialDataKey::Display(credential_id));
    let display = match stored {
        Some(d) => one_display(env, d),
        None => no_display(env),
    };
    CredentialVerification { info, display }
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

/// Check whether a credential is valid (exists, not revoked, and not expired).
///
/// Existence is checked with `has()`, the revoked flag is read from its
/// own single-bool entry, and expiration is checked against the current
/// ledger height (#193).
///
/// # Arguments
/// * `env` - Soroban environment
/// * `credential_id` - The unique credential identifier
///
/// # Returns
/// `true` if the credential exists, is not revoked, and has not expired.
pub fn is_credential_valid(env: &Env, credential_id: u64) -> bool {
    if !env
        .storage()
        .persistent()
        .has(&CredentialDataKey::Credential(credential_id))
    {
        return false;
    }
    if env
        .storage()
        .persistent()
        .get::<CredentialDataKey, bool>(&CredentialDataKey::Revoked(credential_id))
        .unwrap_or(false)
    {
        return false;
    }
    // Check expiration (#193)
    let info: CredentialInfo = env
        .storage()
        .persistent()
        .get(&CredentialDataKey::Credential(credential_id))
        .expect("credential not found");
    if info.expires_at > 0 && env.ledger().sequence() > info.expires_at {
        return false;
    }
    true
}

/// Revoke a credential. Admin only.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `credential_id` - The credential to revoke
///
/// On revocation the credential ID is pruned from both the learner's
/// credential list and the course's credential index (#104). This keeps
/// those lists free of stale revoked entries.
pub fn revoke_credential(env: &Env, credential_id: u64) {
    let admin: Address = env
        .storage()
        .persistent()
        .get(&CredentialDataKey::Admin)
        .expect("not initialized");
    admin.require_auth();

    let mut info: CredentialInfo = env
        .storage()
        .persistent()
        .get(&CredentialDataKey::Credential(credential_id))
        .expect("credential not found");

    if info.revoked {
        panic!("credential already revoked");
    }

    info.revoked = true;
    crate::metadata::write_entry(env, &CredentialDataKey::Credential(credential_id), &info);
    // Kept in sync with `info.revoked` so `is_credential_valid` can check
    // revocation without deserializing the full `CredentialInfo` (#109).
    crate::metadata::write_entry(env, &CredentialDataKey::Revoked(credential_id), &true);

    // #104 — prune from learner's credential list
    let mut learner_list: Vec<u64> = env
        .storage()
        .persistent()
        .get(&CredentialDataKey::LearnerCredentials(info.learner.clone()))
        .unwrap_or(Vec::new(env));
    if let Some(pos) =
        (0..learner_list.len()).find(|&i| learner_list.get(i).unwrap() == credential_id)
    {
        learner_list.remove(pos);
        crate::metadata::write_entry(
            env,
            &CredentialDataKey::LearnerCredentials(info.learner.clone()),
            &learner_list,
        );
    }

    // #104 — prune from course credential index
    let mut course_list: Vec<u64> = env
        .storage()
        .persistent()
        .get(&CredentialDataKey::CourseCredentials(
            info.course_id.clone(),
        ))
        .unwrap_or(Vec::new(env));
    if let Some(pos) =
        (0..course_list.len()).find(|&i| course_list.get(i).unwrap() == credential_id)
    {
        course_list.remove(pos);
        crate::metadata::write_entry(
            env,
            &CredentialDataKey::CourseCredentials(info.course_id.clone()),
            &course_list,
        );
    }

    // Emit the learner, course and revoking admin alongside the ID so revocations
    // can be audited without a follow-up state read (#100).
    env.events().publish(
        (Symbol::new(env, "credential_revoked"),),
        (info.learner, info.course_id, credential_id, admin),
    );
}

/// Revoke a credential with a reason. Admin only (#194).
///
/// # Arguments
/// * `env` - Soroban environment
/// * `credential_id` - The credential to revoke
/// * `reason` - The reason for revocation
pub fn revoke_credential_with_reason(env: &Env, credential_id: u64, reason: Symbol) {
    let admin: Address = env
        .storage()
        .persistent()
        .get(&CredentialDataKey::Admin)
        .expect("not initialized");
    admin.require_auth();

    let mut info: CredentialInfo = env
        .storage()
        .persistent()
        .get(&CredentialDataKey::Credential(credential_id))
        .expect("credential not found");

    if info.revoked {
        panic!("credential already revoked");
    }

    info.revoked = true;
    crate::metadata::write_entry(env, &CredentialDataKey::Credential(credential_id), &info);
    crate::metadata::write_entry(env, &CredentialDataKey::Revoked(credential_id), &true);
    // Store the revocation reason (#194)
    crate::metadata::write_entry(
        env,
        &CredentialDataKey::RevocationReason(credential_id),
        &reason,
    );

    // #104 — prune from learner's credential list
    let mut learner_list: Vec<u64> = env
        .storage()
        .persistent()
        .get(&CredentialDataKey::LearnerCredentials(info.learner.clone()))
        .unwrap_or(Vec::new(env));
    if let Some(pos) =
        (0..learner_list.len()).find(|&i| learner_list.get(i).unwrap() == credential_id)
    {
        learner_list.remove(pos);
        crate::metadata::write_entry(
            env,
            &CredentialDataKey::LearnerCredentials(info.learner.clone()),
            &learner_list,
        );
    }

    // #104 — prune from course credential index
    let mut course_list: Vec<u64> = env
        .storage()
        .persistent()
        .get(&CredentialDataKey::CourseCredentials(
            info.course_id.clone(),
        ))
        .unwrap_or(Vec::new(env));
    if let Some(pos) =
        (0..course_list.len()).find(|&i| course_list.get(i).unwrap() == credential_id)
    {
        course_list.remove(pos);
        crate::metadata::write_entry(
            env,
            &CredentialDataKey::CourseCredentials(info.course_id.clone()),
            &course_list,
        );
    }

    env.events().publish(
        (Symbol::new(env, "credential_revoked"),),
        (info.learner, info.course_id, credential_id, admin, reason),
    );
}

/// Get the reason a credential was revoked (#194).
///
/// # Arguments
/// * `env` - Soroban environment
/// * `credential_id` - The credential to query
///
/// # Returns
/// The revocation reason, or None if the credential has not been revoked.
pub fn get_revocation_reason(env: &Env, credential_id: u64) -> Option<Symbol> {
    env.storage()
        .persistent()
        .get(&CredentialDataKey::RevocationReason(credential_id))
}

/// Renew a credential's expiration (#193). Admin only.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `credential_id` - The credential to renew
/// * `new_expiry` - The new expiration ledger height (0 = no expiration)
pub fn renew_credential(env: &Env, credential_id: u64, new_expiry: u32) {
    let admin: Address = env
        .storage()
        .persistent()
        .get(&CredentialDataKey::Admin)
        .expect("not initialized");
    admin.require_auth();

    let mut info: CredentialInfo = env
        .storage()
        .persistent()
        .get(&CredentialDataKey::Credential(credential_id))
        .expect("credential not found");

    if info.revoked {
        panic!("cannot renew revoked credential");
    }

    info.expires_at = new_expiry;
    crate::metadata::write_entry(env, &CredentialDataKey::Credential(credential_id), &info);

    env.events().publish(
        (Symbol::new(env, "credential_renewed"),),
        (credential_id, new_expiry),
    );
}
