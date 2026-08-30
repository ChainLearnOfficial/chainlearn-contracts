use chainlearn_shared::{ContractMetadata, PERSISTENT_TTL_EXTEND_TO, PERSISTENT_TTL_THRESHOLD};
use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

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
    /// Role assignments per address (#190).
    Role(RoleKey),
    /// Cumulative amount ever minted to an address (#236).
    TotalMintedTo(Address),
    /// Append-only list of a learner's reward claims (#237).
    ClaimHistory(Address),
    /// Whether the contract is currently paused (#238).
    Paused,
    /// Vesting schedule for a beneficiary (#225).
    VestingSchedule(Address),
    /// Amount already claimed from a vesting schedule (#225).
    VestingClaimed(Address),
    /// Governance proposals, keyed by proposal ID (#226).
    Proposal(u64),
    /// Proposal counter (#226).
    ProposalCounter,
    /// Per-voter vote record for a proposal (#226).
    Vote(ProposalVoteKey),
    /// Per-address permit nonce for replay protection (#224).
    PermitNonce(Address),
    /// Count of persistent entries the contract has created (#254).
    StorageEntryCount,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdminRole {
    Admin,
    Minter,
    Pauser,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleKey {
    pub address: Address,
    pub role: AdminRole,
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

/// A single reward claim, recorded for a learner's history (#237).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimRecord {
    /// The course the quiz belonged to.
    pub course_id: Symbol,
    /// The quiz that was claimed.
    pub quiz_id: Symbol,
    /// Reward amount minted for the claim.
    pub amount: i128,
    /// Ledger timestamp when the claim was made.
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewardKey {
    pub learner: Address,
    pub course_id: soroban_sdk::Symbol,
    pub quiz_id: soroban_sdk::Symbol,
}

/// A token vesting schedule for a beneficiary (#225).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VestingSchedule {
    /// Total tokens to vest.
    pub total_amount: i128,
    /// Ledger timestamp after which tokens begin vesting.
    pub cliff_timestamp: u64,
    /// Duration in seconds over which tokens vest linearly after cliff.
    pub duration_seconds: u64,
    /// Timestamp when the schedule was created.
    pub created_at: u64,
    /// Whether the schedule has been fully claimed.
    pub exhausted: bool,
}

/// A governance proposal (#226).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    /// Human-readable description of the proposal.
    pub description: soroban_sdk::String,
    /// Number of choices (e.g. 2 = yes/no).
    pub choices: u32,
    /// Ledger timestamp when voting opens.
    pub start_time: u64,
    /// Ledger timestamp when voting closes.
    pub end_time: u64,
    /// Snapshot ledger height used for voting power.
    pub snapshot_ledger: u32,
    /// Total votes cast per choice (index 0 = choice 1, etc.).
    pub vote_totals: soroban_sdk::Vec<i128>,
    /// Whether the proposal has been executed.
    pub executed: bool,
    /// Which choice won on execution (u32::MAX = not yet executed).
    pub winning_choice: u32,
}

/// Key for a voter's vote record on a proposal (#226).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalVoteKey {
    pub proposal_id: u64,
    pub voter: Address,
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

// ── Role Management (#190) ───────────────────────────────────────────────────

/// Check if an address has a specific role.
pub fn has_role(env: &Env, address: &Address, role: &AdminRole) -> bool {
    // Backward compatibility: the main admin has all roles
    let admin = get_admin(env);
    if address == &admin {
        return true;
    }
    
    // Also, anyone with AdminRole::Admin has all roles
    if role != &AdminRole::Admin {
        let admin_key = TokenDataKey::Role(RoleKey {
            address: address.clone(),
            role: AdminRole::Admin,
        });
        if env.storage().persistent().get(&admin_key).unwrap_or(false) {
            return true;
        }
    }

    let key = TokenDataKey::Role(RoleKey {
        address: address.clone(),
        role: role.clone(),
    });
    env.storage().persistent().get(&key).unwrap_or(false)
}

/// Grant a role to an address.
pub fn grant_role(env: &Env, address: &Address, role: &AdminRole) {
    let key = TokenDataKey::Role(RoleKey {
        address: address.clone(),
        role: role.clone(),
    });
    let is_new = !env.storage().persistent().has(&key);
    env.storage().persistent().set(&key, &true);
    if is_new {
        track_entry_created(env);
    }
}

/// Revoke a role from an address.
pub fn revoke_role(env: &Env, address: &Address, role: &AdminRole) {
    let key = TokenDataKey::Role(RoleKey {
        address: address.clone(),
        role: role.clone(),
    });
    let existed = env.storage().persistent().has(&key);
    env.storage().persistent().remove(&key);
    if existed {
        track_entry_removed(env);
    }
}


