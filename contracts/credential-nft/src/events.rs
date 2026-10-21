//! Contract event definitions.
//!
//! Mirrors the `learn-token` event module so the three contracts emit a
//! consistent set of operational events (#422). Pause events previously had no
//! home here, so `pause`/`unpause` were the only admin operations in
//! this contract that monitoring tooling could not observe.

use soroban_sdk::{Address, Env, Symbol};

use crate::metadata::AdminRole;

// ── Event Indexing Convention (#548) ─────────────────────────────────────────
//
// Every event below puts its event-name `Symbol` in `topics[0]` (unchanged
// from before this change — any indexer already filtering on that symbol
// keeps working), then indexes the field(s) an indexer is most likely to
// filter by as additional topic slots, exactly like `learn-token`'s #200
// convention. Soroban's `getEvents` RPC filters match topics positionally with
// server-side indexing, but never inspect the `data` payload, so any field
// only present in `data` requires a full scan-and-decode of every event of
// that type to query by it. Before this change every credential event here
// used a single-symbol topic and pushed learner/course/credential ids into
// `data`, so "every credential minted for course X" or "everything that
// happened to credential 7" required exactly that full scan.
//
// Ordering is kept consistent so a client doesn't need per-event-type logic:
// the primary actor (the learner) is always topics[1], the secondary
// dimension (the course) is always topics[2] where one exists, and a bare
// `credential_id` occupies topics[1] for events that carry no learner/course
// dimension. Indexed fields move out of `data` rather than being duplicated
// there, matching the `learn-token` convention.

/// Emitted when a credential is minted.
///
/// Topics: ["credential_minted", learner, course_id] — indexed so "every
/// credential minted for course X" or "every credential learner Y earned" is
/// a server-side topic filter instead of a scan of every mint event.
/// Data: (credential_id, score, metadata_uri)
pub fn credential_minted(
    env: &Env,
    learner: &Address,
    course_id: &Symbol,
    credential_id: u64,
    score: u32,
    metadata_uri: &Symbol,
) {
    let topics = (
        Symbol::new(env, "credential_minted"),
        learner.clone(),
        course_id.clone(),
    );
    env.events()
        .publish(topics, (credential_id, score, metadata_uri.clone()));
}

/// Emitted when a credential is revoked by an admin.
///
/// Topics: ["credential_revoked", learner, course_id] — same indexed pair as
/// `credential_minted`, so an indexer can correlate a credential's issuance
/// and revocation with one topic shape.
/// Data: (credential_id, admin)
pub fn credential_revoked(
    env: &Env,
    learner: &Address,
    course_id: &Symbol,
    credential_id: u64,
    admin: &Address,
) {
    let topics = (
        Symbol::new(env, "credential_revoked"),
        learner.clone(),
        course_id.clone(),
    );
    env.events()
        .publish(topics, (credential_id, admin.clone()));
}

/// Emitted when a credential is revoked with a stated reason (#194).
///
/// Topics: ["credential_revoked", learner, course_id] — identical to the
/// reason-less variant so a revocation query covers both shapes.
/// Data: (credential_id, admin, reason)
pub fn credential_revoked_with_reason(
    env: &Env,
    learner: &Address,
    course_id: &Symbol,
    credential_id: u64,
    admin: &Address,
    reason: &Symbol,
) {
    let topics = (
        Symbol::new(env, "credential_revoked"),
        learner.clone(),
        course_id.clone(),
    );
    env.events().publish(
        topics,
        (credential_id, admin.clone(), reason.clone()),
    );
}

/// Emitted when a credential's expiration is renewed (#193).
///
/// Topics: ["credential_renewed", credential_id] — indexed by the credential
/// so "what happened to credential N" is one topic filter across the whole
/// credential lifecycle.
/// Data: (new_expiry,)
pub fn credential_renewed(env: &Env, credential_id: u64, new_expiry: u32) {
    let topics = (Symbol::new(env, "credential_renewed"), credential_id);
    env.events().publish(topics, (new_expiry,));
}

/// Emitted when a credential's metadata URI is replaced (#243).
///
/// Topics: ["credential_metadata_updated", credential_id] — same credential
/// slot as `credential_renewed`, so the lifecycle of one credential can be
/// traced with a single topic filter.
/// Data: (new_metadata_uri,)
pub fn credential_metadata_updated(env: &Env, credential_id: u64, new_metadata_uri: &Symbol) {
    let topics = (
        Symbol::new(env, "credential_metadata_updated"),
        credential_id,
    );
    env.events().publish(topics, (new_metadata_uri.clone(),));
}

