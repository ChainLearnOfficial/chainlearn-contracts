//! Contract event definitions.
//!
//! Mirrors the `learn-token` event module so the three contracts emit a
//! consistent set of operational events (#422). Pause events previously had no
//! home here, so `emergency_pause`/`unpause` were the only admin operations in
//! this contract that monitoring tooling could not observe.

use soroban_sdk::{Address, Env, Symbol};

/// Emitted when the contract is paused by an admin (#422).
///
/// Topics: ["paused"]
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