// ── Emergency Pause (#189) ──────────────────────────────────────────────────

/// Check if the contract is currently paused.
pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get(&TokenDataKey::Paused)
        .unwrap_or(false)
}

/// Set the paused state.
pub fn set_paused(env: &Env, paused: bool) {
    env.storage().persistent().set(&TokenDataKey::Paused, &paused);
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
pub fn check_allowance_expired_readonly(env: &Env, owner: &Address, spender: &Address) -> (bool, bool, u32) {
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
    let is_new = !env.storage().persistent().has(&data_key);
    env.storage().persistent().set(&data_key, &true);
    env.storage().persistent().extend_ttl(
        &data_key,
        PERSISTENT_TTL_THRESHOLD,
        PERSISTENT_TTL_EXTEND_TO,
    );
    if is_new {
        track_entry_created(env);
    }
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
    let key = TokenDataKey::Whitelist(address.clone());
    let is_new = !env.storage().persistent().has(&key);
    env.storage().persistent().set(&key, &true);
    if is_new {
        track_entry_created(env);
    }
}

/// Remove an address from the whitelist.
pub fn remove_from_whitelist(env: &Env, address: &Address) {
    let key = TokenDataKey::Whitelist(address.clone());
    let existed = env.storage().persistent().has(&key);
    env.storage().persistent().remove(&key);
    if existed {
        track_entry_removed(env);
    }
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
    let is_new = !env.storage().persistent().has(&key);
    env.storage().persistent().set(&key, &balance);
    if is_new {
        track_entry_created(env);
    }
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
    let is_new = !env.storage().persistent().has(&key);
    let mut spenders: Vec<Address> = env.storage().persistent().get(&key).unwrap_or(Vec::new(env));
    if !spenders.contains(spender) {
        spenders.push_back(spender.clone());
        env.storage().persistent().set(&key, &spenders);
        if is_new {
            track_entry_created(env);
        }
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
    let is_new = !env.storage().persistent().has(&data_key);
    let current: i128 = env.storage().persistent().get(&data_key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&data_key, &(current + amount));
    env.storage().persistent().extend_ttl(
        &data_key,
        PERSISTENT_TTL_THRESHOLD,
        PERSISTENT_TTL_EXTEND_TO,
    );
    if is_new {
        track_entry_created(env);
    }
}

/// Get a learner's full reward claim history (#237).
///
/// Returns an empty vector for a learner who has never claimed.
pub fn get_claim_history(env: &Env, learner: &Address) -> Vec<ClaimRecord> {
    env.storage()
        .persistent()
        .get(&TokenDataKey::ClaimHistory(learner.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Append a claim to a learner's history (#237).
///
/// History is append-only: entries are never modified or removed, which is
/// safe because `claim_reward` rejects double-claims before reaching here.
pub fn append_claim_record(env: &Env, learner: &Address, record: &ClaimRecord) {
    let data_key = TokenDataKey::ClaimHistory(learner.clone());
    let is_new = !env.storage().persistent().has(&data_key);
    let mut history: Vec<ClaimRecord> = env
        .storage()
        .persistent()
        .get(&data_key)
        .unwrap_or_else(|| Vec::new(env));
    history.push_back(record.clone());
    env.storage().persistent().set(&data_key, &history);
    env.storage().persistent().extend_ttl(
        &data_key,
        PERSISTENT_TTL_THRESHOLD,
        PERSISTENT_TTL_EXTEND_TO,
    );
    if is_new {
        track_entry_created(env);
    }
}

// ── Vesting Schedules (#225) ──────────────────────────────────────────────────

/// Store a vesting schedule for a beneficiary.
pub fn set_vesting_schedule(env: &Env, beneficiary: &Address, schedule: &VestingSchedule) {
    let key = TokenDataKey::VestingSchedule(beneficiary.clone());
    let is_new = !env.storage().persistent().has(&key);
    env.storage().persistent().set(&key, schedule);
    if is_new {
        track_entry_created(env);
    }
}

/// Retrieve a vesting schedule for a beneficiary, if one exists.
pub fn get_vesting_schedule(env: &Env, beneficiary: &Address) -> Option<VestingSchedule> {
    env.storage()
        .persistent()
        .get(&TokenDataKey::VestingSchedule(beneficiary.clone()))
}

/// Get the total amount already claimed by a beneficiary from vesting.
pub fn get_vesting_claimed(env: &Env, beneficiary: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&TokenDataKey::VestingClaimed(beneficiary.clone()))
        .unwrap_or(0)
}

/// Record cumulative claimed amount for a beneficiary.
pub fn set_vesting_claimed(env: &Env, beneficiary: &Address, claimed: i128) {
    let key = TokenDataKey::VestingClaimed(beneficiary.clone());
    let is_new = !env.storage().persistent().has(&key);
    env.storage().persistent().set(&key, &claimed);
    if is_new {
        track_entry_created(env);
    }
}

// ── Governance Proposals (#226) ───────────────────────────────────────────────

/// Get the current proposal counter (next proposal ID = counter + 1).
pub fn get_proposal_counter(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&TokenDataKey::ProposalCounter)
        .unwrap_or(0)
}

/// Increment and return the next proposal ID.
pub fn next_proposal_id(env: &Env) -> u64 {
    let next = get_proposal_counter(env) + 1;
    env.storage()
        .persistent()
        .set(&TokenDataKey::ProposalCounter, &next);
    next
}

/// Store a governance proposal.
pub fn set_proposal(env: &Env, proposal_id: u64, proposal: &Proposal) {
    let key = TokenDataKey::Proposal(proposal_id);
    let is_new = !env.storage().persistent().has(&key);
    env.storage().persistent().set(&key, proposal);
    if is_new {
        track_entry_created(env);
    }
}

/// Retrieve a governance proposal by ID.
pub fn get_proposal(env: &Env, proposal_id: u64) -> Option<Proposal> {
    env.storage()
        .persistent()
        .get(&TokenDataKey::Proposal(proposal_id))
}

/// Record that a voter has voted on a proposal (choice index).
pub fn set_vote(env: &Env, proposal_id: u64, voter: &Address, choice: u32) {
    let key = TokenDataKey::Vote(ProposalVoteKey {
        proposal_id,
        voter: voter.clone(),
    });
    let is_new = !env.storage().persistent().has(&key);
    env.storage().persistent().set(&key, &choice);
    if is_new {
        track_entry_created(env);
    }
}

/// Whether a voter has already voted on a proposal.
pub fn has_voted(env: &Env, proposal_id: u64, voter: &Address) -> bool {
    let key = TokenDataKey::Vote(ProposalVoteKey {
        proposal_id,
        voter: voter.clone(),
    });
    env.storage().persistent().has(&key)
}

// ── Permit Nonces (#224) ──────────────────────────────────────────────────────

/// Get the current permit nonce for an owner (starts at 0).
pub fn get_permit_nonce(env: &Env, owner: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&TokenDataKey::PermitNonce(owner.clone()))
        .unwrap_or(0)
}

/// Increment the permit nonce for an owner and return the new value.
pub fn increment_permit_nonce(env: &Env, owner: &Address) -> u64 {
    let next = get_permit_nonce(env, owner) + 1;
    let key = TokenDataKey::PermitNonce(owner.clone());
    let is_new = !env.storage().persistent().has(&key);
    env.storage().persistent().set(&key, &next);
    if is_new {
        track_entry_created(env);
    }
    next
}

// ── Storage Size Tracking (#254) ─────────────────────────────────────────────
//
// Soroban has no host API to enumerate or count a contract's own storage
// keys, so the entry count is tracked by hand: every storage helper that
// creates a brand-new per-entity entry (as opposed to overwriting one that
// already exists) calls `track_entry_created` / `track_entry_removed`
// around the write, guarded by a `has()` check so repeat writes to the
// same key don't inflate the count.
//
// Singleton config values set once at `initialize()` (admin, name, symbol,
// decimals, total supply, max supply, metadata, transfer restriction,
// wasm hash, upgrade version, paused flag, proposal counter) are not
// counted -- they don't grow with usage, so they aren't what a caller is
// asking about when checking "how much storage is this contract using".
// `Balance` and `Allowance` are also excluded: balances are written on
// every transfer/mint/burn and allowances live in temporary storage, so
// instrumenting either would add a `has()` check to the contract's hottest
// paths for a self-referential accounting entry that isn't itself billed
// as persistent storage in the allowance's case.

/// Increment the storage entry counter. Call exactly once per brand-new
/// persistent entry (i.e. only after confirming the entry did not already
/// exist).
fn track_entry_created(env: &Env) {
    let next = get_storage_size(env) + 1;
    env.storage()
        .persistent()
        .set(&TokenDataKey::StorageEntryCount, &next);
}

/// Decrement the storage entry counter. Call exactly once per persistent
/// entry removed.
fn track_entry_removed(env: &Env) {
    let current = get_storage_size(env);
    let next = current.saturating_sub(1);
    env.storage()
        .persistent()
        .set(&TokenDataKey::StorageEntryCount, &next);
}

/// Number of persistent entries the contract has created, net of any that
/// have since been removed. Defaults to 0 before any tracked entry exists.
pub fn get_storage_size(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get(&TokenDataKey::StorageEntryCount)
        .unwrap_or(0)
}
