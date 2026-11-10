# Testing Guide

This guide outlines the conventions and procedures for testing ChainLearn contracts. Testing is critical for maintaining the security and reliability of the platform.

## Test Setup

ChainLearn uses the standard Rust testing framework along with `soroban-sdk`'s test utilities.

1. **Prerequisites**: Ensure you have Rust installed with the appropriate targets.
2. **Running Tests**:
   - To run all tests: `cargo test`
   - To run tests for a specific contract: `cargo test -p <contract_name>`
   - Example: `cargo test -p learn-token`

## Mocking Patterns

When testing contracts that interact with other contracts (cross-contract calls), use the `soroban-sdk` environment to register contracts and mock their behavior.

### Registering Contracts in Tests
```rust
#![cfg(test)]
use soroban_sdk::{Env, Address};
use crate::{LearnTokenClient, LearnToken};

fn setup_test() -> (Env, LearnTokenClient<'static>, Address) {
    let env = Env::default();
    let contract_id = env.register_contract(None, LearnToken);
    let client = LearnTokenClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    (env, client, admin)
}
```

### Mocking External Calls
If you need to mock an external contract, define a dummy contract with the same interface and register it in the test environment in place of the real contract.

## Assertion Patterns

Use standard Rust assertions along with expected state checks.

- `assert_eq!(client.balance(&user), expected_balance);`
- `assert!(client.is_enrolled(&user));`

When testing for expected panics (e.g., unauthorized access), use the `#[should_panic(expected = "...")]` attribute.

```rust
#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_unauthorized_access() {
    let (env, client, _admin) = setup_test();
    let unauthorized_user = Address::generate(&env);
    // This should panic
    client.admin_function(&unauthorized_user);
}
```

## Coverage Requirements

To ensure robustness, the following coverage requirements must be met:

1. **High Coverage (90%+)**: All core contracts must have at least 90% instruction coverage.
2. **Edge Cases**: Explicit tests must be written for boundary conditions, authorization failures, and arithmetic overflows.
3. **Integration Tests**: Tests verifying the interactions between `learn-token`, `credential-nft`, and `progress-tracker` must be included.

Run coverage tools (e.g., `tarpaulin`) locally before submitting a pull request to verify requirements are met.

## Gas Regression Thresholds

Execution cost is guarded by `./scripts/estimate-gas.sh`, which runs the
benchmarks in `tests/benchmarks/gas_benchmarks.rs` and compares each operation's
CPU instruction count against a per-operation threshold. CI runs this script on
every pull request and fails if any operation exceeds its threshold.

Thresholds are defined in `scripts/gas-thresholds.env`. To update one:

1. Run `./scripts/estimate-gas.sh` and note the reported `CPU INSNS` for the
   operation.
2. Set `GAS_LIMIT_<operation>` with headroom above the observed cost (the
   in-test baselines use roughly 2x the observed cost).
3. Commit the changed threshold together with the change that moved the cost,
   and explain the increase in the pull request.

## Gas Benchmarks (`tests/benchmarks/`)

The workspace includes a dedicated CPU-instruction benchmark suite in `tests/benchmarks/gas_benchmarks.rs` (within the `integration-tests` package) that measures the execution cost of critical contract operations such as token minting, reward claiming, course creation, module completion, and NFT credential minting.

### Running Gas Benchmarks
```bash
# Run the benchmark suite with instruction output printed to stdout
cargo test --test gas_benchmarks -- --nocapture

# Run the automated gas threshold verification script used in CI
./scripts/estimate-gas.sh
```

Each benchmark resets the Soroban `Env` budget (`env.budget().reset_default()`), executes the target contract invocation, and asserts that `env.budget().cpu_instruction_cost()` remains below both the in-test safety baseline and the CI threshold configured in `scripts/gas-thresholds.env`.

## Security Test Suites

Security-critical invariants are verified in dedicated suites (such as `tests/security_tests.rs` and per-contract security modules):

1. **Authorization & Spoofing Resistance**:
   Verify that privileged entry points (`mint`, `create_course`, `revoke_credential`, `set_admin`, `pause`) reject non-admin callers and that learner-scoped functions require the learner's own signature.
2. **Arithmetic Overflow, Underflow, and Boundary Checks**:
   Exercise `0`, `1`, maximum quiz scores (`100`), boundary token amounts (`i128::MAX` / `u64::MAX`), and invalid percentages to ensure checked arithmetic prevents silent wraparound.
3. **Double-Spending & Replay Prevention**:
   Confirm that learners cannot claim token rewards twice for the same quiz (`claim_reward`), complete the same course module repeatedly to inflate progress, or mint duplicate credentials for the same `(learner, course_id)` pair.
4. **Reentrancy & State Ordering**:
   Ensure contract state updates (such as marking a reward claimed or a credential revoked) are persisted to storage before or atomically with external cross-contract calls.

