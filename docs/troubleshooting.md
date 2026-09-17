# Troubleshooting Guide

This guide covers common issues, contract error codes, diagnostic techniques, and frequently asked questions encountered when building, testing, and deploying ChainLearn smart contracts on Stellar/Soroban.

---

## Table of Contents

1. [Compilation & Build Errors](#compilation--build-errors)
2. [Contract-Specific Error Codes](#contract-specific-error-codes)
3. [Transaction & Invocation Failures](#transaction--invocation-failures)
4. [Testing & Simulation Issues](#testing--simulation-issues)
5. [Cross-Contract Call Failures](#cross-contract-call-failures)
6. [Debugging Tips & Tools](#debugging-tips--tools)
7. [Frequently Asked Questions (FAQ)](#frequently-asked-questions-faq)

---

## Compilation & Build Errors

### 1. `can't find crate for 'core'` or Missing Target

**Symptom:**
```text
error[E0463]: can't find crate for `core`
  |
  = note: the `wasm32-unknown-unknown` target may not be installed
```

**Cause:** The Rust compiler needs the WebAssembly target installed to compile Soroban contracts.

**Solution:**
Install the WebAssembly target for your current toolchain:
```bash
rustup target add wasm32-unknown-unknown
```

---

### 2. `error: cannot find macro 'panic'` in `#![no_std]`

**Symptom:** Compilation fails when building with `cargo build --target wasm32-unknown-unknown --release`.

**Cause:** Soroban contracts run in a `#![no_std]` environment. Standard library features like `std::string::String`, `std::vec::Vec`, or standard file I/O are unsupported on-chain.

**Solution:**
- Ensure all types are imported from `soroban_sdk` (e.g., `soroban_sdk::String`, `soroban_sdk::Vec`, `soroban_sdk::Address`).
- Use `soroban_sdk::panic_with_error!` rather than standard `panic!`.

---

### 3. Cargo Lock or Dependency Mismatch

**Symptom:**
```text
error: package `soroban-sdk v...` cannot be built due to mismatched dependency versions
```

**Solution:**
Update dependencies or sync Cargo.lock:
```bash
cargo update --workspace
```

---

## Contract-Specific Error Codes

### `learn-token` Errors

| Error Code | Identifier | Description | Resolution |
| :--- | :--- | :--- | :--- |
| **0** | `AlreadyInitialized` | Contract has already been initialized with an admin and token parameters. | Do not re-call `initialize`. Use current token configuration or deploy a new instance. |
| **1** | `ZeroAddress` | An invalid zero/empty address was supplied. | Ensure valid non-zero Stellar addresses are passed as caller/recipient. |
| **2** | `RewardCapped` | Attempted reward claim exceeds `MAX_REWARD_AMOUNT` (10,000 tokens). | Ensure learner quiz score does not exceed maximum allowable reward calculation (`score * BASE_REWARD_PER_POINT`). |

### `credential-nft` Errors

| Error Code | Identifier | Description | Resolution |
| :--- | :--- | :--- | :--- |
| **0** | `AlreadyInitialized` | The credential contract was already initialized. | Initialize only once upon deployment. |
| **1** | `Soulbound` | Attempted to call `transfer()` or transfer ownership. | Credentials are strictly soulbound to the earner. Transfers are permanently prohibited by design. |

### `progress-tracker` Errors

| Error Code | Identifier | Description | Resolution |
| :--- | :--- | :--- | :--- |
| **0** | `AlreadyInitialized` | Progress tracker contract is already initialized. | Re-initialization is prohibited. |

---

## Transaction & Invocation Failures

### 1. `HostError: Error(Auth, InvalidAction)`

**Symptom:**
The transaction fails during pre-flight simulation or on-chain execution with authentication error.

**Cause:** The caller address did not sign or author the transaction with the matching address passed into `require_auth()` or `require_auth_for_args()`.

**Solution:**
- For CLI: Add `--source <IDENTITY>` matching the required signer.
- For tests: Ensure `env.mock_all_auths()` is called or specific mock auth is configured prior to invoking the contract function.

---

### 2. `HostError: Error(Storage, ExistingValue)`

**Symptom:**
Re-submitting course enrollment or re-claiming a reward causes transaction failure.

**Cause:** The contract enforces unique records (e.g., each quiz reward can only be claimed once per learner, each learner can only be enrolled once per course).

**Solution:**
- Call query entrypoints (`get_progress`, `get_quiz_score`) first to verify status before dispatching state-changing transactions.

---

### 3. `HostError: Error(Budget, ExceededLimit)`

**Symptom:** Invocation consumes more CPU instructions or RAM than the Soroban ledger limit.

**Cause:** Iterating over unbounded storage structures or oversized vector payloads.

**Solution:**
- Use pagination (`get_credentials_for(learner, start, limit)`).
- Store compact data keys and avoid repetitive instance storage reads in loops.

---

## Testing & Simulation Issues

### 1. Test Panics with `MockAuth` Mismatch

**Symptom:**
```text
thread 'tests::test_claim' panicked at 'called `Result::unwrap()` on an `Err` value: HostError: Error(Auth, ...)'
```

**Solution:**
In test setups, use `env.mock_all_auths()` before making contract calls:
```rust
let env = Env::default();
env.mock_all_auths();
let client = LearnTokenClient::new(&env, &contract_id);
```

---

### 2. Snapshot or Ledger State Discrepancy

**Symptom:** Tests succeed individually with `cargo test <test_name>` but fail when executed together in full test suite.

**Cause:** Shared mock environments or reliance on non-deterministic ordering.

**Solution:**
Ensure each test instantiates an isolated `Env::default()` instance.

---

## Cross-Contract Call Failures

### 1. Missing Progress Tracker Registration in `learn-token` or `credential-nft`

**Symptom:**
Calling `claim_reward` or `mint_credential` fails with contract invocation error.

**Cause:**
`learn-token` and `credential-nft` query the `progress-tracker` contract to verify scores and completion. If the progress-tracker address is not properly linked or points to an uninitialized address, the cross-contract call will revert.

**Solution:**
1. Deploy `progress-tracker` first.
2. Initialize `learn-token` and `credential-nft` passing the valid `progress-tracker` contract address.

---

## Debugging Tips & Tools

1. **Enable Soroban Logs in Tests:**
   Use `std::println!` inside tests or inspect contract events via `env.events().all()`.

2. **Soroban CLI Simulation:**
   Simulate transactions without broadcasting to verify authorization and footprint:
   ```bash
   soroban contract invoke \
     --id <CONTRACT_ID> \
     --source <IDENTITY> \
     --network testnet \
     -- <FUNCTION> <ARGS>
   ```

3. **Format & Clippy Checks:**
   Catch structural bugs before deployment:
   ```bash
   cargo fmt --all --check
   cargo clippy --workspace --all-targets -- -D warnings
   ```

---

## Frequently Asked Questions (FAQ)

#### Q: Can credential NFTs be transferred to a cold wallet after minting?
**A:** No. ChainLearn credential NFTs are strictly non-transferable (soulbound). Any call to `transfer()` returns `ContractError::Soulbound (1)` and reverts.

#### Q: How is the quiz reward calculated?
**A:** The reward amount is calculated as `score * BASE_REWARD_PER_POINT` (100 tokens per score point), capped at `MAX_REWARD_AMOUNT` (10,000 tokens for a score of 100).

#### Q: Can a learner re-take a quiz and claim rewards again?
**A:** Quizzes can be re-taken to improve the course average score in `progress-tracker`, but reward claims in `learn-token` are idempotent and single-claim per quiz to prevent double-spending.

#### Q: Where can I report bugs or security vulnerabilities?
**A:** Please open an issue on GitHub or refer to `CONTRIBUTING.md` for our responsible disclosure guidelines.
