//! Contract events. Pause events match learn-token's structure (#430).

use soroban_sdk::{Address, Env, Symbol};

/// Emitted when the contract is paused by an admin.
///
/// Topics: ["paused"]
/// Data: (admin, timestamp)
pub fn paused(env: &Env, admin: &Address, timestamp: u64) {
    let topics = (Symbol::new(env, "paused"),);
    env.events().publish(topics, (admin, timestamp));
}

/// Emitted when the contract is unpaused by an admin.
///
/// Topics: ["unpaused"]
/// Data: (admin, timestamp)
pub fn unpaused(env: &Env, admin: &Address, timestamp: u64) {
    let topics = (Symbol::new(env, "unpaused"),);
    env.events().publish(topics, (admin, timestamp));
}
