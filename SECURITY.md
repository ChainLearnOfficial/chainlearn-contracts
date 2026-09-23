# Security Audit Checklist

This document provides a structured security audit checklist for reviewing the ChainLearn Soroban smart contracts. It covers the three contracts (`learn-token`, `credential-nft`, `progress-tracker`) and the shared utilities package.

## 1. Authorization & Access Control

### 1.1 Authentication Checks

- [ ] Every state-changing function that requires auth calls `require_auth()` on the correct address
- [ ] `require_auth()` is called BEFORE any state reads or writes (first line after pause checks)
- [ ] Cross-contract calls do not bypass authorization requirements
- [ ] Admin-only functions verify the caller holds the required role via `has_role()`
- [ ] Multi-sig operations require both signers and verify they are distinct addresses

### 1.2 Role-Based Access Control

- [ ] Role assignments are stored correctly and checked consistently
- [ ] The admin role grants all permissions (Admin implies Minter and Pauser)
- [ ] Role revocation takes effect immediately and cannot be bypassed
- [ ] The initial admin is correctly registered at `initialize()` time
- [ ] `add_admin` / `remove_admin` require Admin role authorization

### 1.3 Initialization Guards

- [ ] All three contracts reject double-initialization (`AlreadyInitialized` error)
- [ ] `initialize()` can only be called once (checked via persistent storage key existence)
- [ ] Initialization sets all required state (admin, progress-tracker link, metadata)
- [ ] `is_initialized()` accurately reflects the initialization state

## 2. Arithmetic & Overflow Protection

- [ ] All arithmetic uses checked or saturating operations where overflow is possible
- [ ] Token supply never exceeds `max_supply` (checked on every mint path: `mint`, `claim_reward`, batch variants)
- [ ] Reward amounts are bounded: `score * BASE_REWARD_PER_POINT <= MAX_REWARD_AMOUNT`
- [ ] Credential ID counter uses `checked_add()` to prevent u64 overflow
- [ ] Progress percentages use integer division with no rounding that could exceed 100
- [ ] `total_quiz_score` accumulation cannot overflow (bounded by quiz count * MAX_QUIZ_SCORE)
- [ ] `modules_completed_bitmap` uses u64 and rejects module indices >= 64
- [ ] `set_max_supply` prevents increases beyond 2x the current cap (governance safeguard)
- [ ] `set_max_supply` prevents decreasing below current total supply

## 3. Reentrancy Prevention

- [ ] Cross-contract calls are read-only (progress-tracker queries only, no state mutations via callbacks)
- [ ] `claim_reward` resolves progress-tracker address once and reuses it across batch iterations
- [ ] `batch_complete_module` is atomic: all modules validated before any storage writes
- [ ] `batch_submit_quiz_score` uses per-quiz skip logic but writes progress once at the end
- [ ] No external contract calls exist in the token transfer or burn paths
- [ ] State updates happen after validation but before event emission

## 4. Storage Safety

### 4.1 Data Integrity

- [ ] No duplicate storage keys for the same logical entity
- [ ] Reward claims are tracked per (learner, course_id, quiz_id) to prevent double-claiming
- [ ] Quiz results are stored once under their own key, not duplicated into ProgressInfo
- [ ] Credential uniqueness is enforced per (learner, course_id) pair
- [ ] Module completion uses a bitmap indexed by position in the course's module list
- [ ] Sequential module ordering is enforced (module N requires module N-1 completed)

### 4.2 Storage Economics

- [ ] Allowances use temporary storage (not persistent) since they are short-lived
- [ ] Allowance TTL is extended to cover the expiration ledger
- [ ] Persistent entries extend TTL when created (`PERSISTENT_TTL_THRESHOLD` / `PERSISTENT_TTL_EXTEND_TO`)
- [ ] Storage entry counts are tracked for cost monitoring (`get_storage_size`)
- [ ] Singleton config values are not counted in storage size tracking
- [ ] Expired allowances can be pruned via `prune_expired_allowance` and `cleanup_expired_allowances`

### 4.3 Revocation Cleanup

- [ ] Revoking a credential removes it from the learner's credential list
- [ ] Revoking a credential removes it from the course credential index
- [ ] The `Revoked` flag is stored in its own key for efficient validity checks
- [ ] Revocation reasons are stored separately and retrievable

## 5. Cross-Contract Call Safety

- [ ] Progress-tracker address is resolved once per batch operation and reused
- [ ] Cross-contract calls only invoke read-only functions (`get_quiz_score`, `get_course_score`, `is_eligible_for_credential`, `course_exists`)
- [ ] No cross-contract calls in the allowance, transfer, or burn paths
- [ ] The progress-tracker address can be updated by admin when the tracker is upgraded (`set_progress_tracker`)
- [ ] `initialize.sh` verifies the wiring by reading back the stored progress-tracker address

## 6. Token Economics

- [ ] Reward formula is consistent: `score * BASE_REWARD_PER_POINT` (100 tokens per point)
- [ ] Maximum reward per quiz is capped at `MAX_QUIZ_SCORE * BASE_REWARD_PER_POINT` (10,000)
- [ ] Total supply cap is enforced on every mint path (admin mint, reward claim, batch variants)
- [ ] Burning reduces total supply (circulating supply stays accurate)
- [ ] `total_minted_to` tracks cumulative minting independently of balance changes
- [ ] Transfer restrictions (WhitelistOnly, Cooldown, MaxAmount) are checked on every transfer
- [ ] Transfers to the contract address are rejected (prevents token locking)
- [ ] Self-transfers (from == to) are no-ops

