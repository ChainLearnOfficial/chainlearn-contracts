use soroban_sdk::{contracttype, Address, Env};

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
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
    env.storage().persistent().has(&DataKey::Admin)
}

/// Store the admin address.
pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().persistent().set(&DataKey::Admin, admin);
}

/// Retrieve the admin address.
pub fn get_admin(env: &Env) -> Address {
    env.storage()
        .persistent()
        .get(&DataKey::Admin)
        .expect("contract not initialized")
}

/// Get the balance for a given address.
pub fn get_balance(env: &Env, address: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::Balance(address.clone()))
        .unwrap_or(0)
}

/// Set the balance for a given address.
pub fn set_balance(env: &Env, address: &Address, amount: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::Balance(address.clone()), &amount);
}

/// Get the total supply.
pub fn get_total_supply(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::TotalSupply)
        .unwrap_or(0)
}

/// Set the total supply.
pub fn set_total_supply(env: &Env, amount: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::TotalSupply, &amount);
}

/// Get the allowance for an owner-spender pair.
/// Returns 0 if the allowance has expired or does not exist.
pub fn get_allowance(env: &Env, owner: &Address, spender: &Address) -> i128 {
    let key = AllowanceKey {
        owner: owner.clone(),
        spender: spender.clone(),
    };
    match env
        .storage()
        .persistent()
        .get::<DataKey, AllowanceData>(&DataKey::Allowance(key))
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
        .set(&DataKey::Allowance(key), &data);
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
        .get(&DataKey::Allowance(key.clone()))
        .expect("allowance not set");
    let new_amount = data.amount - spend;
    let updated = AllowanceData {
        amount: new_amount,
        expiration_ledger: data.expiration_ledger,
    };
    env.storage()
        .persistent()
        .set(&DataKey::Allowance(key), &updated);
}

/// Check if a reward has already been claimed for a given learner + quiz.
pub fn is_reward_claimed(env: &Env, learner: &Address, quiz_id: &soroban_sdk::Symbol) -> bool {
    let key = RewardKey {
        learner: learner.clone(),
        quiz_id: quiz_id.clone(),
    };
    env.storage()
        .persistent()
        .get(&DataKey::RewardClaimed(key))
        .unwrap_or(false)
}

/// Mark a reward as claimed.
pub fn set_reward_claimed(env: &Env, learner: &Address, quiz_id: &soroban_sdk::Symbol) {
    let key = RewardKey {
        learner: learner.clone(),
        quiz_id: quiz_id.clone(),
    };
    env.storage()
        .persistent()
        .set(&DataKey::RewardClaimed(key), &true);
}

/// Store the progress-tracker contract address.
pub fn set_progress_tracker(env: &Env, address: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::ProgressTracker, address);
}

/// Retrieve the progress-tracker contract address.
pub fn get_progress_tracker(env: &Env) -> Address {
    env.storage()
        .persistent()
        .get(&DataKey::ProgressTracker)
        .expect("progress tracker not set")
}
