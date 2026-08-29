#![no_std]

mod events;
mod storage;

use chainlearn_shared::{BASE_REWARD_PER_POINT, MAX_QUIZ_SCORE};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env, IntoVal,
    String as SorobanString, Symbol,
};

/// Maximum reward tokens that can be minted in a single claim (#78).
/// Caps at MAX_QUIZ_SCORE * BASE_REWARD_PER_POINT (100 * 100 = 10_000).
const MAX_REWARD_AMOUNT: i128 = (MAX_QUIZ_SCORE as i128) * BASE_REWARD_PER_POINT;

#[soroban_sdk::contractclient(name = "ProgressTrackerClient")]
pub trait ProgressTrackerInterface {
    fn get_quiz_score(env: Env, learner: Address, course_id: Symbol, quiz_id: Symbol) -> u32;
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    AlreadyInitialized = 0,
    ZeroAddress = 1,
    RewardCapped = 2,
}

/// Result of previewing a `claim_reward` call without executing it (#199).
///
/// A Soroban contract has no way to introspect its own CPU/resource-fee
/// cost — that's computed by the host during `simulateTransaction`, a
/// client/RPC-side step no contract invocation can perform on itself. What
/// this *can* do on-chain is deterministically re-run `claim_reward`'s
/// validation and reward-calculation path with zero state changes, so a
/// caller learns whether the claim would succeed and for how much before
/// spending a real transaction (and its real fee) to find out.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimEstimate {
    /// Whether calling `claim_reward` with these arguments right now would succeed.
    pub would_succeed: bool,
    /// The reward amount `claim_reward` would mint, if `would_succeed` is true. `0` otherwise.
    pub estimated_reward: i128,
    /// Human-readable reason `would_succeed` is false. Empty string if `would_succeed` is true.
    pub failure_reason: SorobanString,
}

/// SEP-41 compliant fungible token contract for ChainLearn rewards.
///
/// This token is minted as rewards when learners complete quizzes.
/// Each quiz completion mints tokens proportional to the learner's score.
#[contract]
pub struct LearnToken;

#[contractimpl]
impl LearnToken {
    // ── Initialization ────────────────────────────────────────────────────

    /// Initialize the token contract. Can only be called once.
    ///
    /// # Arguments
    /// * `admin` - Address that has minting privileges
    /// * `name` - Token name (e.g., "ChainLearn Token")
    /// * `symbol` - Token symbol (e.g., "CLRN")
    /// * `decimal` - Number of decimal places
    /// * `progress_tracker` - Address of the progress-tracker contract
    /// * `max_supply` - On-chain maximum token supply cap
    pub fn initialize(
        env: Env,
        admin: Address,
        name: SorobanString,
        symbol: SorobanString,
        decimal: u32,
        progress_tracker: Address,
        max_supply: i128,
    ) -> Result<(), ContractError> {
        if storage::is_initialized(&env) {
            return Err(ContractError::AlreadyInitialized);
        }
        if max_supply < 0 {
            panic!("max supply cannot be negative");
        }
        storage::set_admin(&env, &admin);
        storage::set_total_supply(&env, 0);
        storage::set_progress_tracker(&env, &progress_tracker);
        storage::set_max_supply(&env, max_supply);

        env.storage()
            .persistent()
            .set(&storage::TokenDataKey::Name, &name);
        env.storage()
            .persistent()
            .set(&storage::TokenDataKey::Symbol, &symbol);
        env.storage()
            .persistent()
            .set(&storage::TokenDataKey::Decimal, &decimal);
        storage::set_contract_metadata(&env);
        Ok(())
    }

    /// Get the contract's on-chain name and version (#107).
    ///
    /// Lets external tools (indexers, block explorers, upgrade tooling)
    /// identify which contract and release is deployed without inferring it
    /// from behavior.
    pub fn contract_metadata(env: Env) -> chainlearn_shared::ContractMetadata {
        storage::get_contract_metadata(&env)
    }

    // ── Transfer Restrictions (#191) ───────────────────────────────────────

    /// Validate that the transfer is permitted under the current restriction.
    ///
    /// Must be called *before* balances are updated.  For `Cooldown`, call
    /// [`Self::record_transfer_timestamp`] *after* the transfer succeeds to
    /// latch the per-sender timestamp.
    fn check_transfer_restriction(env: &Env, from: &Address, to: &Address, amount: i128) {
        let restriction = storage::get_transfer_restriction(env);
        match restriction {
            storage::TransferRestriction::None => {}
            storage::TransferRestriction::WhitelistOnly => {
                if !storage::is_whitelisted(env, to) {
                    panic!("recipient not whitelisted");
                }
            }
            storage::TransferRestriction::Cooldown(cooldown_ledgers) => {
                // Per-sender cooldown: each address has its own last-transfer
                // ledger so one user's transfer doesn't block all others.
                if let Some(last_ledger) = storage::get_last_transfer_ledger(env, from) {
                    let current = env.ledger().sequence();
                    if current < last_ledger + cooldown_ledgers {
                        panic!("cooldown period active");
                    }
                }
            }
            storage::TransferRestriction::MaxAmount(max) => {
                if amount > max {
                    panic!("transfer amount exceeds maximum");
                }
            }
        }
    }

    /// Record the current ledger as the sender's most recent transfer ledger.
    ///
    /// Only has an effect when `Cooldown` is active; a no-op otherwise so it
    /// is safe to call unconditionally after every successful transfer.
    fn record_transfer_timestamp(env: &Env, from: &Address) {
        if let storage::TransferRestriction::Cooldown(_) =
            storage::get_transfer_restriction(env)
        {
            storage::set_last_transfer_ledger(env, from, env.ledger().sequence());
        }
    }

    /// Set the transfer restriction. Admin only.
    ///
    /// # Arguments
    /// * `restriction` - The new restriction to apply
    pub fn set_transfer_restriction(env: Env, restriction: storage::TransferRestriction) {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        storage::set_transfer_restriction(&env, &restriction);
        events::restriction_updated(&env, &restriction);
    }

    /// Get the current transfer restriction.
    pub fn get_transfer_restriction(env: Env) -> storage::TransferRestriction {
        storage::get_transfer_restriction(&env)
    }

    /// Add an address to the transfer whitelist. Admin only.
    ///
    /// # Arguments
    /// * `address` - The address to whitelist
    pub fn add_to_whitelist(env: Env, address: Address) {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        storage::add_to_whitelist(&env, &address);
        events::whitelist_updated(&env, &address, true);
    }

    /// Remove an address from the transfer whitelist. Admin only.
    ///
    /// # Arguments
    /// * `address` - The address to remove from the whitelist
    pub fn remove_from_whitelist(env: Env, address: Address) {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        storage::remove_from_whitelist(&env, &address);
        events::whitelist_updated(&env, &address, false);
    }

    /// Check if an address is on the transfer whitelist.
    pub fn is_whitelisted(env: Env, address: Address) -> bool {
        storage::is_whitelisted(&env, &address)
    }

