use soroban_sdk::{Address, BytesN, Env, Symbol};

// ── Event Indexing Convention (#200) ─────────────────────────────────────────
//
// Every event below puts its event-name `Symbol` in `topics[0]` (unchanged
// from before this change — any indexer already filtering on that symbol
// keeps working), then indexes the field(s) an indexer is most likely to
// filter by (an owner/learner/course address or id) as additional topic
// slots. Soroban's `getEvents` RPC filters match topics positionally with
// server-side indexing, but never inspects the `data` payload, so any field
// only present in `data` requires a full scan-and-decode of every event of
// that type to query by it. Before this change every event here used a
// single-symbol topic and pushed all addresses/ids into `data`, so "every
// transfer touching address X" or "every reward claimed for course Y"
// required exactly that full scan.
//
// Ordering is kept consistent across related events so a client doesn't
// need per-event-type logic to find "the counterparty address topic": the
// primary actor (`from`/`owner`/`learner`) is always topics[1], and the
// secondary party (`to`/`spender`/`course_id`) is always topics[2] where one
// exists.

/// Emitted when a learner claims a reward for completing a quiz.
///
/// Topics: ["reward", learner, course_id] — indexed so "every reward claimed
/// by learner X" or "every reward claimed for course Y" can be queried
/// server-side instead of scanning every reward_claimed event's payload.
/// Data: (quiz_id, score, reward_amount)
pub fn reward_claimed(
    env: &Env,
    learner: &Address,
    quiz_id: &Symbol,
    score: u32,
    reward_amount: i128,
    course_id: &Symbol,
) {
    let topics = (Symbol::new(env, "reward"), learner.clone(), course_id.clone());
    env.events().publish(topics, (quiz_id, score, reward_amount));
}

/// Emitted when tokens are transferred directly.
///
/// Topics: ["transfer", from, to] — matches the SEP-41 reference token
/// convention, so "every transfer touching address X" is a server-side
/// topic filter rather than a full scan.
/// Data: (amount,)
pub fn transfer(env: &Env, from: &Address, to: &Address, amount: i128) {
    let topics = (Symbol::new(env, "transfer"), from.clone(), to.clone());
    env.events().publish(topics, (amount,));
}

/// Emitted when tokens are transferred on behalf of another address (delegated).
///
/// Topics: ["transfer_from", from, to] — `from`/`to` occupy the same topic
/// positions as the plain `transfer` event, so a query for "everything that
/// moved address X's tokens" can filter on one topic shape across both event
/// kinds. `spender` (who was delegated, rather than whose funds moved) stays
/// in `data`.
/// Data: (spender, amount)
pub fn transfer_from(env: &Env, spender: &Address, from: &Address, to: &Address, amount: i128) {
    let topics = (Symbol::new(env, "transfer_from"), from.clone(), to.clone());
    env.events().publish(topics, (spender, amount));
}

/// Emitted when tokens are burned by their owner.
///
/// Topics: ["burn", from] — `from` in the same topic slot `transfer`/
/// `transfer_from` use for the balance-reducing party.
/// Data: (amount,)
pub fn burn(env: &Env, from: &Address, amount: i128) {
    let topics = (Symbol::new(env, "burn"), from.clone());
    env.events().publish(topics, (amount,));
}

/// Emitted when tokens are burned by an approved spender (delegated).
///
/// Topics: ["burn_from", from] — same topic position as `burn`, so "every
/// burn affecting address X" is one filter shape regardless of who
/// triggered it. `spender` stays in `data`.
/// Data: (spender, amount)
pub fn burn_from(env: &Env, spender: &Address, from: &Address, amount: i128) {
    let topics = (Symbol::new(env, "burn_from"), from.clone());
    env.events().publish(topics, (spender, amount));
}

/// Emitted when tokens are minted.
///
/// Topics: ["mint", to] — indexed so "every mint to address X" doesn't
/// require scanning every mint event.
/// Data: (amount,)
pub fn mint(env: &Env, to: &Address, amount: i128) {
    let topics = (Symbol::new(env, "mint"), to.clone());
    env.events().publish(topics, (amount,));
}

/// Emitted when an allowance is set.
///
/// Topics: ["approve", owner, spender] — both parties of an approval are
/// frequently queried together ("what did X approve", "what can Y spend"),
/// so both are indexed.
/// Data: (amount, expiration_ledger)
pub fn approve(
    env: &Env,
    owner: &Address,
    spender: &Address,
    amount: i128,
    expiration_ledger: u32,
) {
    let topics = (Symbol::new(env, "approve"), owner.clone(), spender.clone());
    env.events().publish(topics, (amount, expiration_ledger));
}

/// Emitted when the progress-tracker address is updated (#75).
///
/// Topics: ["progress"] — a rare, admin-only, singleton-config event; there
/// is no per-address query pattern to index.
/// Data: (new_address,)
pub fn progress_tracker_updated(env: &Env, new_address: &Address) {
    let topics = (Symbol::new(env, "progress"),);
    env.events().publish(topics, (new_address,));
}

