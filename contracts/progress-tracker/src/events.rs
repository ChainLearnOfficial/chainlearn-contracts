//! Contract event definitions.
//!
//! Mirrors the `learn-token` event module so the three contracts emit a
//! consistent set of operational events (#422). Pause events previously had no
//! home here, so `pause`/`unpause` were the only admin operations in
//! this contract that monitoring tooling could not observe.

use soroban_sdk::{Address, BytesN, Env, Symbol, Vec};

use crate::types::{AdminRole, AchievementType};

// ── Event Indexing Convention (#200, #549) ───────────────────────────────────
//
// Every event below puts its event-name `Symbol` in `topics[0]` (unchanged
// from before this change — any indexer already filtering on that symbol
// keeps working), then indexes the field(s) an indexer is most likely to
// filter by (the learner address and/or the course id) as additional topic
// slots. Soroban's `getEvents` RPC filters match topics positionally with
// server-side indexing, but never inspects the `data` payload, so any field
// only present in `data` requires a full scan-and-decode of every event of
// that type to query by it. Before this change every learner/course event
// here used a single-symbol topic and pushed learner/course ids into `data`,
// so "every module completed by learner X" or "every event for course Y"
// required exactly that full scan.
//
// Ordering is kept consistent so a client doesn't need per-event-type logic
// to find the indexed fields: the primary actor (`learner`) is always
// topics[1], and the course the action happened in (`course_id`) is always
// topics[2] where one exists. Course-admin events that have no acting
// learner index `course_id` directly in topics[1].

/// Emitted when a new course is registered by an admin.
///
/// Topics: ["course_created", course_id] — indexed so "every event about
/// course Y" is one server-side topic filter.
/// Data: (total_modules, total_quizzes, module_ids)
pub fn course_created(
    env: &Env,
    course_id: &Symbol,
    total_modules: u32,
    total_quizzes: u32,
    module_ids: Vec<Symbol>,
) {
    let topics = (Symbol::new(env, "course_created"), course_id.clone());
    env.events()
        .publish(topics, (total_modules, total_quizzes, module_ids));
}

/// Emitted when a learner enrolls in a course.
///
/// Topics: ["enrolled", learner, course_id] — indexed so "every enrollment by
/// learner X" or "every enrollment in course Y" is a server-side topic filter
/// rather than a full scan.
/// Data: (enrolled_at,)
pub fn enrolled(env: &Env, learner: &Address, course_id: &Symbol, enrolled_at: u64) {
    let topics = (
        Symbol::new(env, "enrolled"),
        learner.clone(),
        course_id.clone(),
    );
    env.events().publish(topics, (enrolled_at,));
}

/// Emitted when a learner completes a module.
///
/// Topics: ["module_completed", learner, course_id] — same indexed pair as
/// `enrolled`, so a query for "every module completed by learner X" works
/// across events without decoding payloads.
/// Data: (module_id, overall_progress)
pub fn module_completed(
    env: &Env,
    learner: &Address,
    course_id: &Symbol,
    module_id: &Symbol,
    overall_progress: u32,
) {
    let topics = (
        Symbol::new(env, "module_completed"),
        learner.clone(),
        course_id.clone(),
    );
    env.events()
        .publish(topics, (module_id.clone(), overall_progress));
}

/// Emitted the moment a learner becomes eligible for a credential (#96).
///
/// Topics: ["credential_eligible", learner, course_id] — same indexed pair as
/// `enrolled`/`module_completed`, so "every credential-eligible flip for
/// course Y" is one server-side filter.
/// Data: none — every field is already indexed.
pub fn credential_eligible(env: &Env, learner: &Address, course_id: &Symbol) {
    let topics = (
        Symbol::new(env, "credential_eligible"),
        learner.clone(),
        course_id.clone(),
    );
    env.events().publish(topics, ());
}

/// Emitted when a learner submits a quiz score.
///
/// Topics: ["quiz_submitted", learner, course_id] — same indexed pair as the
/// other learner-progress events.
/// Data: (quiz_id, score)
pub fn quiz_submitted(
    env: &Env,
    learner: &Address,
    course_id: &Symbol,
    quiz_id: &Symbol,
    score: u32,
) {
    let topics = (
        Symbol::new(env, "quiz_submitted"),
        learner.clone(),
        course_id.clone(),
    );
    env.events().publish(topics, (quiz_id.clone(), score));
}

/// Emitted when a learner retakes a quiz with a higher score (#234).
///
/// Topics: ["quiz_retaken", learner, course_id] — same indexed pair as
/// `quiz_submitted`, so both attempts at a quiz share one filter shape.
/// Data: (quiz_id, previous_score, new_score)
pub fn quiz_retaken(
    env: &Env,
    learner: &Address,
    course_id: &Symbol,
    quiz_id: &Symbol,
    previous_score: u32,
    new_score: u32,
) {
    let topics = (
        Symbol::new(env, "quiz_retaken"),
        learner.clone(),
        course_id.clone(),
    );
    env.events()
        .publish(topics, (quiz_id.clone(), previous_score, new_score));
}