/// Emitted when display properties are set for a credential (#244).
///
/// Topics: ["credential_display_set", credential_id] — indexed by the
/// credential for the same cross-event correlation as above.
/// Data: (image_url, description, issuer_name)
pub fn credential_display_set(
    env: &Env,
    credential_id: u64,
    image_url: &Option<Symbol>,
    description: &Option<Symbol>,
    issuer_name: &Option<Symbol>,
) {
    let topics = (Symbol::new(env, "credential_display_set"), credential_id);
    env.events().publish(
        topics,
        (
            image_url.clone(),
            description.clone(),
            issuer_name.clone(),
        ),
    );
}

/// Emitted when a course completion certificate URI is generated (#223).
///
/// Topics: ["certificate_generated", learner, course_id] — indexed so "every
/// certificate generated for learner X" and "for course Y" are topic filters.
/// Data: (cert_uri,)
pub fn certificate_generated(
    env: &Env,
    learner: &Address,
    course_id: &Symbol,
    cert_uri: &Symbol,
) {
    let topics = (
        Symbol::new(env, "certificate_generated"),
        learner.clone(),
        course_id.clone(),
    );
    env.events().publish(topics, (cert_uri.clone(),));
}

/// Emitted when the contract is paused by an admin (#422).
///
/// Topics: ["paused"] — a rare, contract-wide transition event; there is no
/// per-address or per-credential query pattern to index, and `learn-token`'s
/// `paused` event uses the same single-symbol shape.
/// Data: (admin, timestamp)
pub fn paused(env: &Env, admin: &Address, timestamp: u64) {
    let topics = (Symbol::new(env, "paused"),);
    env.events().publish(topics, (admin, timestamp));
}

/// Emitted when the contract is unpaused by an admin (#422).
///
/// Topics: ["unpaused"]
/// Data: (admin, timestamp)
pub fn unpaused(env: &Env, admin: &Address, timestamp: u64) {
    let topics = (Symbol::new(env, "unpaused"),);
    env.events().publish(topics, (admin, timestamp));
}

/// Emitted when an admin transfer is initiated; the transfer is not yet
/// effective (#423).
///
/// Topics: ["admin_transfer_initiated", new_admin]
/// Data: (current_admin, initiated_at, accept_after)
pub fn admin_transfer_initiated(
    env: &Env,
    current_admin: &Address,
    new_admin: &Address,
    initiated_at: u64,
    accept_after: u64,
) {
    let topics = (
        Symbol::new(env, "admin_transfer_initiated"),
        new_admin.clone(),
    );
    env.events()
        .publish(topics, (current_admin.clone(), initiated_at, accept_after));
}

/// Emitted when a pending admin transfer is accepted and takes effect (#423).
///
/// Topics: ["admin_transfer_accepted", new_admin]
/// Data: (previous_admin,)
pub fn admin_transfer_accepted(env: &Env, previous_admin: &Address, new_admin: &Address) {
    let topics = (
        Symbol::new(env, "admin_transfer_accepted"),
        new_admin.clone(),
    );
    env.events().publish(topics, (previous_admin.clone(),));
}

/// Emitted when a pending admin transfer is cancelled before acceptance
/// (#423).
///
/// Topics: ["admin_transfer_cancelled", new_admin] — same topic slot as the
/// other two admin-transfer events, so a client can correlate the lifecycle of
/// one candidate transfer with a single topic filter.
/// Data: (current_admin,)
pub fn admin_transfer_cancelled(env: &Env, current_admin: &Address, new_admin: &Address) {
    let topics = (
        Symbol::new(env, "admin_transfer_cancelled"),
        new_admin.clone(),
    );
    env.events().publish(topics, (current_admin.clone(),));
}

// ── Role Management Events (#551) ────────────────────────────────────────────

/// Emitted when a role is granted to an address (#551).
///
/// Topics: ["role_granted", address]
/// Data: (role_symbol,)
pub fn role_granted(env: &Env, address: &Address, role: &AdminRole) {
    let role_str = match role {
        AdminRole::Admin => "Admin",
        AdminRole::Minter => "Minter",
        AdminRole::Pauser => "Pauser",
    };
    let topics = (Symbol::new(env, "role_granted"), address.clone());
    env.events()
        .publish(topics, (Symbol::new(env, role_str),));
}

/// Emitted when a role is revoked from an address (#551).
///
/// Topics: ["role_revoked", address]
/// Data: (role_symbol,)
pub fn role_revoked(env: &Env, address: &Address, role: &AdminRole) {
    let role_str = match role {
        AdminRole::Admin => "Admin",
        AdminRole::Minter => "Minter",
        AdminRole::Pauser => "Pauser",
    };
    let topics = (Symbol::new(env, "role_revoked"), address.clone());
    env.events()
        .publish(topics, (Symbol::new(env, role_str),));
}
