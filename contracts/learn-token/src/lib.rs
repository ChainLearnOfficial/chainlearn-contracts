#![no_std]

mod events;
mod storage;

use chainlearn_shared::{BASE_REWARD_PER_POINT, MAX_QUIZ_SCORE};
use soroban_sdk::{
    contract, contracterror, contractimpl, Address, Env, String as SorobanString, Symbol,
};
use soroban_token_sdk::metadata::TokenMetadata;

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

        let metadata = TokenMetadata {
            name,
            symbol,
            decimal,
        };
        env.storage()
            .persistent()
            .set(&storage::TokenDataKey::TokenMetadata, &metadata);
        Ok(())
    }

    // ── SEP-41 Standard Interface ─────────────────────────────────────────

    /// Returns the token name.
    pub fn name(env: Env) -> SorobanString {
        let metadata: TokenMetadata = env
            .storage()
            .persistent()
            .get(&storage::TokenDataKey::TokenMetadata)
            .expect("not initialized");
        metadata.name
    }

    /// Returns the token symbol.
    pub fn symbol(env: Env) -> SorobanString {
        let metadata: TokenMetadata = env
            .storage()
            .persistent()
            .get(&storage::TokenDataKey::TokenMetadata)
            .expect("not initialized");
        metadata.symbol
    }

    /// Returns the number of decimals.
    pub fn decimals(env: Env) -> u32 {
        let metadata: TokenMetadata = env
            .storage()
            .persistent()
            .get(&storage::TokenDataKey::TokenMetadata)
            .expect("not initialized");
        metadata.decimal
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

        storage::set_balance(&env, &from, from_balance - amount);
        let to_balance = storage::get_balance(&env, &to);
        storage::set_balance(&env, &to, to_balance + amount);

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

        storage::reduce_allowance(&env, &from, &spender, amount);
        storage::set_balance(&env, &from, from_balance - amount);
        let to_balance = storage::get_balance(&env, &to);
        storage::set_balance(&env, &to, to_balance + amount);

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
        events::approve(&env, &owner, &spender, amount, expiration_ledger);
    }

    /// Returns the allowance for a spender on behalf of an owner.
    /// Emits an allowance_expired event if the allowance has expired.
    pub fn allowance(env: Env, owner: Address, spender: Address) -> i128 {
        let (exists, is_expired, expiration_ledger) =
            storage::check_allowance_expired_readonly(&env, &owner, &spender);

        if exists && is_expired {
            events::allowance_expired(&env, &owner, &spender, expiration_ledger);
        }

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
    pub fn mint(env: Env, to: Address, amount: i128) {
        let admin = storage::get_admin(&env);
        admin.require_auth();

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

        if storage::is_reward_claimed(&env, &learner, &course_id, &quiz_id) {
            panic!("reward already claimed");
        }

        // Verify score by querying the progress-tracker contract.
        // We construct the client once per call to avoid redundant loading gas overhead.
        let client = get_progress_client(&env);
        let score = client.get_quiz_score(&learner, &course_id, &quiz_id);

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

        // Mint tokens to the learner
        let current_balance = storage::get_balance(&env, &learner);
        storage::set_balance(&env, &learner, current_balance + reward_amount);

        let current_supply = storage::get_total_supply(&env);
        storage::set_total_supply(&env, current_supply + reward_amount);

        // Mark reward as claimed to prevent double-claiming
        storage::set_reward_claimed(&env, &learner, &course_id, &quiz_id);

        events::reward_claimed(&env, &learner, &quiz_id, score, reward_amount, &course_id);
    }

    // ── Admin ─────────────────────────────────────────────────────────────

    /// Returns the admin address.
    pub fn admin(env: Env) -> Address {
        storage::get_admin(&env)
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
            .persistent()
            .get(&storage::TokenDataKey::Allowance(key.clone()))
            .expect("allowance not set");
        storage::set_allowance(&env, &owner, &spender, new_amount, data.expiration_ledger);
        events::approve(&env, &owner, &spender, new_amount, data.expiration_ledger);
    }
}

fn get_progress_client(env: &Env) -> ProgressTrackerClient<'_> {
    let address = storage::get_progress_tracker(env);
    ProgressTrackerClient::new(env, &address)
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

    #[test]
    fn test_mint() {
        let env = Env::default();
        let (_, lt_contract_id, _) = setup(&env);
        let client = LearnTokenClient::new(&env, &lt_contract_id);

        let learner = Address::generate(&env);
        env.mock_all_auths();

        client.mint(&learner, &1000);

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

        client.mint(&alice, &500);
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

        let stranger = Address::generate(&env);
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

        client.mint(&alice, &500);
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

        client.mint(&owner, &1000);
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

        client.mint(&alice, &1000);
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

        client.mint(&alice, &500);
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

        client.mint(&alice, &100);
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

        client.mint(&alice, &100);
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

        client.mint(&alice, &100);
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
        client.mint(&alice, &100);

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

        client.mint(&owner, &1000);
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

        client.mint(&owner, &1000);
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

        client.mint(&owner, &50);
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

        client.mint(&owner, &1000);
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

        client.mint(&owner, &1000);
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
            assert!(!env.storage().persistent().has(&key));
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
            assert!(env.storage().persistent().has(&key));
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
}