/// Emitted when an allowance expires or is accessed after expiration.
///
/// Topics: ["allowance_expired", owner, spender] — same indexed pair as
/// `approve`, so an indexer can correlate an allowance's creation and its
/// expiry with one topic shape.
/// Data: (expiration_ledger,)
pub fn allowance_expired(env: &Env, owner: &Address, spender: &Address, expiration_ledger: u32) {
    let topics = (
        Symbol::new(env, "allowance_expired"),
        owner.clone(),
        spender.clone(),
    );
    env.events().publish(topics, (expiration_ledger,));
}

/// Emitted when the transfer restriction is updated (#191).
///
/// Topics: ["restriction_updated"] — a rare, admin-only, contract-wide
/// config event; there is no per-address query pattern to index.
/// Data: (restriction,)
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
/// Topics: ["whitelist_updated", address] — indexed so "is/was address X
/// whitelisted" is a topic filter instead of a scan.
/// Data: (added,)
pub fn whitelist_updated(env: &Env, address: &Address, added: bool) {
    let topics = (Symbol::new(env, "whitelist_updated"), address.clone());
    env.events().publish(topics, (added,));
}

/// Emitted when a token snapshot is created (#192).
///
/// Topics: ["snapshot_created"] — a contract-wide event with no per-address
/// dimension to index.
/// Data: (ledger_height,)
pub fn snapshot_created(env: &Env, ledger_height: u32) {
    let topics = (Symbol::new(env, "snapshot_created"),);
    env.events().publish(topics, (ledger_height,));
}

/// Emitted when the contract's wasm code is upgraded (#198).
///
/// Topics: ["upgraded"] — a rare, admin-only, contract-wide event; there is
/// no per-address query pattern to index.
/// Data: (new_wasm_hash, upgrade_version)
pub fn upgraded(env: &Env, new_wasm_hash: &BytesN<32>, upgrade_version: u32) {
    let topics = (Symbol::new(env, "upgraded"),);
    env.events()
        .publish(topics, (new_wasm_hash.clone(), upgrade_version));
}

/// Emitted when the contract is paused by an admin (#238).
///
/// Topics: ["paused"]
/// Data: (admin, timestamp)
pub fn paused(env: &Env, admin: &Address, timestamp: u64) {
    let topics = (Symbol::new(env, "paused"),);
    env.events().publish(topics, (admin, timestamp));
}

/// Emitted when the contract is unpaused by an admin (#238).
///
/// Topics: ["unpaused"]
/// Data: (admin, timestamp)
pub fn unpaused(env: &Env, admin: &Address, timestamp: u64) {
    let topics = (Symbol::new(env, "unpaused"),);
    env.events().publish(topics, (admin, timestamp));
}

/// Emitted when a vesting schedule is created (#225).
///
/// Topics: ["vesting_created", beneficiary]
/// Data: (total_amount, cliff_timestamp, duration_seconds)
pub fn vesting_created(
    env: &Env,
    beneficiary: &Address,
    total_amount: i128,
    cliff_timestamp: u64,
    duration_seconds: u64,
) {
    let topics = (Symbol::new(env, "vesting_created"), beneficiary.clone());
    env.events().publish(
        topics,
        (total_amount, cliff_timestamp, duration_seconds),
    );
}

/// Emitted when vested tokens are claimed (#225).
///
/// Topics: ["vesting_claimed", beneficiary]
/// Data: (claimed_amount, total_claimed)
pub fn vesting_claimed(
    env: &Env,
    beneficiary: &Address,
    claimed_amount: i128,
    total_claimed: i128,
) {
    let topics = (Symbol::new(env, "vesting_claimed"), beneficiary.clone());
    env.events().publish(topics, (claimed_amount, total_claimed));
}

/// Emitted when a governance proposal is created (#226).
///
/// Topics: ["proposal_created"]
/// Data: (proposal_id, start_time, end_time)
pub fn proposal_created(env: &Env, proposal_id: u64, start_time: u64, end_time: u64) {
    let topics = (Symbol::new(env, "proposal_created"),);
    env.events().publish(topics, (proposal_id, start_time, end_time));
}

/// Emitted when a vote is cast on a proposal (#226).
///
/// Topics: ["vote_cast", voter]
/// Data: (proposal_id, choice, voting_power)
pub fn vote_cast(
    env: &Env,
    proposal_id: u64,
    voter: &Address,
    choice: u32,
    voting_power: i128,
) {
    let topics = (Symbol::new(env, "vote_cast"), voter.clone());
    env.events().publish(topics, (proposal_id, choice, voting_power));
}

/// Emitted when a governance proposal is executed (#226).
///
/// Topics: ["proposal_executed"]
/// Data: (proposal_id, winning_choice, winning_votes)
pub fn proposal_executed(
    env: &Env,
    proposal_id: u64,
    winning_choice: u32,
    winning_votes: i128,
) {
    let topics = (Symbol::new(env, "proposal_executed"),);
    env.events().publish(topics, (proposal_id, winning_choice, winning_votes));
}