    // ── Token Snapshots (#192) ────────────────────────────────────────────

    /// Create a snapshot of all token balances at the current ledger height.
    /// Admin only. Stores the current balance for every address that has a
    /// non-zero balance.
    pub fn snapshot(env: Env, ledger_height: u32) {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        events::snapshot_created(&env, ledger_height);
    }

    /// Get the balance of an address at a specific snapshot ledger height.
    ///
    /// # Arguments
    /// * `address` - The address to query
    /// * `ledger_height` - The ledger height of the snapshot
    ///
    /// # Returns
    /// The balance at that snapshot, or 0 if no snapshot exists.
    pub fn balance_at(env: Env, address: Address, ledger_height: u32) -> i128 {
        storage::get_snapshot_balance(&env, &address, ledger_height).unwrap_or(0)
    }

    /// Record a balance snapshot for an address at the current ledger.
    /// This is called internally when a snapshot is created.
    pub fn record_balance_snapshot(env: Env, address: Address, ledger_height: u32) {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        let balance = storage::get_balance(&env, &address);
        storage::set_snapshot_balance(&env, &address, ledger_height, balance);
    }

    // ── SEP-41 Standard Interface ─────────────────────────────────────────

    /// Returns the token name.
    pub fn name(env: Env) -> SorobanString {
        env.storage()
            .persistent()
            .get(&storage::TokenDataKey::Name)
            .expect("not initialized")
    }

    /// Returns the token symbol.
    pub fn symbol(env: Env) -> SorobanString {
        env.storage()
            .persistent()
            .get(&storage::TokenDataKey::Symbol)
            .expect("not initialized")
    }

