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
#[should_panic(expected = "HostError: Error(Contract, #1)")]
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