## 7. Credential & NFT Security

- [ ] Score verification: minted score must exactly match progress-tracker's `get_course_score`
- [ ] Eligibility gate: `is_eligible_for_credential` is checked before minting
- [ ] Course existence is verified before minting (clear error message)
- [ ] Minimum score threshold (50) is enforced
- [ ] One credential per learner per course is enforced
- [ ] Credentials are soulbound: `transfer()` always returns `Err(Soulbound)`
- [ ] Metadata URI validation: non-empty, >= 8 chars, valid scheme (ipfs, http, https, cert)
- [ ] Credential expiration can be set and checked (0 = no expiration)
- [ ] Revoked credentials cannot be renewed

## 8. Admin Transfer Safety

- [ ] Admin transfer is delayed (default 48 hours) to allow cancellation
- [ ] The pending admin must call `accept_admin` to complete the transfer
- [ ] Current admin can call `cancel_admin_transfer` at any time before acceptance
- [ ] The `admin_transfer_initiated` event is emitted for monitoring
- [ ] Zero address cannot be set as the new admin
- [ ] Multiple `transfer_admin` calls overwrite the previous pending transfer

## 9. Emergency Controls

- [ ] Pause prevents all state-changing operations (transfer, mint, burn, claim, enroll, etc.)
- [ ] Pause is checked at the top of every state-changing function
- [ ] Read-only functions work normally when paused
- [ ] Only Pauser or Admin roles can pause/unpause
- [ ] Double-pause and unpause-when-not-paused are rejected with clear errors
- [ ] Events are emitted on pause/unpause for monitoring

## 10. Upgrade Safety

- [ ] `upgrade()` requires admin authorization
- [ ] `upgrade_multisig()` requires two distinct admin signers
- [ ] State is preserved across upgrades (Soroban replaces only executable code)
- [ ] The new WASM hash is stored for audit trail
- [ ] Upgrade version counter is incremented on each upgrade
- [ ] `wasm_hash()` and `upgrade_version()` are readable for verification

## 11. Governance Security

- [ ] Proposals have a defined voting window (start_time, end_time)
- [ ] Voting power is based on token balance at snapshot height
- [ ] Double-voting is prevented (one vote per address per proposal)
- [ ] Proposals can only be executed once
- [ ] Only admin can create proposals

## 12. Vesting Security

- [ ] Cliff period is enforced before any tokens can be claimed
- [ ] Linear vesting calculates correct pro-rata amounts
- [ ] Claimed amounts are tracked to prevent over-claiming
- [ ] Fully claimed (exhausted) schedules cannot claim more
- [ ] Only admin can create vesting schedules

## 13. Event Integrity

- [ ] All state-changing operations emit appropriate events
- [ ] Events include indexed topics for efficient server-side filtering
- [ ] Event data contains all fields needed to reconstruct the operation
- [ ] `credential_revoked` events include learner, course_id, credential_id, and admin
- [ ] `reward` events include learner, course_id, quiz_id, score, and amount
- [ ] `credential_eligible` is emitted once per (learner, course) when eligibility first flips

## 14. Edge Cases & Error Handling

- [ ] Zero-amount transfers, mints, and burns are handled correctly
- [ ] Unknown course/module/quiz IDs produce clear panic messages
- [ ] Paginated queries (`get_credentials_for`) clamp limits and reject zero/oversized pages
- [ ] Empty batch operations (empty quiz_ids, empty module_ids) are no-ops
- [ ] Concurrent enrollment attempts are rejected ("already enrolled")
- [ ] Quiz retake requires the new score to be strictly higher

## 15. Code Quality

- [ ] No hardcoded addresses or private keys in contract code
- [ ] All public functions have doc comments describing parameters and return values
- [ ] Constants are defined in the shared package and used consistently
- [ ] No `unsafe` code blocks
- [ ] `#[no_std]` is used (no standard library dependencies)
- [ ] Test coverage exists for all critical paths including error conditions

## Audit Scope

| Contract | Files | Lines |
|---|---|---|
| `learn-token` | `lib.rs`, `storage.rs`, `events.rs` | ~1400 |
| `credential-nft` | `lib.rs`, `mint.rs`, `verify.rs`, `metadata.rs`, `xcall.rs` | ~750 |
| `progress-tracker` | `lib.rs`, `types.rs`, `rewards.rs` | ~1500 |
| `shared` | `constants.rs`, `lib.rs` | ~60 |
| **Total** | | **~3710** |

## Known Limitations

1. **No on-chain key enumeration**: Soroban has no API to list storage keys, so the contracts maintain their own counters (`get_storage_size`) and spender registries for cleanup.
2. **Integer division rounding**: Progress calculations use integer division, which floors results. This is by design and documented.
3. **Sequential module ordering**: Modules must be completed in the order they appear in the course's `module_ids` list. This is enforced by checking the predecessor in the bitmap.
4. **No automatic expiration**: Credentials and allowances do not expire by default unless explicitly set. Expired allowances must be pruned manually or by indexers.
