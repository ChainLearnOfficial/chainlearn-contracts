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
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllowanceKey {
    pub owner: Address,
    pub spender: Address,
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
pub fn get_allowance(env: &Env, owner: &Address, spender: &Address) -> i128 {
    let key = AllowanceKey {
        owner: owner.clone(),
        spender: spender.clone(),
    };
    env.storage()
        .persistent()
        .get(&DataKey::Allowance(key))
        .unwrap_or(0)
}

/// Set the allowance for an owner-spender pair.
pub fn set_allowance(env: &Env, owner: &Address, spender: &Address, amount: i128) {
    let key = AllowanceKey {
        owner: owner.clone(),
        spender: spender.clone(),
    };
    env.storage()
        .persistent()
        .set(&DataKey::Allowance(key), &amount);
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
