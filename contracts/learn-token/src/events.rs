use soroban_sdk::{symbol_short, Address, Env, Symbol};

/// Emitted when a learner claims a reward for completing a quiz.
///
/// Topics: ["reward_claimed"]
/// Data: (learner, quiz_id, score, reward_amount)
pub fn reward_claimed(
    env: &Env,
    learner: &Address,
    quiz_id: &Symbol,
    score: u32,
    reward_amount: i128,
) {
    let topics = (symbol_short!("reward"),);
    env.events()
        .publish(topics, (learner, quiz_id, score, reward_amount));
}

/// Emitted when tokens are transferred.
///
/// Topics: ["transfer"]
/// Data: (from, to, amount)
pub fn transfer(env: &Env, from: &Address, to: &Address, amount: i128) {
    let topics = (symbol_short!("transfer"),);
    env.events().publish(topics, (from, to, amount));
}

/// Emitted when tokens are minted.
///
/// Topics: ["mint"]
/// Data: (to, amount)
pub fn mint(env: &Env, to: &Address, amount: i128) {
    let topics = (symbol_short!("mint"),);
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
    let topics = (symbol_short!("approve"),);
    env.events()
        .publish(topics, (owner, spender, amount, expiration_ledger));
}