```bash
cargo test --test security_tests -- --nocapture
```

## Cross-Contract Test Patterns

ChainLearn's architecture links three contracts together: `progress-tracker` (source of truth for courses, module completion, and quiz scores), `learn-token` (mints rewards after verifying quiz completion with `progress-tracker`), and `credential-nft` (mints soulbound credentials after verifying course completion and score thresholds with `progress-tracker`).

When writing cross-contract integration tests in `tests/`:

1. **Register all participating contracts in the same `Env`** and mock auths with `env.mock_all_auths()`.
2. **Initialize contracts in dependency order**: initialize `progress-tracker` first, then pass its `Address` when initializing `learn-token` and `credential-nft`.
3. **Exercise both happy-path and cross-contract rejection flows** (for example, attempting to mint a credential before `progress-tracker` records all modules complete).

```rust
use credential_nft::{CredentialNft, CredentialNftClient};
use learn_token::{LearnToken, LearnTokenClient};
use progress_tracker::{ProgressTracker, ProgressTrackerClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

struct CrossContractFixture<'a> {
    env: Env,
    admin: Address,
    learner: Address,
    tracker: ProgressTrackerClient<'a>,
    token: LearnTokenClient<'a>,
    nft: CredentialNftClient<'a>,
}

impl<'a> CrossContractFixture<'a> {
    fn setup() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let learner = Address::generate(&env);

        let tracker_id = env.register_contract(None, ProgressTracker);
        let token_id = env.register_contract(None, LearnToken);
        let nft_id = env.register_contract(None, CredentialNft);

        let tracker = ProgressTrackerClient::new(&env, &tracker_id);
        let token = LearnTokenClient::new(&env, &token_id);
        let nft = CredentialNftClient::new(&env, &nft_id);

        // Initialize in dependency order: tracker first, then dependents
        tracker.initialize(&admin);
        token.initialize(&admin, &tracker_id);
        nft.initialize(&admin, &tracker_id);

        Self { env, admin, learner, tracker, token, nft }
    }
}
```

## Fuzzing and Property-Based Testing

For arithmetic-heavy or state-transition logic (such as token vesting schedules, reward calculations, pagination boundaries, and progress percentage math), use property-based or randomized input loops to test invariants across wide input domains:

- **Conservation Invariants**: Total supply equals the sum of all holder balances after any sequence of mints, transfers, and burns.
- **Monotonicity**: Completing additional modules or increasing quiz scores never decreases a learner's completion percentage or reward eligibility.
- **Idempotency & Rejection of Out-of-Range Inputs**: Invalid scores (`> 100`), zero-module courses, or unauthorized callers always fail deterministically regardless of ledger timestamp or prior state.

## Complete Test Template for a New Contract Function

Use the template below when adding tests for a new contract entry point to ensure happy-path behavior, state persistence, authorization checks, and boundary conditions are all covered:

```rust
#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events as _},
    Address, Env, String,
};
use crate::{ProgressTracker, ProgressTrackerClient};

struct TestContext<'a> {
    env: Env,
    client: ProgressTrackerClient<'a>,
    admin: Address,
    learner: Address,
}

impl<'a> TestContext<'a> {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, ProgressTracker);
        let client = ProgressTrackerClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let learner = Address::generate(&env);

        client.initialize(&admin);
        Self { env, client, admin, learner }
    }
}

#[test]
fn test_new_function_happy_path_and_events() {
    let ctx = TestContext::new();
    let course_id = String::from_str(&ctx.env, "rust-101");
    let title = String::from_str(&ctx.env, "Intro to Soroban");

    ctx.client.create_course(&ctx.admin, &course_id, &title, &3u32);
    ctx.client.enroll(&ctx.learner, &course_id);

    assert!(ctx.client.is_enrolled(&ctx.learner, &course_id));
    assert!(!ctx.env.events().all().is_empty());
}

#[test]
#[should_panic]
fn test_new_function_rejects_unauthorized_caller() {
    let ctx = TestContext::new();
    let outsider = Address::generate(&ctx.env);
    let course_id = String::from_str(&ctx.env, "rust-101");
    let title = String::from_str(&ctx.env, "Intro to Soroban");

    // Non-admin caller must be rejected
    ctx.client.create_course(&outsider, &course_id, &title, &3u32);
}

#[test]
#[should_panic]
fn test_new_function_rejects_invalid_boundary_input() {
    let ctx = TestContext::new();
    let course_id = String::from_str(&ctx.env, "rust-101");
    let title = String::from_str(&ctx.env, "Intro to Soroban");

    // Zero modules is a boundary violation and must panic
    ctx.client.create_course(&ctx.admin, &course_id, &title, &0u32);
}
```
