# Cross-Contract Reentrancy Protection Audit

Audit of cross-contract calls, reentrancy vulnerabilities, state safety, and attack resilience across ChainLearn smart contracts (#354).

## Executive Summary

Reentrancy vulnerabilities occur when external contract calls interrupt the execution flow of a calling contract, allowing malicious state re-entry before initial invariants and storage updates have finalized. In the Soroban VM, contracts execute in an isolated host environment with transactional atomicity. However, application-level reentrancy across interdependent contracts (e.g., token minting, reward claims, credential issuance) must strictly adhere to the **Checks-Effects-Interactions (CEI)** pattern and state locking to prevent re-entrant exploitation.

This audit reviews all cross-contract call sites across `learn-token`, `progress-tracker`, and `credential-nft`, maps their threat profile, verifies reentrancy defenses, and documents attack test coverage.

---

## Scope & Inventory of Cross-Contract Calls

The ChainLearn protocol consists of three primary contracts:
1. `contracts/progress-tracker`: Core state machine for course progression and quiz completions (acts as a leaf contract / source of truth).
2. `contracts/learn-token`: Educational utility token with quiz rewards minting and transfers.
3. `contracts/credential-nft`: Soulbound certificate/NFT contract verifying course completion.

### Cross-Contract Call Inventory

| Origin Contract | Target Contract | Target Method | Call Purpose | Reentrancy Risk Level |
|---|---|---|---|:---:|
| `learn-token` | `progress-tracker` | `get_quiz_score` | Fetch validated score in `claim_reward` | Low (Read-only call, checked prior to state change) |
| `credential-nft` | `progress-tracker` | `is_eligible_for_credential` | Verify course completion before minting | Low (Read-only call, CEI compliant) |
| `credential-nft` | `progress-tracker` | `get_course_score` | Verify reported score matches tracked score | Low (Read-only call, CEI compliant) |
| `progress-tracker` | *None* | *None* | Leaf node: does not make external contract invocations | None |

---

## Detailed Vulnerability & Threat Assessment

### 1. `learn-token::claim_reward`
- **Call flow**: Learner calls `claim_reward(learner, course_id, quiz_id)`. `learn-token` makes a cross-contract query to `progress-tracker.get_quiz_score(learner, course_id, quiz_id)`.
- **Potential Risk**: If a caller could re-enter `claim_reward` before `RewardClaimed` state is committed, tokens could be double-minted.
- **Protection Architecture**:
  - `storage::has_reward_claimed(&env, &learner, &course_id, &quiz_id)` is evaluated immediately.
  - The cross-contract call to `progress-tracker` is strictly read-only and invokes static contract client methods (`ProgressTrackerClient`).
  - `storage::set_reward_claimed` is recorded upon successful validation.
  - Transaction rollbacks revert any intermediate state on failure.

### 2. `learn-token::transfer` & `transfer_from`
- **Call flow**: Direct balance updates.
- **Potential Risk**: Reentrant calls during recipient callback or custom hook execution.
- **Protection Architecture**:
  - Soroban token transfers do not trigger fallback code or arbitrary external hooks on recipient accounts.
  - Balances are updated synchronously and atomically using checked balance updates (`from_balance - amount`, `to_balance + amount`).
  - Transfers to the contract address itself are explicitly disallowed (`cannot transfer to contract`).

### 3. `credential-nft::mint_credential`
- **Call flow**: Minter invokes `mint_credential`, triggering cross-contract verification against `progress-tracker`.
- **Potential Risk**: Reentrancy allowing duplicate credential minting for a single course completion.
- **Protection Architecture**:
  - Strict duplicate check: `storage::has_credential(&env, &learner, &course_id)`.
  - Eligibility verification via `progress_tracker.is_eligible_for_credential`.
  - Credentials are non-transferable (soulbound) and indexed per `(learner, course_id)`.

---

## Attack Scenarios & Test Suite Validation

The test suite in `tests/integration/security_reentrancy_tests.rs` explicitly tests adversarial reentrancy scenarios:

1. **`test_reentrancy_during_transfer`**:
   - Asserts that re-entrant invocation during token transfers triggers panic and reverts state.
2. **`test_reentrancy_prevented_state_consistent_and_no_funds_lost`**:
   - Deploys a malicious attack contract (`MaliciousContract`) attempting reentrant `mint` operations during state transitions.
   - Verifies that reentrancy is caught and all balances/total supplies remain perfectly consistent without funds loss.
3. **`test_reentrant_claim_reward_attack_rejected`**:
   - Asserts that recursive reentrancy attempts against `claim_reward` fail safely without compromising token supply invariants.

---

## Acceptance Criteria Checklist

- [x] **All calls are reviewed**: Every cross-contract interaction across all crates cataloged and verified.
- [x] **Risks are identified**: CEI pattern, cross-contract callback behavior, and state mutation risks evaluated.
- [x] **Protection is added**: Verified single-claim assertions, balance checks, and atomic state updates.
- [x] **Attacks are tested**: Attack scenarios simulated via `tests/integration/security_reentrancy_tests.rs`.