    /// Returns the number of decimals.
    pub fn decimals(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&storage::TokenDataKey::Decimal)
            .expect("not initialized")
    }

    /// Returns the total supply of tokens.
    pub fn total_supply(env: Env) -> i128 {
        storage::get_total_supply(&env)
    }

    /// Returns the balance of the given address.
    pub fn balance(env: Env, address: Address) -> i128 {
        storage::get_balance(&env, &address)
    }

    /// Transfer tokens from the caller to another address.
    ///
    /// # Arguments
    /// * `from` - Source address (must authorize)
    /// * `to` - Destination address
    /// * `amount` - Amount to transfer
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();

        Self::require_not_paused(&env);

        if from == to {
            return;
        }

        // Prevent transfers to the contract itself, which would lock tokens
        // irretrievably (#76).
        if to == env.current_contract_address() {
            panic!("cannot transfer to contract");
        }

        if amount < 0 {
            panic!("negative amount");
        }

        let from_balance = storage::get_balance(&env, &from);
        if from_balance < amount {
            panic!("insufficient balance");
        }

        // Check transfer restrictions (#191)
        Self::check_transfer_restriction(&env, &from, &to, amount);

        storage::set_balance(&env, &from, from_balance - amount);
        let to_balance = storage::get_balance(&env, &to);
        storage::set_balance(&env, &to, to_balance + amount);

        // Latch the per-sender cooldown timestamp after a successful transfer.
        Self::record_transfer_timestamp(&env, &from);

        events::transfer(&env, &from, &to, amount);
    }

    /// Transfer tokens on behalf of another address.
    ///
    /// # Arguments
    /// * `spender` - The address authorizing the transfer (must authorize)
    /// * `from` - Source address
    /// * `to` - Destination address
    /// * `amount` - Amount to transfer
    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();

        Self::require_not_paused(&env);

        if from == to {
            return;
        }

        // Prevent transfers to the contract itself, which would lock tokens
        // irretrievably (#76).
        if to == env.current_contract_address() {
            panic!("cannot transfer to contract");
        }

        if amount < 0 {
            panic!("negative amount");
        }

        let (exists, is_expired, expiration_ledger) =
            storage::check_allowance_expired(&env, &from, &spender);
        if exists && is_expired {
            events::allowance_expired(&env, &from, &spender, expiration_ledger);
        }

        let allowance = storage::get_allowance(&env, &from, &spender);
        if allowance < amount {
            panic!("insufficient allowance");
        }

        let from_balance = storage::get_balance(&env, &from);
        if from_balance < amount {
            panic!("insufficient balance");
        }

        // Check transfer restrictions (#191)
        Self::check_transfer_restriction(&env, &from, &to, amount);

        storage::reduce_allowance(&env, &from, &spender, amount);
        storage::set_balance(&env, &from, from_balance - amount);
        let to_balance = storage::get_balance(&env, &to);
        storage::set_balance(&env, &to, to_balance + amount);

        // Latch the per-sender cooldown timestamp after a successful transfer.
        Self::record_transfer_timestamp(&env, &from);

        events::transfer_from(&env, &spender, &from, &to, amount);
    }

    /// Approve a spender to spend tokens on behalf of the caller.
    ///
    /// # Arguments
    /// * `owner` - Token owner (must authorize)
    /// * `spender` - Address being approved
    /// * `amount` - Allowance amount
    /// * `expiration_ledger` - Ledger number when the allowance expires
    pub fn approve(
        env: Env,
        owner: Address,
        spender: Address,
        amount: i128,
        expiration_ledger: u32,
    ) {
        owner.require_auth();

        if amount < 0 {
            panic!("negative amount");
        }

        if expiration_ledger <= env.ledger().sequence() {
            panic!("expiration_ledger must be in the future");
        }

        storage::set_allowance(&env, &owner, &spender, amount, expiration_ledger);
        storage::track_allowance_spender(&env, &owner, &spender);
        events::approve(&env, &owner, &spender, amount, expiration_ledger);
    }

    /// Returns the allowance for a spender on behalf of an owner.
    ///
    /// View function: does not emit events (#179). allowance_expired events
    /// are emitted only from state-mutating functions (transfer_from,
    /// burn_from, prune_expired_allowance) where expiration is acted upon.
    pub fn allowance(env: Env, owner: Address, spender: Address) -> i128 {
        storage::get_allowance_readonly(&env, &owner, &spender)
    }

    // ── SEP-41 Burning ────────────────────────────────────────────────────

    /// Burn tokens held by their owner, permanently reducing the total supply.
    ///
    /// Required by SEP-41. Burning is unrestricted: any holder may destroy
    /// their own tokens, with no admin involvement.
    ///
    /// # Arguments
    /// * `from` - Token owner whose balance is reduced (must authorize)
    /// * `amount` - Amount to burn
    ///
    /// # Panics
    /// * If `amount` is negative
    /// * If `from` holds less than `amount`
    pub fn burn(env: Env, from: Address, amount: i128) {
        from.require_auth();

        Self::require_not_paused(&env);

        if amount < 0 {
            panic!("negative amount");
        }

        let from_balance = storage::get_balance(&env, &from);
        if from_balance < amount {
            panic!("insufficient balance");
        }

        storage::set_balance(&env, &from, from_balance - amount);

        // Total supply tracks circulating tokens, so burning reduces it —
        // otherwise the supply would overstate what actually exists.
        let current_supply = storage::get_total_supply(&env);
        storage::set_total_supply(&env, current_supply - amount);

        events::burn(&env, &from, amount);
    }

    /// Burn tokens on behalf of an owner, drawing on an approved allowance.
    ///
    /// Required by SEP-41. The spender authorizes; the owner's balance and the
    /// spender's allowance are both reduced, mirroring `transfer_from`.
    ///
    /// # Arguments
    /// * `spender` - Address spending the allowance (must authorize)
    /// * `from` - Token owner whose balance is reduced
    /// * `amount` - Amount to burn
    ///
    /// # Panics
    /// * If `amount` is negative
    /// * If the spender's allowance is below `amount`
    /// * If `from` holds less than `amount`
    pub fn burn_from(env: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();

        Self::require_not_paused(&env);

        if amount < 0 {
            panic!("negative amount");
        }

        // Surface an expired allowance the same way transfer_from does, so
        // indexers see one consistent signal regardless of which path spent it.
        let (exists, is_expired, expiration_ledger) =
            storage::check_allowance_expired(&env, &from, &spender);
        if exists && is_expired {
            events::allowance_expired(&env, &from, &spender, expiration_ledger);
        }

        let allowance = storage::get_allowance(&env, &from, &spender);
        if allowance < amount {
            panic!("insufficient allowance");
        }

        let from_balance = storage::get_balance(&env, &from);
        if from_balance < amount {
            panic!("insufficient balance");
        }

        storage::reduce_allowance(&env, &from, &spender, amount);
        storage::set_balance(&env, &from, from_balance - amount);

        let current_supply = storage::get_total_supply(&env);
        storage::set_total_supply(&env, current_supply - amount);

        events::burn_from(&env, &spender, &from, amount);
    }

    // ── Minting (Admin Only) ──────────────────────────────────────────────

    /// Mint new tokens to an address. Admin only.
    ///
    /// # Arguments
    /// * `to` - Recipient address
    /// * `amount` - Amount to mint
    pub fn mint(env: Env, caller: Address, to: Address, amount: i128) {
        caller.require_auth();

        Self::require_not_paused(&env);
        if !storage::has_role(&env, &caller, &storage::AdminRole::Minter) {
            panic!("not authorized");
        }



        let zero_address = Address::from_string(&SorobanString::from_str(
            &env,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        ));
        if to == zero_address {
            panic!("cannot mint to zero address");
        }

        if amount < 0 {
            panic!("negative amount");
        }

        let current_supply = storage::get_total_supply(&env);
        let max_supply = storage::get_max_supply(&env);
        if current_supply + amount > max_supply {
            panic!("maximum supply cap exceeded");
        }

        let current_balance = storage::get_balance(&env, &to);
        storage::set_balance(&env, &to, current_balance + amount);

        storage::set_total_supply(&env, current_supply + amount);

        events::mint(&env, &to, amount);
    }

    // ── ChainLearn Reward Logic ───────────────────────────────────────────

    /// Claim a token reward for completing a quiz.
    ///
    /// The reward amount is calculated as: `verified_score * BASE_REWARD_PER_POINT`.
    /// The score is verified by querying the progress-tracker contract.
    /// Each learner can only claim a reward once per quiz.
    ///
    /// # Arguments
    /// * `learner` - The learner claiming the reward (must authorize)
    /// * `course_id` - The course the quiz belongs to
    /// * `quiz_id` - Unique identifier for the quiz
    pub fn claim_reward(env: Env, learner: Address, course_id: Symbol, quiz_id: Symbol) {
        learner.require_auth();

        Self::require_not_paused(&env);

        if storage::is_reward_claimed(&env, &learner, &course_id, &quiz_id) {
            panic!("reward already claimed");
        }

        // Verify score by querying the progress-tracker contract.
        // Use env.invoke_contract directly to avoid the gas cost of
        // ProgressTrackerClient::new() on every invocation (#133).
        let progress_tracker = storage::get_progress_tracker(&env);
        let score: u32 = env.invoke_contract(
            &progress_tracker,
            &Symbol::new(&env, "get_quiz_score"),
            (&learner, &course_id, &quiz_id).into_val(&env),
        );

        if score == 0 {
            panic!("score must be greater than 0");
        }

        if score > MAX_QUIZ_SCORE {
            panic!("score exceeds maximum");
        }

        let reward_amount = (score as i128) * BASE_REWARD_PER_POINT;

        // Cap the maximum reward to prevent excessively large minting if
        // MAX_QUIZ_SCORE or BASE_REWARD_PER_POINT change in the future (#78).
        if reward_amount > MAX_REWARD_AMOUNT {
            panic!("reward exceeds cap");
        }

        // Check max_supply cap before minting (#178)
        let current_supply = storage::get_total_supply(&env);
        let max_supply = storage::get_max_supply(&env);
        if current_supply + reward_amount > max_supply {
            panic!("maximum supply cap exceeded");
        }

        // Mint tokens to the learner
        let current_balance = storage::get_balance(&env, &learner);
        storage::set_balance(&env, &learner, current_balance + reward_amount);

        storage::set_total_supply(&env, current_supply + reward_amount);

        // Mark reward as claimed to prevent double-claiming
        storage::set_reward_claimed(&env, &learner, &course_id, &quiz_id);

        events::reward_claimed(&env, &learner, &quiz_id, score, reward_amount, &course_id);
    }

    /// Claim token rewards for completing multiple quizzes in a batch.
    ///
    /// Iterates through `quiz_ids`, claiming rewards for each. Each quiz is processed
    /// independently. Partial failures (e.g. already claimed, score 0) do not block
    /// successful claims in the batch.
    ///
    /// # Arguments
    /// * `learner` - The learner claiming the rewards (must authorize)
    /// * `course_id` - The course the quizzes belong to
    /// * `quiz_ids` - Unique identifiers for the quizzes
    ///
    /// # Returns
    /// * `Vec<Symbol>` containing the IDs of successfully claimed quizzes.
    pub fn batch_claim_reward(
        env: Env,
        learner: Address,
        course_id: Symbol,
        quiz_ids: soroban_sdk::Vec<Symbol>,
    ) -> soroban_sdk::Vec<Symbol> {
        learner.require_auth();

        let mut successful = soroban_sdk::Vec::new(&env);
        let progress_tracker = storage::get_progress_tracker(&env);
        let max_supply = storage::get_max_supply(&env);

        let mut current_supply = storage::get_total_supply(&env);
        let mut current_balance = storage::get_balance(&env, &learner);

        for quiz_id in quiz_ids.iter() {
            if storage::is_reward_claimed(&env, &learner, &course_id, &quiz_id) {
                continue;
            }

            let score: u32 = env.invoke_contract(
                &progress_tracker,
                &Symbol::new(&env, "get_quiz_score"),
                (&learner, &course_id, &quiz_id).into_val(&env),
            );

            if score == 0 || score > MAX_QUIZ_SCORE {
                continue;
            }

            let reward_amount = (score as i128) * BASE_REWARD_PER_POINT;
            if reward_amount > MAX_REWARD_AMOUNT {
                continue;
            }

            if current_supply + reward_amount > max_supply {
                continue;
            }

            current_supply += reward_amount;
            current_balance += reward_amount;

            storage::set_reward_claimed(&env, &learner, &course_id, &quiz_id);
            events::reward_claimed(&env, &learner, &quiz_id, score, reward_amount, &course_id);
            successful.push_back(quiz_id);
        }

        if successful.len() > 0 {
            storage::set_balance(&env, &learner, current_balance);
            storage::set_total_supply(&env, current_supply);
        }

        successful
    }

    /// Preview a `claim_reward` call without executing it or changing any
    /// state (#199).
    ///
    /// See [`ClaimEstimate`] for why this reports the reward amount rather
    /// than a raw gas/CPU figure — that number isn't something a Soroban
    /// contract can compute about its own execution. Re-runs exactly the
    /// same checks `claim_reward` does (already-claimed, quiz score via the
    /// progress-tracker, score bounds, reward cap, supply cap) so a caller
    /// can tell whether the real call would succeed, and for what amount,
    /// before spending a transaction to find out. Read-only: it never
    /// calls `require_auth`, never touches storage other than reads, and
    /// never invokes anything beyond the progress-tracker's read-only
    /// `get_quiz_score`.
    ///
    /// # Arguments
    /// * `learner` - The learner who would claim the reward
    /// * `course_id` - The course the quiz belongs to
    /// * `quiz_id` - Unique identifier for the quiz
    pub fn estimate_claim_gas(
        env: Env,
        learner: Address,
        course_id: Symbol,
        quiz_id: Symbol,
    ) -> ClaimEstimate {
        let fail = |reason: &str| ClaimEstimate {
            would_succeed: false,
            estimated_reward: 0,
            failure_reason: SorobanString::from_str(&env, reason),
        };

        if storage::is_reward_claimed(&env, &learner, &course_id, &quiz_id) {
            return fail("reward already claimed");
        }

        let progress_tracker = storage::get_progress_tracker(&env);
        let score: u32 = env.invoke_contract(
            &progress_tracker,
            &Symbol::new(&env, "get_quiz_score"),
            (&learner, &course_id, &quiz_id).into_val(&env),
        );

        if score == 0 {
            return fail("score must be greater than 0");
        }
        if score > MAX_QUIZ_SCORE {
            return fail("score exceeds maximum");
        }

        let reward_amount = (score as i128) * BASE_REWARD_PER_POINT;
        if reward_amount > MAX_REWARD_AMOUNT {
            return fail("reward exceeds cap");
        }

        let current_supply = storage::get_total_supply(&env);
        let max_supply = storage::get_max_supply(&env);
        if current_supply + reward_amount > max_supply {
            return fail("maximum supply cap exceeded");
        }

        ClaimEstimate {
            would_succeed: true,
            estimated_reward: reward_amount,
            failure_reason: SorobanString::from_str(&env, ""),
        }
    }


    // ── Emergency Pause (#189) ────────────────────────────────────────────

    fn require_not_paused(env: &Env) {
        if storage::is_paused(env) {
            panic!("contract is paused");
        }
    }

    /// Pause all state-changing operations. Admin or Pauser only.
    pub fn emergency_pause(env: Env, caller: Address) {
        caller.require_auth();
        if !storage::has_role(&env, &caller, &storage::AdminRole::Pauser) {
            panic!("not authorized");
        }
        storage::set_paused(&env, true);
        events::paused(&env, &caller);
    }

    /// Unpause state-changing operations. Admin or Pauser only.
    pub fn unpause(env: Env, caller: Address) {
        caller.require_auth();
        if !storage::has_role(&env, &caller, &storage::AdminRole::Pauser) {
            panic!("not authorized");
        }
        storage::set_paused(&env, false);
        events::unpaused(&env, &caller);
    }

    // ── Admin ─────────────────────────────────────────────────────────────


    /// Grant an admin role to an address. Admin only.
    pub fn grant_role(env: Env, caller: Address, address: Address, role: storage::AdminRole) {
        caller.require_auth();
        if !storage::has_role(&env, &caller, &storage::AdminRole::Admin) {
            panic!("not authorized");
        }
        storage::grant_role(&env, &address, &role);
        events::role_granted(&env, &address, &role);
    }

    /// Revoke an admin role from an address. Admin only.
    pub fn revoke_role(env: Env, caller: Address, address: Address, role: storage::AdminRole) {
        caller.require_auth();
        if !storage::has_role(&env, &caller, &storage::AdminRole::Admin) {
            panic!("not authorized");
        }
        storage::revoke_role(&env, &address, &role);
        events::role_revoked(&env, &address, &role);
    }

    /// Check if an address has a specific role.
    pub fn has_role(env: Env, address: Address, role: storage::AdminRole) -> bool {
        storage::has_role(&env, &address, &role)
    }

    /// Returns the main admin address.
    pub fn admin(env: Env) -> Address {
        storage::get_admin(&env)
    }

    /// Upgrade the contract's wasm code. Admin only (#198).
    ///
    /// State is preserved across the upgrade by construction: Soroban
    /// upgrades replace only the executable code at this contract's
    /// address, not its storage, so every balance, allowance, and other
    /// persistent/temporary entry survives untouched. The new wasm is
    /// expected to have already been uploaded to the network (e.g. via
    /// `soroban contract install`) before this is called with its hash.
    ///
    /// # Arguments
    /// * `new_wasm_hash` - Hash of the already-uploaded wasm to install
    ///
    /// # Panics
    /// * If the caller is not the admin
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        let admin = storage::get_admin(&env);
        admin.require_auth();

        env.deployer()
            .update_current_contract_wasm(new_wasm_hash.clone());
        storage::set_wasm_hash(&env, &new_wasm_hash);
        let version = storage::increment_upgrade_version(&env);

        events::upgraded(&env, &new_wasm_hash, version);
    }

    /// Wasm hash the contract was most recently upgraded to, or `None` if
    /// it has never been upgraded (#198).
    pub fn wasm_hash(env: Env) -> Option<BytesN<32>> {
        storage::get_wasm_hash(&env)
    }

    /// Number of times the contract has been upgraded via `upgrade()` (#198).
    /// Starts at `0` for a never-upgraded contract.
    pub fn upgrade_version(env: Env) -> u32 {
        storage::get_upgrade_version(&env)
    }

    /// Returns the progress-tracker address rewards are verified against.
    ///
    /// Read-only. Deployment scripts use this to confirm the wiring actually
    /// landed, instead of discovering an unset tracker when the first
    /// `claim_reward` panics (#31).
    pub fn progress_tracker(env: Env) -> Address {
        storage::get_progress_tracker(&env)
    }

    /// Returns the maximum supply cap.
    pub fn max_supply(env: Env) -> i128 {
        storage::get_max_supply(&env)
    }

    /// Update the maximum supply cap. Admin only.
    pub fn set_max_supply(env: Env, new_max_supply: i128) {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        if new_max_supply < 0 {
            panic!("max supply cannot be negative");
        }
        let current_supply = storage::get_total_supply(&env);
        if new_max_supply < current_supply {
            panic!("new cap cannot be less than current total supply");
        }
        storage::set_max_supply(&env, new_max_supply);
    }

    /// Transfer admin rights to a new address.
    ///
    /// # Arguments
    /// * `new_admin` - The new admin address
    pub fn transfer_admin(env: Env, new_admin: Address) {
        let admin = storage::get_admin(&env);
        admin.require_auth();

        let zero_address = Address::from_string(&SorobanString::from_str(
            &env,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        ));
        if new_admin == zero_address {
            panic!("cannot transfer admin to zero address");
        }

        storage::set_admin(&env, &new_admin);
    }

    /// Update the progress-tracker contract address. Admin only.
    ///
    /// Required when the progress-tracker contract is upgraded or redeployed.
    /// Without this, the learn-token becomes permanently broken after a
    /// progress-tracker upgrade (#75).
    ///
    /// # Arguments
    /// * `new_progress_tracker` - The new progress-tracker contract address
    pub fn set_progress_tracker(env: Env, new_progress_tracker: Address) {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        storage::set_progress_tracker(&env, &new_progress_tracker);
        events::progress_tracker_updated(&env, &new_progress_tracker);
    }

    /// Increase the allowance for a spender (#77).
    ///
    /// Unlike `approve()`, this adds to the existing allowance rather than
    /// overwriting it, preventing the front-running vulnerability where a
    /// spender spends the old allowance before the new one takes effect.
    ///
    /// # Arguments
    /// * `owner` - Token owner (must authorize)
    /// * `spender` - Address whose allowance to increase
    /// * `additional_amount` - Amount to add to the current allowance
    /// * `expiration_ledger` - New expiration ledger (replaces old)
    pub fn increase_allowance(
        env: Env,
        owner: Address,
        spender: Address,
        additional_amount: i128,
        expiration_ledger: u32,
    ) {
        owner.require_auth();

        if additional_amount < 0 {
            panic!("negative amount");
        }

        if expiration_ledger <= env.ledger().sequence() {
            panic!("expiration_ledger must be in the future");
        }

        let current = storage::get_allowance(&env, &owner, &spender);
        let new_amount = current + additional_amount;
        storage::set_allowance(&env, &owner, &spender, new_amount, expiration_ledger);
        storage::track_allowance_spender(&env, &owner, &spender);
        events::approve(&env, &owner, &spender, new_amount, expiration_ledger);
    }

    /// Remove an expired allowance from persistent storage (#111).
    ///
    /// Until now, an expired `AllowanceData` entry was only pruned as a side
    /// effect of someone calling `allowance()` or `transfer_from()` for that
    /// exact owner/spender pair -- if nobody ever touched it again, it stayed
    /// in persistent storage indefinitely. This gives anyone (no auth
    /// required, since it can only remove data that is already expired and
    /// therefore already worthless) an explicit way to prune a known expired
    /// allowance, e.g. from an indexer that watched the `approve` event and
    /// noticed its `expiration_ledger` has passed.
    ///
    /// # Arguments
    /// * `owner` - Token owner
    /// * `spender` - Approved spender
    ///
    /// # Returns
    /// `true` if an expired allowance was found and removed, `false` if the
    /// allowance does not exist or has not yet expired.
    pub fn prune_expired_allowance(env: Env, owner: Address, spender: Address) -> bool {
        let (exists, is_expired, expiration_ledger) =
            storage::check_allowance_expired(&env, &owner, &spender);
        if exists && is_expired {
            events::allowance_expired(&env, &owner, &spender, expiration_ledger);
        }
        exists && is_expired
    }

    /// Remove every expired allowance for `owner` in one call (#201).
    ///
    /// Permissionless (like `prune_expired_allowance`, no auth is required
    /// since this only removes data that is already expired and therefore
    /// already worthless), and walks the registry of spenders `owner` has
    /// ever approved (tracked by `approve`/`increase_allowance`) rather than
    /// requiring the caller to name each spender — Soroban storage has no
    /// key-enumeration API, so that registry is the only way this can be
    /// "all of them" instead of one at a time.
    ///
    /// # Arguments
    /// * `owner` - Token owner whose expired allowances should be swept
    ///
    /// # Returns
    /// The number of expired allowances that were removed.
    pub fn cleanup_expired_allowances(env: Env, owner: Address) -> u32 {
        let spenders = storage::get_allowance_spenders(&env, &owner);
        let mut remaining = soroban_sdk::Vec::new(&env);
        let mut removed_count: u32 = 0;

        for spender in spenders.iter() {
            let (exists, is_expired, expiration_ledger) =
                storage::check_allowance_expired(&env, &owner, &spender);
            if exists && is_expired {
                events::allowance_expired(&env, &owner, &spender, expiration_ledger);
                removed_count += 1;
            } else if exists {
                // Still active — stays in the registry for a future sweep.
                remaining.push_back(spender.clone());
            }
            // If it doesn't exist at all (fully spent/never set), it's
            // already gone from storage; drop it from the registry too.
        }

        storage::set_allowance_spenders(&env, &owner, &remaining);
        removed_count
    }

    /// Number of spenders currently tracked in `owner`'s allowance registry
    /// (#201) — an upper bound on how many *active* allowance entries `owner`
    /// has in persistent/temporary storage (some tracked entries may already
    /// be expired but not yet swept by `cleanup_expired_allowances`).
    ///
    /// Intended as a lightweight signal for whether it's worth calling
    /// `cleanup_expired_allowances` for a given owner.
    pub fn allowance_spender_count(env: Env, owner: Address) -> u32 {
        storage::get_allowance_spenders(&env, &owner).len()
    }

    /// Decrease the allowance for a spender (#77).
    ///
    /// Allows a granular reduction of the allowance without resetting it.
    ///
    /// # Arguments
    /// * `owner` - Token owner (must authorize)
    /// * `spender` - Address whose allowance to decrease
    /// * `decrease_amount` - Amount to subtract from the current allowance
    pub fn decrease_allowance(env: Env, owner: Address, spender: Address, decrease_amount: i128) {
        owner.require_auth();

        if decrease_amount < 0 {
            panic!("negative amount");
        }

        let current = storage::get_allowance(&env, &owner, &spender);
        if decrease_amount > current {
            panic!("decrease exceeds allowance");
        }
        let new_amount = current - decrease_amount;
        // Preserve existing expiration
        let key = storage::AllowanceKey {
            owner: owner.clone(),
            spender: spender.clone(),
        };
        let data: storage::AllowanceData = env
            .storage()
            .temporary()
            .get(&storage::TokenDataKey::Allowance(key.clone()))
            .expect("allowance not set");
        storage::set_allowance(&env, &owner, &spender, new_amount, data.expiration_ledger);
        events::approve(&env, &owner, &spender, new_amount, data.expiration_ledger);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{storage::Persistent as _, Address as _, Ledger as _},
        Address, Env, IntoVal, String as SorobanString, Vec,
    };

    fn setup(env: &Env) -> (Address, Address, Address) {
        let admin = Address::generate(env);

        // Register progress-tracker
        let pt_contract_id = env.register_contract(None, progress_tracker::ProgressTracker);
        let pt_client = progress_tracker::ProgressTrackerClient::new(env, &pt_contract_id);
        pt_client.initialize(&admin);

        // Register learn-token with progress-tracker address
        let lt_contract_id = env.register_contract(None, LearnToken);
        let lt_client = LearnTokenClient::new(env, &lt_contract_id);
        lt_client.initialize(
            &admin,
            &SorobanString::from_str(env, "CLearn"),
            &SorobanString::from_str(env, "CLRN"),
            &7,
            &pt_contract_id,
            &1_000_000_000_000_000,
        );

        (admin, lt_contract_id, pt_contract_id)
    }

    fn create_course_and_submit_quiz(
        env: &Env,
        pt_client: &progress_tracker::ProgressTrackerClient,
        learner: &Address,
        course_id: &Symbol,
        quiz_id: &Symbol,
        score: u32,
    ) {
        let mut module_ids = Vec::new(env);
        module_ids.push_back(Symbol::new(env, "mod_1"));
        let mut quiz_ids = Vec::new(env);
        quiz_ids.push_back(quiz_id.clone());
        pt_client.create_course(course_id, &1, &1, &module_ids, &quiz_ids);
        pt_client.enroll(learner, course_id);
        pt_client.submit_quiz_score(learner, course_id, quiz_id, &score);
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        let (admin, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        assert_eq!(client.name(), SorobanString::from_str(&env, "CLearn"));
        assert_eq!(client.symbol(), SorobanString::from_str(&env, "CLRN"));
        assert_eq!(client.decimals(), 7);
        assert_eq!(client.total_supply(), 0);
        assert_eq!(client.admin(), admin);
    }

    // ── Issue #107: initialize() stores contract name/version metadata ──────

    #[test]
    fn test_initialize_stores_contract_metadata() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let metadata = client.contract_metadata();
        assert_eq!(metadata.name, SorobanString::from_str(&env, "learn-token"));
        assert_eq!(
            metadata.version,
            SorobanString::from_str(&env, chainlearn_shared::CONTRACT_VERSION)
        );
    }

    #[test]
    fn test_mint() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &learner, &1000);

        assert_eq!(client.balance(&learner), 1000);
        assert_eq!(client.total_supply(), 1000);
    }

    #[test]
    fn test_transfer() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &alice, &500);
        client.transfer(&alice, &bob, &200);

        assert_eq!(client.balance(&alice), 300);
        assert_eq!(client.balance(&bob), 200);
    }

    #[test]
    fn test_claim_reward() {
        let env = Env::default();
        let (_, lt_contract_id, pt_contract_id) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);
        let pt_client = progress_tracker::ProgressTrackerClient::new(&env, &pt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "math_101");
        let quiz_id = Symbol::new(&env, "quiz_math_101");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 85);

        client.claim_reward(&learner, &course_id, &quiz_id);

        // 85 * 100 (BASE_REWARD_PER_POINT) = 8500
        assert_eq!(client.balance(&learner), 8500);
    }

    #[test]
    #[should_panic(expected = "reward already claimed")]
    fn test_claim_reward_prevents_double_claim() {
        let env = Env::default();
        let (_, lt_contract_id, pt_contract_id) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);
        let pt_client = progress_tracker::ProgressTrackerClient::new(&env, &pt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "math_101");
        let quiz_id = Symbol::new(&env, "quiz_math_101");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 85);

        client.claim_reward(&learner, &course_id, &quiz_id);
        client.claim_reward(&learner, &course_id, &quiz_id); // should panic
    }

    #[test]
    #[should_panic(expected = "quiz not submitted")]
    fn test_claim_reward_rejects_unverified_score() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        // Try to claim without submitting a quiz — should panic
        let course_id = Symbol::new(&env, "math_101");
        let quiz_id = Symbol::new(&env, "quiz_math_101");
        client.claim_reward(&learner, &course_id, &quiz_id);
    }

    // ── #31: progress-tracker wiring is readable after initialize ───────────

    #[test]
    fn test_progress_tracker_returns_configured_address() {
        let env = Env::default();
        let (_, lt_contract_id, pt_contract_id) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        assert_eq!(client.progress_tracker(), pt_contract_id);
    }

    #[test]
    fn test_progress_tracker_reflects_updates() {
        let env = Env::default();
        let (admin, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);
        env.mock_all_auths();

        let new_pt_id = env.register_contract(None, progress_tracker::ProgressTracker);
        progress_tracker::ProgressTrackerClient::new(&env, &new_pt_id).initialize(&admin);

        client.set_progress_tracker(&new_pt_id);
        assert_eq!(client.progress_tracker(), new_pt_id);
    }

    // ── #75: set_progress_tracker ────────────────────────────────────────────

    #[test]
    fn test_set_progress_tracker_updates_address() {
        let env = Env::default();
        let (admin, lt_contract_id, _old_pt_id) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        env.mock_all_auths();

        // Create a new progress-tracker (simulating an upgrade)
        let new_pt_id = env.register_contract(None, progress_tracker::ProgressTracker);
        let new_pt_client = progress_tracker::ProgressTrackerClient::new(&env, &new_pt_id);
        new_pt_client.initialize(&admin);

        // Update the learn-token to point to the new progress-tracker
        client.set_progress_tracker(&new_pt_id);

        // Verify rewards now query the new progress-tracker
        let learner = Address::generate(&env);
        let course_id = Symbol::new(&env, "course_new");
        let quiz_id = Symbol::new(&env, "quiz_new");

        // Submit quiz on the NEW progress-tracker
        let mut module_ids = Vec::new(&env);
        module_ids.push_back(Symbol::new(&env, "mod_1"));
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(quiz_id.clone());
        new_pt_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
        new_pt_client.enroll(&learner, &course_id);
        new_pt_client.submit_quiz_score(&learner, &course_id, &quiz_id, &90);

        // Claim reward — should succeed using the new progress-tracker
        client.claim_reward(&learner, &course_id, &quiz_id);
        assert_eq!(client.balance(&learner), 9000); // 90 * 100
    }

    #[test]
    #[should_panic]
    fn test_set_progress_tracker_requires_admin() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let fake_pt = Address::generate(&env);

        // Only authorize a stranger — admin auth is missing, must panic
        env.mock_auths(&[]);
        client.set_progress_tracker(&fake_pt);
    }

    // ── #76: transfer to contract address ───────────────────────────────────

    #[test]
    #[should_panic(expected = "cannot transfer to contract")]
    fn test_transfer_to_contract_address_panics() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let alice = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &alice, &500);
        // Attempt to transfer to the contract itself — must panic
        client.transfer(&alice, &lt_contract_id, &200);
    }

    #[test]
    #[should_panic(expected = "cannot transfer to contract")]
    fn test_transfer_from_to_contract_address_panics() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &owner, &1000);
        client.approve(&owner, &spender, &500, &999999);

        // Attempt transfer_from to the contract itself — must panic
        client.transfer_from(&spender, &owner, &lt_contract_id, &200);
    }

    // ── #77: increase_allowance / decrease_allowance ─────────────────────────

    #[test]
    fn test_increase_allowance_adds_to_existing() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        client.approve(&owner, &spender, &100, &999999);
        assert_eq!(client.allowance(&owner, &spender), 100);

        client.increase_allowance(&owner, &spender, &50, &999999);
        assert_eq!(client.allowance(&owner, &spender), 150);
    }

    #[test]
    fn test_decrease_allowance_subtracts_from_existing() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        client.approve(&owner, &spender, &200, &999999);
        client.decrease_allowance(&owner, &spender, &80);
        assert_eq!(client.allowance(&owner, &spender), 120);
    }

    #[test]
    #[should_panic(expected = "decrease exceeds allowance")]
    fn test_decrease_allowance_below_zero_panics() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        client.approve(&owner, &spender, &50, &999999);
        client.decrease_allowance(&owner, &spender, &100); // exceeds 50
    }

    #[test]
    fn test_increase_then_decrease_allowance_roundtrip() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        client.approve(&owner, &spender, &100, &999999);
        client.increase_allowance(&owner, &spender, &200, &999999);
        assert_eq!(client.allowance(&owner, &spender), 300);

        client.decrease_allowance(&owner, &spender, &150);
        assert_eq!(client.allowance(&owner, &spender), 150);
    }

    // ── #33: SEP-41 burn / burn_from ────────────────────────────────────────

    #[test]
    fn test_burn_reduces_balance_and_supply() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let alice = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &alice, &1000);
        client.burn(&alice, &400);

        assert_eq!(client.balance(&alice), 600);
        assert_eq!(client.total_supply(), 600);
    }

    #[test]
    fn test_burn_entire_balance() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let alice = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &alice, &500);
        client.burn(&alice, &500);

        assert_eq!(client.balance(&alice), 0);
        assert_eq!(client.total_supply(), 0);
    }

    #[test]
    fn test_burn_zero_is_a_noop() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let alice = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &alice, &100);
        client.burn(&alice, &0);

        assert_eq!(client.balance(&alice), 100);
        assert_eq!(client.total_supply(), 100);
    }

    #[test]
    #[should_panic(expected = "insufficient balance")]
    fn test_burn_more_than_balance_panics() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let alice = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &alice, &100);
        client.burn(&alice, &101);
    }

    #[test]
    #[should_panic(expected = "negative amount")]
    fn test_burn_negative_amount_panics() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let alice = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &alice, &100);
        client.burn(&alice, &-1);
    }

    #[test]
    #[should_panic]
    fn test_burn_requires_owner_auth() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let alice = Address::generate(&env);
        env.mock_all_auths();
        client.mint(&admin, &alice, &100);

        // Nobody authorizes the burn — the owner's auth is required.
        env.mock_auths(&[]);
        client.burn(&alice, &50);
    }

    #[test]
    fn test_burn_from_spends_allowance() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &owner, &1000);
        client.approve(&owner, &spender, &300, &999999);

        client.burn_from(&spender, &owner, &200);

        assert_eq!(client.balance(&owner), 800);
        assert_eq!(client.total_supply(), 800);
        assert_eq!(client.allowance(&owner, &spender), 100);
    }

    #[test]
    #[should_panic(expected = "insufficient allowance")]
    fn test_burn_from_beyond_allowance_panics() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &owner, &1000);
        client.approve(&owner, &spender, &100, &999999);

        client.burn_from(&spender, &owner, &101);
    }

    #[test]
    #[should_panic(expected = "insufficient balance")]
    fn test_burn_from_beyond_balance_panics() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &owner, &50);
        // Allowance exceeds what the owner actually holds.
        client.approve(&owner, &spender, &500, &999999);

        client.burn_from(&spender, &owner, &100);
    }

    #[test]
    #[should_panic(expected = "insufficient allowance")]
    fn test_burn_from_without_allowance_panics() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &owner, &1000);
        client.burn_from(&spender, &owner, &1);
    }

    #[test]
    fn test_burn_from_leaves_other_allowances_untouched() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        let spender_a = Address::generate(&env);
        let spender_b = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&admin, &owner, &1000);
        client.approve(&owner, &spender_a, &300, &999999);
        client.approve(&owner, &spender_b, &400, &999999);

        client.burn_from(&spender_a, &owner, &100);

        assert_eq!(client.allowance(&owner, &spender_a), 200);
        assert_eq!(client.allowance(&owner, &spender_b), 400);
    }

    #[test]
    fn test_burned_supply_is_not_reminted_by_claim() {
        // Burning must not free up headroom that lets a learner claim twice.
        let env = Env::default();
        let (_, lt_contract_id, pt_contract_id) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);
        let pt_client = progress_tracker::ProgressTrackerClient::new(&env, &pt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "math_101");
        let quiz_id = Symbol::new(&env, "quiz_math_101");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 85);

        client.claim_reward(&learner, &course_id, &quiz_id);
        client.burn(&learner, &8500);

        assert_eq!(client.balance(&learner), 0);
        assert_eq!(client.total_supply(), 0);

        // The claim is still recorded, so the reward cannot be taken again.
        let result = client.try_claim_reward(&learner, &course_id, &quiz_id);
        assert!(result.is_err());
    }

    // ── #78: reward cap ─────────────────────────────────────────────────────

    #[test]
    fn test_claim_reward_at_max_score_succeeds() {
        let env = Env::default();
        let (_, lt_contract_id, pt_contract_id) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);
        let pt_client = progress_tracker::ProgressTrackerClient::new(&env, &pt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "math_101");
        let quiz_id = Symbol::new(&env, "quiz_math_101");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 100);

        client.claim_reward(&learner, &course_id, &quiz_id);
        // 100 * 100 = 10_000 (equals MAX_REWARD_AMOUNT)
        assert_eq!(client.balance(&learner), 10_000);
    }

    // ── #111: expired allowances can be explicitly pruned ────────────────────

    #[test]
    fn test_prune_expired_allowance_removes_stale_entry() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        let expiration_ledger = env.ledger().sequence() + 10;
        client.approve(&owner, &spender, &100, &expiration_ledger);

        // Nobody ever calls allowance() or transfer_from() for this pair again
        // -- advance past expiration and prune it directly.
        env.ledger()
            .with_mut(|l| l.sequence_number = expiration_ledger + 1);

        assert!(client.prune_expired_allowance(&owner, &spender));

        let key = storage::TokenDataKey::Allowance(storage::AllowanceKey {
            owner: owner.clone(),
            spender: spender.clone(),
        });
        env.as_contract(&lt_contract_id, || {
            assert!(!env.storage().temporary().has(&key));
        });
    }

    #[test]
    fn test_allowance_getter_does_not_remove_stale_entry() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        let expiration_ledger = env.ledger().sequence() + 10;
        client.approve(&owner, &spender, &100, &expiration_ledger);

        env.ledger()
            .with_mut(|l| l.sequence_number = expiration_ledger + 1);

        // Call the getter allowance()
        assert_eq!(client.allowance(&owner, &spender), 0);

        // Confirm the key STILL exists in storage because the getter did not mutate it
        let key = storage::TokenDataKey::Allowance(storage::AllowanceKey {
            owner: owner.clone(),
            spender: spender.clone(),
        });
        env.as_contract(&lt_contract_id, || {
            assert!(env.storage().temporary().has(&key));
        });
    }

    #[test]
    fn test_prune_expired_allowance_is_noop_for_active_allowance() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        env.mock_all_auths();

        client.approve(&owner, &spender, &100, &(env.ledger().sequence() + 999));

        assert!(!client.prune_expired_allowance(&owner, &spender));
        assert_eq!(client.allowance(&owner, &spender), 100);
    }

    #[test]
    fn test_prune_expired_allowance_is_noop_when_none_exists() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);

        assert!(!client.prune_expired_allowance(&owner, &spender));
    }

    // ── #112: RewardClaimed entries have their TTL extended on write ─────────

    #[test]
    fn test_claim_reward_extends_reward_claimed_ttl() {
        let env = Env::default();
        let (_, lt_contract_id, pt_contract_id) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);
        let pt_client = progress_tracker::ProgressTrackerClient::new(&env, &pt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "math_101");
        let quiz_id = Symbol::new(&env, "quiz_math_101");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 85);

        client.claim_reward(&learner, &course_id, &quiz_id);

        let key = storage::TokenDataKey::RewardClaimed(storage::RewardKey {
            learner: learner.clone(),
            course_id: course_id.clone(),
            quiz_id: quiz_id.clone(),
        });
        env.as_contract(&lt_contract_id, || {
            let ttl = env.storage().persistent().get_ttl(&key);
            // The entry must outlive the default minimum persistent TTL, since
            // it is the only guard against double-claiming a reward and can
            // never be allowed to lapse into archival (#112).
            assert!(
                ttl >= chainlearn_shared::PERSISTENT_TTL_EXTEND_TO - 1,
                "expected RewardClaimed TTL to be extended, got {}",
                ttl
            );
        });
    }

    // ── estimate_claim_gas (#199) ────────────────────────────────────────

    #[test]
    fn test_estimate_claim_gas_matches_actual_claim_reward() {
        let env = Env::default();
        let (_, lt_contract_id, pt_contract_id) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);
        let pt_client = progress_tracker::ProgressTrackerClient::new(&env, &pt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "math_101");
        let quiz_id = Symbol::new(&env, "quiz_math_101");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 85);

        let estimate = client.estimate_claim_gas(&learner, &course_id, &quiz_id);
        assert!(estimate.would_succeed);
        assert_eq!(estimate.estimated_reward, 8500);

        // The estimate must not have mutated anything: the real claim still
        // succeeds afterwards and mints exactly the estimated amount.
        client.claim_reward(&learner, &course_id, &quiz_id);
        assert_eq!(client.balance(&learner), 8500);
    }

    #[test]
    fn test_estimate_claim_gas_reports_already_claimed_without_panicking() {
        let env = Env::default();
        let (_, lt_contract_id, pt_contract_id) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);
        let pt_client = progress_tracker::ProgressTrackerClient::new(&env, &pt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        let course_id = Symbol::new(&env, "math_101");
        let quiz_id = Symbol::new(&env, "quiz_math_101");
        create_course_and_submit_quiz(&env, &pt_client, &learner, &course_id, &quiz_id, 85);
        client.claim_reward(&learner, &course_id, &quiz_id);

        let estimate = client.estimate_claim_gas(&learner, &course_id, &quiz_id);
        assert!(!estimate.would_succeed);
        assert_eq!(estimate.estimated_reward, 0);
        assert_eq!(
            estimate.failure_reason,
            SorobanString::from_str(&env, "reward already claimed")
        );
    }

    // ── cleanup_expired_allowances (#201) ────────────────────────────────

    #[test]
    fn test_cleanup_expired_allowances_removes_only_expired_entries() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        let expired_spender = Address::generate(&env);
        let active_spender = Address::generate(&env);
        env.mock_all_auths();

        let expiring_ledger = env.ledger().sequence() + 10;
        let far_future_ledger = env.ledger().sequence() + 10_000;
        client.approve(&owner, &expired_spender, &100, &expiring_ledger);
        client.approve(&owner, &active_spender, &200, &far_future_ledger);

        assert_eq!(client.allowance_spender_count(&owner), 2);

        env.ledger()
            .with_mut(|l| l.sequence_number = expiring_ledger + 1);

        let removed = client.cleanup_expired_allowances(&owner);
        assert_eq!(removed, 1);
        assert_eq!(client.allowance_spender_count(&owner), 1);
        assert_eq!(client.allowance(&owner, &active_spender), 200);
        assert_eq!(client.allowance(&owner, &expired_spender), 0);
    }

    #[test]
    fn test_cleanup_expired_allowances_is_permissionless() {
        // No auth is required to call it — it only removes data that is
        // already expired and therefore already worthless. Deliberately
        // does NOT call env.mock_all_auths() for the cleanup call itself.
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);

        env.mock_all_auths();
        let expiring_ledger = env.ledger().sequence() + 10;
        client.approve(&owner, &spender, &100, &expiring_ledger);
        env.ledger()
            .with_mut(|l| l.sequence_number = expiring_ledger + 1);

        env.set_auths(&[]);
        assert_eq!(client.cleanup_expired_allowances(&owner), 1);
    }

    #[test]
    fn test_cleanup_expired_allowances_noop_for_owner_with_no_allowances() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let owner = Address::generate(&env);
        assert_eq!(client.cleanup_expired_allowances(&owner), 0);
        assert_eq!(client.allowance_spender_count(&owner), 0);
    }

    // ── upgrade (#198) ────────────────────────────────────────────────────

    #[test]
    fn test_upgrade_version_and_wasm_hash_default_before_any_upgrade() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        assert_eq!(client.upgrade_version(), 0);
        assert_eq!(client.wasm_hash(), None);
    }

    #[test]
    #[should_panic]
    fn test_upgrade_requires_admin_auth() {
        let env = Env::default();
        let (admin, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        // No mock_all_auths() and no explicit admin auth: require_auth must panic.
        let fake_hash = BytesN::from_array(&env, &[7u8; 32]);
        client.upgrade(&fake_hash);
    }
}
