use soroban_sdk::{Address, Env, Symbol};

/// Emitted when a learner claims a reward for completing a quiz.
///
/// Topics: ["reward_claimed"]
/// Data: (learner, quiz_id, score, reward_amount, course_id)
/// Topics: ["reward"]
/// Data: (learner, quiz_id, score, reward_amount)
pub fn reward_claimed(
    env: &Env,
    learner: &Address,
    quiz_id: &Symbol,
    score: u32,
    reward_amount: i128,
    course_id: &Symbol,
) {
    // Symbol::new (not symbol_short!) so topic construction matches
    // progress-tracker and credential-nft, which indexers rely on (#118).
    let topics = (Symbol::new(env, "reward"),);
    env.events()
        .publish(topics, (learner, quiz_id, score, reward_amount, course_id));
}

/// Emitted when tokens are transferred directly.
///
/// Topics: ["transfer"]
/// Data: (from, to, amount)
pub fn transfer(env: &Env, from: &Address, to: &Address, amount: i128) {
    let topics = (Symbol::new(env, "transfer"),);
    env.events().publish(topics, (from, to, amount));
}

/// Emitted when tokens are transferred on behalf of another address (delegated).
///
/// Topics: ["transfer_from"]
/// Data: (spender, from, to, amount)
pub fn transfer_from(env: &Env, spender: &Address, from: &Address, to: &Address, amount: i128) {
    let topics = (Symbol::new(env, "transfer_from"),);
    env.events().publish(topics, (spender, from, to, amount));
}

/// Emitted when tokens are burned by their owner.
///
/// Topics: ["burn"]
/// Data: (from, amount)
pub fn burn(env: &Env, from: &Address, amount: i128) {
    let topics = (Symbol::new(env, "burn"),);
    env.events().publish(topics, (from, amount));
}

/// Emitted when tokens are burned by an approved spender (delegated).
///
/// Topics: ["burn_from"]
/// Data: (spender, from, amount)
pub fn burn_from(env: &Env, spender: &Address, from: &Address, amount: i128) {
    let topics = (Symbol::new(env, "burn_from"),);
    env.events().publish(topics, (spender, from, amount));
}

/// Emitted when tokens are minted.
///
/// Topics: ["mint"]
/// Data: (to, amount)
pub fn mint(env: &Env, to: &Address, amount: i128) {
    let topics = (Symbol::new(env, "mint"),);
    env.events().publish(topics, (to, amount));
}

/// Emitted when an allowance is set.
///
/// Topics: ["approve"]
/// Data: (owner, spender, amount, expiration_ledger)
pub fn approve(
    env: &Env,
    owner: &Address,
    spender: &Address,
    amount: i128,
    expiration_ledger: u32,
) {
    let topics = (Symbol::new(env, "approve"),);
    env.events()
        .publish(topics, (owner, spender, amount, expiration_ledger));
}

/// Emitted when the progress-tracker address is updated (#75).
///
/// Topics: ["progress"]
/// Data: (new_address,)
pub fn progress_tracker_updated(env: &Env, new_address: &Address) {
    let topics = (Symbol::new(env, "progress"),);
    env.events().publish(topics, (new_address,));
}

/// Emitted when an allowance expires or is accessed after expiration.
///
/// Topics: ["allowance_expired"]
/// Data: (owner, spender, expiration_ledger)
pub fn allowance_expired(env: &Env, owner: &Address, spender: &Address, expiration_ledger: u32) {
    let topics = (Symbol::new(env, "allowance_expired"),);
    env.events()
        .publish(topics, (owner, spender, expiration_ledger));
}

/// Emitted when the transfer restriction is updated (#191).
///
/// Topics: ["restriction_updated"]
/// Data: (restriction)
pub fn restriction_updated(env: &Env, restriction: &super::storage::TransferRestriction) {
    let topics = (Symbol::new(env, "restriction_updated"),);
    let restriction_str = match restriction {
        super::storage::TransferRestriction::None => "None",
        super::storage::TransferRestriction::WhitelistOnly => "WhitelistOnly",
        super::storage::TransferRestriction::Cooldown(_) => "Cooldown",
        super::storage::TransferRestriction::MaxAmount(_) => "MaxAmount",
    };
    env.events()
        .publish(topics, (Symbol::new(env, restriction_str),));
}

/// Emitted when an address is added to or removed from the whitelist (#191).
///
/// Topics: ["whitelist_updated"]
/// Data: (address, added)
pub fn whitelist_updated(env: &Env, address: &Address, added: bool) {
    let topics = (Symbol::new(env, "whitelist_updated"),);
    env.events().publish(topics, (address, added));
}

/// Emitted when a token snapshot is created (#192).
///
/// Topics: ["snapshot_created"]
/// Data: (ledger_height)
pub fn snapshot_created(env: &Env, ledger_height: u32) {
    let topics = (Symbol::new(env, "snapshot_created"),);
    env.events().publish(topics, (ledger_height,));
}
