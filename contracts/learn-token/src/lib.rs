#![no_std]

mod events;
mod storage;

use chainlearn_shared::{BASE_REWARD_PER_POINT, MAX_QUIZ_SCORE};
use soroban_sdk::{contract, contractimpl, Address, Env, String as SorobanString, Symbol};
use soroban_token_sdk::metadata::TokenMetadata;

#[soroban_sdk::contractclient(name = "ProgressTrackerClient")]
pub trait ProgressTrackerInterface {
    fn get_quiz_score(env: Env, learner: Address, course_id: Symbol, quiz_id: Symbol) -> u32;
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
    pub fn initialize(
        env: Env,
        admin: Address,
        name: SorobanString,
        symbol: SorobanString,
        decimal: u32,
        progress_tracker: Address,
    ) {
        if storage::is_initialized(&env) {
            panic!("already initialized");
        }
        storage::set_admin(&env, &admin);
        storage::set_total_supply(&env, 0);
        storage::set_progress_tracker(&env, &progress_tracker);

        let metadata = TokenMetadata {
            name,
            symbol,
            decimal,
        };
        env.storage()
            .persistent()
            .set(&storage::DataKey::TokenMetadata, &metadata);
    }

    // ── SEP-41 Standard Interface ─────────────────────────────────────────

    /// Returns the token name.
    pub fn name(env: Env) -> SorobanString {
        let metadata: TokenMetadata = env
            .storage()
            .persistent()
            .get(&storage::DataKey::TokenMetadata)
            .expect("not initialized");
        metadata.name
    }

    /// Returns the token symbol.
    pub fn symbol(env: Env) -> SorobanString {
        let metadata: TokenMetadata = env
            .storage()
            .persistent()
            .get(&storage::DataKey::TokenMetadata)
            .expect("not initialized");
        metadata.symbol
    }

    /// Returns the number of decimals.
    pub fn decimals(env: Env) -> u32 {
        let metadata: TokenMetadata = env
            .storage()
            .persistent()
            .get(&storage::DataKey::TokenMetadata)
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

        if amount < 0 {
            panic!("negative amount");
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

        events::transfer(&env, &from, &to, amount);
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

        storage::set_allowance(&env, &owner, &spender, amount, expiration_ledger);
        events::approve(&env, &owner, &spender, amount, expiration_ledger);
    }

    /// Returns the allowance for a spender on behalf of an owner.
    pub fn allowance(env: Env, owner: Address, spender: Address) -> i128 {
        storage::get_allowance(&env, &owner, &spender)
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

        let current_balance = storage::get_balance(&env, &to);
        storage::set_balance(&env, &to, current_balance + amount);

        let current_supply = storage::get_total_supply(&env);
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
    pub fn claim_reward(env: Env, learner: Address, course_id: Symbol, quiz_id: Symbol) -> i128 {
        learner.require_auth();

        if storage::is_reward_claimed(&env, &learner, &quiz_id) {
            panic!("reward already claimed");
        }

        // Verify score by querying the progress-tracker contract
        let progress_tracker = storage::get_progress_tracker(&env);
        let client = ProgressTrackerClient::new(&env, &progress_tracker);
        let score = client.get_quiz_score(&learner, &course_id, &quiz_id);

        if score > MAX_QUIZ_SCORE {
            panic!("score exceeds maximum");
        }

        let reward_amount = (score as i128) * BASE_REWARD_PER_POINT;

        // Mint tokens to the learner
        let current_balance = storage::get_balance(&env, &learner);
        storage::set_balance(&env, &learner, current_balance + reward_amount);

        let current_supply = storage::get_total_supply(&env);
        storage::set_total_supply(&env, current_supply + reward_amount);

        // Mark reward as claimed to prevent double-claiming
        storage::set_reward_claimed(&env, &learner, &quiz_id);

        events::reward_claimed(&env, &learner, &quiz_id, score, reward_amount);
        events::mint(&env, &learner, reward_amount);

        reward_amount
    }

    // ── Admin ─────────────────────────────────────────────────────────────

    /// Returns the admin address.
    pub fn admin(env: Env) -> Address {
        storage::get_admin(&env)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, String as SorobanString, Vec};

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
        pt_client.create_course(course_id, &1, &1, &module_ids);
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

        let reward = client.claim_reward(&learner, &course_id, &quiz_id);

        // 85 * 100 (BASE_REWARD_PER_POINT) = 8500
        assert_eq!(reward, 8500);
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
}
