use chainlearn_shared::{PERSISTENT_TTL_EXTEND_TO, PERSISTENT_TTL_THRESHOLD};
use soroban_sdk::{contracttype, Address, Env};

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenDataKey {
    Admin,
    TokenMetadata,
    Balance(Address),
    Allowance(AllowanceKey),
    TotalSupply,
    RewardClaimed(RewardKey),
    ProgressTracker,
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
        .persistent()
        .get::<TokenDataKey, AllowanceData>(&data_key)
    {
        Some(data) => {
            if env.ledger().sequence() > data.expiration_ledger {
                env.storage().persistent().remove(&data_key);
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
        .persistent()
        .get::<TokenDataKey, AllowanceData>(&data_key)
    {
        Some(data) => {
            let is_expired = env.ledger().sequence() > data.expiration_ledger;
            if is_expired {
                env.storage().persistent().remove(&data_key);
            }
            (true, is_expired, data.expiration_ledger)
        }
        None => (false, false, 0),
    }
}

/// Set the allowance for an owner-spender pair with an expiration ledger.
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
    let data = AllowanceData {
        amount,
        expiration_ledger,
    };
    env.storage()
        .persistent()
        .set(&TokenDataKey::Allowance(key), &data);
}

/// Reduce the allowance amount while preserving the expiration ledger.
pub fn reduce_allowance(env: &Env, owner: &Address, spender: &Address, spend: i128) {
    let key = AllowanceKey {
        owner: owner.clone(),
        spender: spender.clone(),
    };
    let data: AllowanceData = env
        .storage()
        .persistent()
        .get(&TokenDataKey::Allowance(key.clone()))
        .expect("allowance not set");
    let new_amount = data.amount - spend;
    let updated = AllowanceData {
        amount: new_amount,
        expiration_ledger: data.expiration_ledger,
    };
    env.storage()
        .persistent()
        .set(&TokenDataKey::Allowance(key), &updated);
}

/// Check if a reward has already been claimed for a given learner + quiz.
pub fn is_reward_claimed(env: &Env, learner: &Address, quiz_id: &soroban_sdk::Symbol) -> bool {
    let key = RewardKey {
        learner: learner.clone(),
        quiz_id: quiz_id.clone(),
    };
    env.storage()
        .persistent()
        .get(&TokenDataKey::RewardClaimed(key))
        .unwrap_or(false)
}

/// Mark a reward as claimed.
///
/// RewardClaimed must never be allowed to lapse: it is the sole guard against
/// double-claiming a reward, so unlike `AllowanceData` it is never removed.
/// Its TTL is extended on every write so the entry keeps living well past
/// Soroban's minimum persistent-entry lifetime instead of relying on it being
/// touched again before it would otherwise be archived (#112).
pub fn set_reward_claimed(env: &Env, learner: &Address, quiz_id: &soroban_sdk::Symbol) {
    let key = RewardKey {
        learner: learner.clone(),
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
