# Testing Guide

This guide provides comprehensive instructions and best practices for writing, running, and maintaining tests across the ChainLearn Soroban smart contract workspace.

---

## Table of Contents

1. [Testing Architecture](#testing-architecture)
2. [Running Tests](#running-tests)
3. [Writing Unit Tests](#writing-unit-tests)
4. [Cross-Contract & Integration Tests](#cross-contract--integration-tests)
5. [Security & Invariant Testing](#security--invariant-testing)
6. [Testing Events & Logs](#testing-events--logs)
7. [Snapshot Testing](#snapshot-testing)
8. [Best Practices](#best-practices)

---

## Testing Architecture

ChainLearn uses the native Soroban SDK test harness (`soroban-sdk::Env`) coupled with Rust's standard test runner. Tests are structured into unit tests, integration tests, and snapshot tests:

```text
chainlearn-contracts/
├── tests/
│   ├── unit/
│   │   ├── token_tests.rs              # learn-token unit test suite
│   │   ├── progress_tests.rs           # progress-tracker unit test suite
│   │   ├── credential_tests.rs         # credential-nft unit test suite
│   │   ├── event_emission_tests.rs     # event topic and data verification
│   │   ├── error_message_tests.rs      # contract error codes and reversions
│   │   ├── security_arithmetic_tests.rs# math boundaries and overflow/underflow
│   │   └── xcontract_call_tests.rs     # cross-contract score & claim calls
│   └── integration/
│       ├── security_auth_tests.rs      # multi-sig & authorization boundary checks
│       ├── security_double_spending_tests.rs # double claim / double submit defense
│       ├── security_reentrancy_tests.rs# reentrancy guard verification
│       └── upgrade_tests.rs            # contract wasm migration tests
└── test_snapshots/                     # deterministic JSON execution snapshots
```

---

## Running Tests

### 1. Run Entire Test Suite

```bash
cargo test
```

### 2. Run Specific Subsystem Tests

```bash
# Run unit tests only
cargo test --test unit

# Run integration tests only
cargo test --test integration

# Run a specific test function
cargo test test_claim_reward_fetches_score_from_progress_tracker
```

### 3. Run with Full Log Output

```bash
cargo test -- --nocapture
```

---

## Writing Unit Tests

Unit tests focus on isolated contract logic, state transitions, and boundary conditions.

### Setting Up Test Environment

Each unit test should construct a fresh `soroban_sdk::Env`:

```rust
use soroban_sdk::{testutils::Address as _, Address, Env, String as SorobanString, Symbol};
use learn_token::{LearnToken, LearnTokenClient};

#[test]
fn test_token_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, LearnToken);
    let client = LearnTokenClient::new(&env, &contract_id);

    client.initialize(
        &admin,
        &SorobanString::from_str(&env, "ChainLearn Token"),
        &SorobanString::from_str(&env, "LEARN"),
        &7,
    );

    assert_eq!(client.balance(&admin), 0);
    assert_eq!(client.total_supply(), 0);
}
```

### Testing Error Reversions

Verify that invalid operations panic with the expected contract error variant:

```rust
#[test]
#[should_panic(expected = "Error(Contract, #1)")] // ContractError::Soulbound = 1
fn test_credential_transfer_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let learner = Address::generate(&env);
    let recipient = Address::generate(&env);
    let contract_id = env.register_contract(None, CredentialNft);
    let client = CredentialNftClient::new(&env, &contract_id);

    // Any call to transfer must fail because credentials are soulbound
    client.transfer(&learner, &recipient, &1u32);
}
```

---

## Cross-Contract & Integration Tests

Cross-contract testing verifies interactions between `learn-token`, `progress-tracker`, and `credential-nft`.

### Multi-Contract Setup Example

```rust
#[test]
fn test_end_to_end_reward_claim() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let learner = Address::generate(&env);

    // 1. Deploy progress-tracker
    let tracker_id = env.register_contract(None, ProgressTracker);
    let tracker_client = ProgressTrackerClient::new(&env, &tracker_id);
    tracker_client.initialize(&admin);

    // 2. Deploy learn-token
    let token_id = env.register_contract(None, LearnToken);
    let token_client = LearnTokenClient::new(&env, &token_id);
    token_client.initialize(&admin, ...);
    token_client.set_progress_tracker(&tracker_id);

    // 3. Learner completes quiz in tracker
    let course_id = Symbol::new(&env, "rust101");
    let quiz_id = Symbol::new(&env, "quiz1");
    tracker_client.enroll(&learner, &course_id);
    tracker_client.submit_quiz_score(&learner, &course_id, &quiz_id, &95u32);

    // 4. Learner claims reward via learn-token
    token_client.claim_reward(&learner, &course_id, &quiz_id);

    // 5. Assert reward minted (95 * 100 = 9,500 LEARN tokens)
    assert_eq!(token_client.balance(&learner), 9_500i128);
}
```

---

## Security & Invariant Testing

Our test suite enforces strict security invariants:

1. **Authorization Enforceability**: Every state-mutating function must verify `caller.require_auth()`. In `tests/integration/security_auth_tests.rs`, we test calls without mock auth to ensure host auth denial.
2. **Double-Action Prevention**:
   - `test_double_claim_reward_is_prevented`: Verifies that a quiz reward cannot be claimed twice.
   - `test_double_enroll_is_prevented`: Verifies that a learner cannot re-enroll into an active course.
   - `test_double_mint_credential_is_prevented`: Verifies that only one credential NFT can be issued per completed course.
3. **Reentrancy Protection**: `security_reentrancy_tests.rs` validates reentrancy guards during token transfers and external client invocations.
4. **Arithmetic Invariants**: `security_arithmetic_tests.rs` tests for integer overflow, underflow on burn, and supply cap saturations.

---

## Testing Events & Logs

Verify that expected events are emitted with correct indexed topics and payloads:

```rust
use soroban_sdk::{testutils::Events, IntoVal};

#[test]
fn test_credential_minted_event() {
    let env = Env::default();
    env.mock_all_auths();

    // ... perform credential minting ...

    let events = env.events().all();
    let last_event = events.last().expect("expected event");
    
    // Assert topic[0] is event name
    assert_eq!(
        last_event.1, // topics
        (Symbol::new(&env, "credential_minted"), learner.clone()).into_val(&env)
    );
}
```

---

## Snapshot Testing

Snapshot tests ensure deterministic transaction footprints and gas consumption across code changes:

- Snapshots are stored in `test_snapshots/`.
- When modifying contract logic or data layout, run:
  ```bash
  UPDATE_SNAPSHOTS=1 cargo test
  ```
- Inspect diffs with `git diff test_snapshots/` to verify gas and state footprint changes before submitting PRs.

---

## Best Practices

1. **Deterministic State**: Do not share mutable state across tests. Always create a new `Env::default()`.
2. **Mock Auths Strategically**: Use `env.mock_all_auths()` for functional unit tests, but use explicit unmocked auth tests in `security_auth_tests.rs` to verify authentication barriers.
3. **Test Edge Cases**: Always test boundary scores: 0 (minimum), 49 (below passing threshold of 50), 50 (exact passing threshold), and 100 (maximum quiz score).
4. **Keep Tests Hermetic**: Avoid depending on real network endpoints, RPCs, or wall-clock timestamps (`env.ledger().set_timestamp(...)` should be used instead).
