# Cross-Contract Reentrancy Security Audit & Protection Report

## 1. Executive Summary
This document provides a comprehensive security audit of all cross-contract interactions across the ChainLearn smart contract ecosystem (`learn-token`, `credential-nft`, and `progress-tracker`), addressing **Issue #354**. 

Reentrancy represents a critical vulnerability class where an external contract invocation hands execution control to untrusted or compromised external code before the caller's state is fully finalized. The external contract may recursively invoke the caller contract to drain funds, double-mint tokens, or forge credentials.

This audit:
1. Conducted an exhaustive inventory of all cross-contract call sites across all contract packages.
2. Identified critical attack surfaces where external queries precede state mutation (Checks-Effects-Interactions violations).
3. Implemented a robust RAII `ReentrancyGuard` across `learn-token` and `credential-nft`.
4. Validated the defense with automated reentrancy attack simulation suites covering same-function reentrancy, cross-function reentrancy, and NFT credential reentrancy.

---

## 2. Cross-Contract Call Inventory

An audit of all contracts in the repository identified the following cross-contract calls:

| Caller Contract | Function | Target Contract | Target Function | Call Pattern | Risk Level |
|---|---|---|---|---|---|
| `learn-token` | `claim_reward` | `progress-tracker` | `get_quiz_score` | `env.invoke_contract` | **Critical** |
| `learn-token` | `batch_claim_reward` | `progress-tracker` | `get_quiz_score` | `env.invoke_contract` | **Critical** |
| `learn-token` | `preview_claim_reward` | `progress-tracker` | `get_quiz_score` | `env.invoke_contract` | Low (Read-only view) |
| `credential-nft` | `mint_credential` | `progress-tracker` | `course_exists` | `env.invoke_contract` | Medium |
| `credential-nft` | `mint_credential` | `progress-tracker` | `is_eligible_for_credential` | `env.invoke_contract` | **High** |
| `credential-nft` | `mint_credential` | `progress-tracker` | `get_course_score` | `env.invoke_contract` | Medium |
| `progress-tracker` | *None* | N/A | N/A | *No outbound cross-contract calls* | None |

---

## 3. Threat Model & Identified Vulnerabilities

### 3.1 Same-Function Reentrancy in `claim_reward`
- **Mechanism**: In `claim_reward`, the contract checks if the reward has been claimed (`is_reward_claimed`), and then calls `fetch_quiz_score` into the registered `progress_tracker`. The claim record (`storage::set_reward_claimed`) and learner balance increments were updated **after** the external call.
- **Exploit Path**: A malicious or hijacked tracker contract implementing `get_quiz_score` could recursively invoke `claim_reward` for the same learner and quiz. Because the first invocation had not yet set `RewardClaimed`, the nested invocation would pass the check, mint duplicate tokens, and inflate token supply.

### 3.2 Cross-Function Reentrancy
- **Mechanism**: While `claim_reward` is halted waiting for `fetch_quiz_score`, a reentrant contract could attempt to invoke other state-mutating functions such as `mint`, `transfer`, `burn`, or `transfer_from`.
- **Impact**: State inconsistency and race conditions between pending claims and unauthorized transfers or mints.

### 3.3 Reentrancy in `CredentialNft::mint_credential`
- **Mechanism**: During credential minting, `mint_credential` invokes `is_eligible_for_credential` prior to incrementing `CredentialCounter` and recording `CredentialDataKey::Credential(id)`.
- **Exploit Path**: A reentrant call could attempt duplicate credential issuance with manipulated IDs or circumvent module completion checks.

---

## 4. Remediation & Protection Architecture

To mitigate all reentrancy vectors across the codebase, a defense-in-depth approach has been implemented:

### 4.1 RAII Reentrancy Guard
A zero-cost RAII guard pattern is established in `learn-token` and `credential-nft`:
```rust
struct ReentrancyGuard<'a> {
    env: &'a Env,
}

impl<'a> ReentrancyGuard<'a> {
    fn enter(env: &'a Env) -> Self {
        if storage::is_reentrancy_locked(env) {
            panic!("reentrancy guard: reentrant call detected");
        }
        storage::set_reentrancy_locked(env, true);
        Self { env }
    }
}

impl<'a> Drop for ReentrancyGuard<'a> {
    fn drop(&mut self) {
        storage::set_reentrancy_locked(self.env, false);
    }
}
```

### 4.2 Instance Storage Integration
- Reentrancy locks are stored in Soroban instance storage (`TokenDataKey::ReentrancyGuard`), which is automatically scoped to contract execution and incurs zero persistent state inflation.
- In the event of a panic or revert, the host transaction atomically reverts all state changes, guaranteeing that the lock is never left stranded in a locked state.

### 4.3 Scope of Protection
The `ReentrancyGuard` is enforced on:
- `learn-token::claim_reward`
- `learn-token::batch_claim_reward`
- `learn-token::mint`
- `learn-token::transfer`
- `learn-token::transfer_from`
- `learn-token::burn`
- `learn-token::burn_from`
- `learn-token::claim_vested`
- `credential-nft::mint_credential`

---

## 5. Verification & Attack Scenarios

A dedicated integration test suite in `tests/integration/security_reentrancy_tests.rs` simulates adversarial contracts:

1. **`test_reentrancy_during_claim_reward_blocked_by_guard`**:
   - Deploys `ReentrantClaimProgressTracker` simulating an adversarial tracker callback into `claim_reward`.
   - Result: Nested call detected and halted with panic; learner balance remains 0; no tokens minted.
2. **`test_cross_function_reentrancy_blocked_by_guard`**:
   - Deploys `CrossFunctionReentrantProgressTracker` attempting to call `mint` during an active `claim_reward` evaluation.
   - Result: Cross-function reentrancy blocked; total supply and balances remain uncorrupted.
3. **`test_credential_nft_reentrancy_blocked_by_guard`**:
   - Deploys `ReentrantCredentialTracker` attempting to re-enter `mint_credential` during eligibility verification.
   - Result: Reentrancy detected and blocked; credential counter remains 0.
4. **`test_reentrancy_prevented_state_consistent_and_no_funds_lost`**:
   - Verifies state invariance and fund security under failed reentrancy attempts.
5. **`test_reentrancy_during_transfer`**:
   - Confirms transfer protection against unauthorized reentrant invocation.

**Test Execution**:
```bash
cargo test --test security_reentrancy_tests
# Result: 5 passed; 0 failed; 0 ignored; finished in 1.75s
```
