use chainlearn_shared::{ContractMetadata, PERSISTENT_TTL_EXTEND_TO, PERSISTENT_TTL_THRESHOLD};
use soroban_sdk::{contracttype, Address, Env, Vec};

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenDataKey {
    Admin,
    Name,
    Symbol,
    Decimal,
    Balance(Address),
    Allowance(AllowanceKey),
    TotalSupply,
    RewardClaimed(RewardKey),
    ProgressTracker,
    MaxSupply,
    /// On-chain contract name/version, set on `initialize()` (#107).
    Metadata,
    /// Current transfer restriction configuration (#191).
    TransferRestriction,
    /// Whitelist of addresses allowed to receive tokens when WhitelistOnly (#191).
    Whitelist(Address),
    /// Snapshot of all balances at a given ledger height (#192).
    Snapshot(u32),
    /// Maps (address, ledger_height) to the balance at that snapshot (#192).
    SnapshotBalance(SnapshotBalanceKey),
    /// Registry of every spender an owner has ever approved, so expired
    /// allowances can be swept in bulk without an on-chain way to enumerate
    /// storage keys (#201).
    AllowanceSpenders(Address),
    /// Wasm hash of the code currently installed via `upgrade()` (#198).
    /// Unset until the first upgrade — the hash the contract was originally
    /// deployed with is not recorded on-chain by Soroban itself.
    WasmHash,
    /// Number of times `upgrade()` has been called (#198). Starts at 0.
    UpgradeVersion,
    /// Ledger sequence of the most recent transfer made by an address, used
    /// to enforce per-sender cooldown periods (#191).
    LastTransfer(Address),
    /// Cumulative amount ever minted to an address (#236).
    TotalMintedTo(Address),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransferRestriction {
    None,
    WhitelistOnly,
    Cooldown(u32),
    MaxAmount(i128),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotBalanceKey {
    pub address: Address,
    pub ledger_height: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllowanceKey {
    pub owner: Address,
    pub spender: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllowanceData {
    pub amount: i128,
    pub expiration_ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewardKey {
    pub learner: Address,
    pub course_id: soroban_sdk::Symbol,
    pub quiz_id: soroban_sdk::Symbol,
}

// ── Storage Helpers ───────────────────────────────────────────────────────────

/// Check whether the contract has been initialized.
pub fn is_initialized(env: &Env) -> bool {
    env.storage().persistent().has(&TokenDataKey::Admin)
}

/// Store the admin address.
pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().persistent().set(&TokenDataKey::Admin, admin);
}

/// Retrieve the admin address.
pub fn get_admin(env: &Env) -> Address {
    env.storage()
        .persistent()
        .get(&TokenDataKey::Admin)
        .expect("contract not initialized")
}

/// Get the balance for a given address.
pub fn get_balance(env: &Env, address: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&TokenDataKey::Balance(address.clone()))
        .unwrap_or(0)
}

/// Set the balance for a given address.
pub fn set_balance(env: &Env, address: &Address, amount: i128) {
    env.storage()
        .persistent()
        .set(&TokenDataKey::Balance(address.clone()), &amount);
}

/// Get the total supply.
pub fn get_total_supply(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get(&TokenDataKey::TotalSupply)
        .unwrap_or(0)
}

/// Set the total supply.
pub fn set_total_supply(env: &Env, amount: i128) {
    env.storage()
        .persistent()
        .set(&TokenDataKey::TotalSupply, &amount);
}

/// Get the allowance for an owner-spender pair.
/// Returns 0 if the allowance has expired or does not exist.
pub fn get_allowance(env: &Env, owner: &Address, spender: &Address) -> i128 {
    let key = AllowanceKey {
        owner: owner.clone(),
        spender: spender.clone(),
    };
    let data_key = TokenDataKey::Allowance(key);
    match env
        .storage()
        .temporary()
        .get::<TokenDataKey, AllowanceData>(&data_key)
    {
        Some(data) => {
            if env.ledger().sequence() > data.expiration_ledger {
                env.storage().temporary().remove(&data_key);
                0
            } else {
                data.amount
            }
        }
        None => 0,
    }
}

/// Read-only version of get_allowance that does not perform storage side-effects.
pub fn get_allowance_readonly(env: &Env, owner: &Address, spender: &Address) -> i128 {
    let key = AllowanceKey {
        owner: owner.clone(),
        spender: spender.clone(),
    };
    let data_key = TokenDataKey::Allowance(key);
    match env
        .storage()
        .temporary()
        .get::<TokenDataKey, AllowanceData>(&data_key)
    {
        Some(data) => {
            if env.ledger().sequence() > data.expiration_ledger {
                0
            } else {
                data.amount
            }
        }
        None => 0,
    }
}

/// Check if an allowance exists and is expired.
/// Returns (exists, expired, expiration_ledger).
pub fn check_allowance_expired(env: &Env, owner: &Address, spender: &Address) -> (bool, bool, u32) {
    let key = AllowanceKey {
        owner: owner.clone(),
        spender: spender.clone(),
    };
    let data_key = TokenDataKey::Allowance(key);
    match env
        .storage()
        .temporary()
        .get::<TokenDataKey, AllowanceData>(&data_key)
    {
        Some(data) => {
            let is_expired = env.ledger().sequence() > data.expiration_ledger;
            if is_expired {
                env.storage().temporary().remove(&data_key);
            }
            (true, is_expired, data.expiration_ledger)
        }
        None => (false, false, 0),
    }
}

/// Read-only version of check_allowance_expired that does not perform storage side-effects.
pub fn check_allowance_expired_readonly(
    env: &Env,
    owner: &Address,
    spender: &Address,
) -> (bool, bool, u32) {
    let key = AllowanceKey {
        owner: owner.clone(),
        spender: spender.clone(),
    };
    let data_key = TokenDataKey::Allowance(key);
    match env
        .storage()
        .temporary()
        .get::<TokenDataKey, AllowanceData>(&data_key)
    {
        Some(data) => {
            let is_expired = env.ledger().sequence() > data.expiration_ledger;
            (true, is_expired, data.expiration_ledger)
        }
        None => (false, false, 0),
    }
}

/// Set the allowance for an owner-spender pair with an expiration ledger.
///
/// Allowances are short-lived by nature -- every entry already carries its own
/// `expiration_ledger` -- so they belong in temporary storage rather than
/// persistent storage, which is priced for data meant to live indefinitely
/// (#110). The entry's TTL is extended to cover exactly the ledgers until it
/// expires; once past `expiration_ledger` the network is free to archive it
/// without the contract paying rent to keep dead allowances around.
pub fn set_allowance(
    env: &Env,
    owner: &Address,
    spender: &Address,
    amount: i128,
    expiration_ledger: u32,
) {
    let key = AllowanceKey {
        owner: owner.clone(),
        spender: spender.clone(),
    };
    let data_key = TokenDataKey::Allowance(key);
    let data = AllowanceData {
        amount,
        expiration_ledger,
    };
    env.storage().temporary().set(&data_key, &data);
    extend_allowance_ttl(env, &data_key, expiration_ledger);
}

/// Reduce the allowance amount while preserving the expiration ledger.
pub fn reduce_allowance(env: &Env, owner: &Address, spender: &Address, spend: i128) {
    let key = AllowanceKey {
        owner: owner.clone(),
        spender: spender.clone(),
    };
    let data_key = TokenDataKey::Allowance(key);
    let data: AllowanceData = env
        .storage()
        .temporary()
        .get(&data_key)
        .expect("allowance not set");
    let new_amount = data.amount - spend;
    let updated = AllowanceData {
        amount: new_amount,
        expiration_ledger: data.expiration_ledger,
    };
    env.storage().temporary().set(&data_key, &updated);
}

/// Extend a temporary allowance entry's TTL so it survives at least until its
/// `expiration_ledger`, since `set()` on temporary storage does not by itself
/// guarantee the entry lives past the current ledger.
fn extend_allowance_ttl(env: &Env, data_key: &TokenDataKey, expiration_ledger: u32) {
    let ttl = expiration_ledger.saturating_sub(env.ledger().sequence());
    if ttl > 0 {
        env.storage().temporary().extend_ttl(data_key, ttl, ttl);
    }
}

/// Check if a reward has already been claimed for a given learner + course + quiz.
pub fn is_reward_claimed(
    env: &Env,
    learner: &Address,
    course_id: &soroban_sdk::Symbol,
    quiz_id: &soroban_sdk::Symbol,
) -> bool {
    let key = RewardKey {
        learner: learner.clone(),
        course_id: course_id.clone(),
        quiz_id: quiz_id.clone(),
    };
    env.storage()
        .persistent()
        .get(&TokenDataKey::RewardClaimed(key))
        .unwrap_or(false)
}

/// Mark a reward as claimed.
pub fn set_reward_claimed(
    env: &Env,
    learner: &Address,
    course_id: &soroban_sdk::Symbol,
    quiz_id: &soroban_sdk::Symbol,
) {
    let key = RewardKey {
        learner: learner.clone(),
        course_id: course_id.clone(),
        quiz_id: quiz_id.clone(),
    };
    let data_key = TokenDataKey::RewardClaimed(key);
    env.storage().persistent().set(&data_key, &true);
    env.storage().persistent().extend_ttl(
        &data_key,
        PERSISTENT_TTL_THRESHOLD,
        PERSISTENT_TTL_EXTEND_TO,
    );
}

/// Store the progress-tracker contract address.
pub fn set_progress_tracker(env: &Env, address: &Address) {
    env.storage()
        .persistent()
        .set(&TokenDataKey::ProgressTracker, address);
}

/// Retrieve the progress-tracker contract address.
pub fn get_progress_tracker(env: &Env) -> Address {
    env.storage()
        .persistent()
        .get(&TokenDataKey::ProgressTracker)
        .expect("progress tracker not set")
}

/// Store the maximum supply cap.
pub fn set_max_supply(env: &Env, cap: i128) {
    env.storage()
        .persistent()
        .set(&TokenDataKey::MaxSupply, &cap);
}

/// Retrieve the maximum supply cap.
pub fn get_max_supply(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get(&TokenDataKey::MaxSupply)
        .expect("max supply not set")
}

/// Store the contract's on-chain name/version metadata (#107).
pub fn set_contract_metadata(env: &Env) {
    env.storage().persistent().set(
        &TokenDataKey::Metadata,
        &ContractMetadata::new(env, "learn-token"),
    );
}

/// Retrieve the contract's on-chain name/version metadata (#107).
pub fn get_contract_metadata(env: &Env) -> ContractMetadata {
    env.storage()
        .persistent()
        .get(&TokenDataKey::Metadata)
        .expect("not initialized")
}

// ── Transfer Restriction (#191) ─────────────────────────────────────────────

/// Get the current transfer restriction.
pub fn get_transfer_restriction(env: &Env) -> TransferRestriction {
    env.storage()
        .persistent()
        .get(&TokenDataKey::TransferRestriction)
        .unwrap_or(TransferRestriction::None)
}

/// Set the transfer restriction.
pub fn set_transfer_restriction(env: &Env, restriction: &TransferRestriction) {
    env.storage()
        .persistent()
        .set(&TokenDataKey::TransferRestriction, restriction);
}

/// Check if an address is on the whitelist.
pub fn is_whitelisted(env: &Env, address: &Address) -> bool {
    env.storage()
        .persistent()
        .get::<TokenDataKey, bool>(&TokenDataKey::Whitelist(address.clone()))
        .unwrap_or(false)
}

/// Add an address to the whitelist.
pub fn add_to_whitelist(env: &Env, address: &Address) {
    env.storage()
        .persistent()
        .set(&TokenDataKey::Whitelist(address.clone()), &true);
}

/// Remove an address from the whitelist.
pub fn remove_from_whitelist(env: &Env, address: &Address) {
    env.storage()
        .persistent()
        .remove(&TokenDataKey::Whitelist(address.clone()));
}

/// Record the ledger sequence of the most recent transfer made by `sender`.
/// Used by the `Cooldown` restriction to enforce a per-sender delay between
/// consecutive transfers.
///
/// The entry lives in temporary storage because it only needs to survive until
/// the cooldown window passes; after that it is inert and can be garbage
/// collected by the network.
pub fn set_last_transfer_ledger(env: &Env, sender: &Address, ledger: u32) {
    let key = TokenDataKey::LastTransfer(sender.clone());
    // Keep the entry alive for at least as long as the cooldown could be
    // checked.  We use a generous TTL (7 days ≈ 100_800 ledgers at 6s/ledger)
    // so a sender cannot bypass the cooldown simply by waiting for the entry
    // to expire.
    let ttl: u32 = 100_800;
    env.storage().temporary().set(&key, &ledger);
    env.storage().temporary().extend_ttl(&key, ttl, ttl);
}

/// Retrieve the ledger sequence of the sender's most recent transfer, if any.
pub fn get_last_transfer_ledger(env: &Env, sender: &Address) -> Option<u32> {
    env.storage()
        .temporary()
        .get(&TokenDataKey::LastTransfer(sender.clone()))
}

// ── Snapshots (#192) ────────────────────────────────────────────────────────

/// Store a snapshot of an address's balance at a given ledger height.
pub fn set_snapshot_balance(env: &Env, address: &Address, ledger_height: u32, balance: i128) {
    let key = TokenDataKey::SnapshotBalance(SnapshotBalanceKey {
        address: address.clone(),
        ledger_height,
    });
    env.storage().persistent().set(&key, &balance);
}

/// Get a snapshot of an address's balance at a given ledger height.
/// Returns None if no snapshot exists for that ledger height.
pub fn get_snapshot_balance(env: &Env, address: &Address, ledger_height: u32) -> Option<i128> {
    let key = TokenDataKey::SnapshotBalance(SnapshotBalanceKey {
        address: address.clone(),
        ledger_height,
    });
    env.storage().persistent().get(&key)
}

// ── Allowance Spender Registry (#201) ────────────────────────────────────────
//
// Soroban contract storage has no key-enumeration API, so a permissionless
// "clean up every expired allowance for this owner" function has no way to
// discover which spenders an owner has ever approved unless the contract
// keeps its own index. This registry is that index: every `approve()` /
// `increase_allowance()` records the spender here (deduplicated), and
// `cleanup_expired_allowances` reads it back to know which (owner, spender)
// pairs to check.

/// Record that `owner` has an allowance entry for `spender`, if not already
/// tracked. Idempotent — safe to call on every approval.
pub fn track_allowance_spender(env: &Env, owner: &Address, spender: &Address) {
    let key = TokenDataKey::AllowanceSpenders(owner.clone());
    let mut spenders: Vec<Address> = env.storage().persistent().get(&key).unwrap_or(Vec::new(env));
    if !spenders.contains(spender) {
        spenders.push_back(spender.clone());
        env.storage().persistent().set(&key, &spenders);
    }
}

/// Every spender `owner` has ever been tracked as approving (may include
/// spenders whose allowance has since expired or been fully spent).
pub fn get_allowance_spenders(env: &Env, owner: &Address) -> Vec<Address> {
    let key = TokenDataKey::AllowanceSpenders(owner.clone());
    env.storage().persistent().get(&key).unwrap_or(Vec::new(env))
}

/// Replace `owner`'s tracked-spender list wholesale (used after a cleanup
/// pass removes the entries that turned out to be expired).
pub fn set_allowance_spenders(env: &Env, owner: &Address, spenders: &Vec<Address>) {
    let key = TokenDataKey::AllowanceSpenders(owner.clone());
    env.storage().persistent().set(&key, spenders);
}

// ── Upgradeability (#198) ─────────────────────────────────────────────────────

/// Store the wasm hash the contract was most recently upgraded to.
pub fn set_wasm_hash(env: &Env, wasm_hash: &soroban_sdk::BytesN<32>) {
    env.storage()
        .persistent()
        .set(&TokenDataKey::WasmHash, wasm_hash);
}

/// The wasm hash the contract was most recently upgraded to, or `None` if
/// `upgrade()` has never been called.
pub fn get_wasm_hash(env: &Env) -> Option<soroban_sdk::BytesN<32>> {
    env.storage().persistent().get(&TokenDataKey::WasmHash)
}

/// Increment and return the upgrade counter (starts at 0, so the first
/// upgrade returns 1).
pub fn increment_upgrade_version(env: &Env) -> u32 {
    let next = get_upgrade_version(env) + 1;
    env.storage()
        .persistent()
        .set(&TokenDataKey::UpgradeVersion, &next);
    next
}

/// Number of times the contract has been upgraded via `upgrade()`.
pub fn get_upgrade_version(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get(&TokenDataKey::UpgradeVersion)
        .unwrap_or(0)
}
/// Get the cumulative amount ever minted to an address (#236).
///
/// Returns 0 for an address that has never been minted to.
pub fn get_total_minted_to(env: &Env, address: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&TokenDataKey::TotalMintedTo(address.clone()))
        .unwrap_or(0)
}

/// Add `amount` to the cumulative minted total for `address` (#236).
///
/// Called on every mint path so the running total stays in step with the
/// balance changes that produced it.
pub fn add_total_minted_to(env: &Env, address: &Address, amount: i128) {
    let data_key = TokenDataKey::TotalMintedTo(address.clone());
    let current: i128 = env.storage().persistent().get(&data_key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&data_key, &(current + amount));
    env.storage().persistent().extend_ttl(
        &data_key,
        PERSISTENT_TTL_THRESHOLD,
        PERSISTENT_TTL_EXTEND_TO,
    );
}