/// Emitted when an admin archives a course (#210).
///
/// Topics: ["course_archived", course_id] — same course slot as
/// `course_created`, so an indexer can follow one course's lifecycle with a
/// single filter shape.
/// Data: none — every field is already indexed.
pub fn course_archived(env: &Env, course_id: &Symbol) {
    let topics = (Symbol::new(env, "course_archived"), course_id.clone());
    env.events().publish(topics, ());
}

/// Emitted when a course's content hash is set or updated (#235).
///
/// Topics: ["content_hash_set", course_id] — same course slot as the other
/// course-admin events.
/// Data: (content_hash,)
pub fn content_hash_set(env: &Env, course_id: &Symbol, content_hash: &Symbol) {
    let topics = (Symbol::new(env, "content_hash_set"), course_id.clone());
    env.events().publish(topics, (content_hash.clone(),));
}

/// Emitted when a course's difficulty is set (#259).
///
/// Topics: ["course_difficulty_set", course_id] — same course slot as the
/// other course-admin events.
/// Data: (difficulty,)
pub fn course_difficulty_set(env: &Env, course_id: &Symbol, difficulty: u32) {
    let topics = (
        Symbol::new(env, "course_difficulty_set"),
        course_id.clone(),
    );
    env.events().publish(topics, (difficulty,));
}

/// Emitted when a course's tags are replaced (#260).
///
/// Topics: ["course_tags_set", course_id] — same course slot as the other
/// course-admin events.
/// Data: (tags,)
pub fn course_tags_set(env: &Env, course_id: &Symbol, tags: Vec<Symbol>) {
    let topics = (Symbol::new(env, "course_tags_set"), course_id.clone());
    env.events().publish(topics, (tags,));
}

/// Emitted when a course's version is updated (#245).
///
/// Topics: ["course_version_updated", course_id] — same course slot as the
/// other course-admin events.
/// Data: (new_version,)
pub fn course_version_updated(env: &Env, course_id: &Symbol, new_version: u32) {
    let topics = (
        Symbol::new(env, "course_version_updated"),
        course_id.clone(),
    );
    env.events().publish(topics, (new_version,));
}

/// Emitted when a course's prerequisites are set or replaced (#231).
///
/// Topics: ["prerequisites_set", course_id] — same course slot as the other
/// course-admin events.
/// Data: (prerequisites,)
pub fn prerequisites_set(env: &Env, course_id: &Symbol, prerequisites: Vec<Symbol>) {
    let topics = (Symbol::new(env, "prerequisites_set"), course_id.clone());
    env.events().publish(topics, (prerequisites,));
}

/// Emitted when a learner earns an achievement.
///
/// Topics: ["achievement_earned", learner] — indexed so "every achievement
/// earned by learner X" is a server-side topic filter.
/// Data: (achievement_type, course_id, timestamp)
pub fn achievement_earned(
    env: &Env,
    learner: &Address,
    achievement_type: AchievementType,
    course_id: Option<Symbol>,
    timestamp: u64,
) {
    let topics = (
        Symbol::new(env, "achievement_earned"),
        learner.clone(),
    );
    env.events()
        .publish(topics, (achievement_type, course_id, timestamp));
}

/// Emitted when a learner delegates progress tracking to another address
/// (#222).
///
/// Topics: ["progress_delegated", learner] — indexed so "every delegation by
/// learner X" is a server-side topic filter.
/// Data: (delegate,)
pub fn progress_delegated(env: &Env, learner: &Address, delegate: &Address) {
    let topics = (
        Symbol::new(env, "progress_delegated"),
        learner.clone(),
    );
    env.events().publish(topics, (delegate.clone(),));
}

/// Emitted when a learner revokes their progress-tracking delegation (#222).
///
/// Topics: ["delegation_revoked", learner] — same learner slot as
/// `progress_delegated`, so the delegation lifecycle of one learner shares
/// one filter shape.
/// Data: none — every field is already indexed.
pub fn delegation_revoked(env: &Env, learner: &Address) {
    let topics = (
        Symbol::new(env, "delegation_revoked"),
        learner.clone(),
    );
    env.events().publish(topics, ());
}

/// Emitted when the contract's wasm is upgraded in place (#219).
///
/// Topics: ["upgraded"] — a rare, admin-only, contract-wide event; there is
/// no per-address or per-course dimension to index.
/// Data: (new_wasm_hash, upgrade_version)
pub fn upgraded(env: &Env, new_wasm_hash: &BytesN<32>, upgrade_version: u32) {
    let topics = (Symbol::new(env, "upgraded"),);
    env.events()
        .publish(topics, (new_wasm_hash.clone(), upgrade_version));
}

// ── Pause and admin-transfer lifecycle (#422, #423) ─────────────────────────

/// Emitted when the contract is paused by an admin (#422).
///
/// Topics: ["paused"] — a rare, contract-wide admin event with no per-address
/// or per-course dimension to index.
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
