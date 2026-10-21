# Plan Validity Check — Issue #551 RBAC

Checked against actual file contents on disk. Files read:
- `contracts/credential-nft/src/metadata.rs`
- `contracts/credential-nft/src/events.rs`
- `contracts/credential-nft/src/lib.rs`
- `contracts/progress-tracker/src/types.rs`
- `contracts/progress-tracker/src/events.rs`
- `contracts/progress-tracker/src/lib.rs`
- `contracts/learn-token/src/lib.rs`
- `contracts/learn-token/src/storage.rs`

---

## 1. Overall Verdict

**MINOR AMENDMENTS**

Most of the RBAC work for `credential-nft` is already done. The plan's "current state" assumptions for `credential-nft` are stale — the code has moved ahead of what the plan describes. The plan's assumptions for `progress-tracker` are mostly correct, with one exception (an `upgrade` function already exists). The plan's items 1 and 2 are largely redundant for `credential-nft`. Items 3–9 are still needed for `progress-tracker` and for tests; the corrected checklist below reflects the real remaining work.

---

## 2. Discrepancies Found

### Discrepancy A — Plan items 1 & 2: credential-nft RBAC types and wiring are already done

**Plan assumption (item 1):**
> `metadata.rs` has no RBAC. Needs `AdminRole`, `RoleKey`, `Role(RoleKey)` storage key, and `has_role`/`grant_role`/`revoke_role` helpers added. `events.rs` needs `role_granted`/`role_revoked`. `lib.rs` needs `NotAuthorized = 2` and `panic_with_error` import.

**Plan assumption (item 2):**
> `mint_credential` currently requires `admin.require_auth()`. `pause`/`unpause` use `(env: Env)` signature. `upgrade` has no `caller`. No public `grant_role`/`revoke_role`/`has_role` exist.

**Actual code:**
- `contracts/credential-nft/src/metadata.rs` already defines `AdminRole { Admin, Minter, Pauser }`, `RoleKey`, `CredentialDataKey::Role(RoleKey)`, and full `has_role`/`grant_role`/`revoke_role` implementations. Every helper described in item 1 is present.
- `contracts/credential-nft/src/lib.rs` already has `NotAuthorized = 2` in `ContractError`, `panic_with_error` in the import, public `grant_role`/`revoke_role`/`has_role` functions, `mint_credential(env, caller, to, course_id, score, metadata_uri)` gated on `Minter` role, `pause(env, caller)` / `unpause(env, caller)` gated on `Pauser` role, `upgrade(env, caller, new_wasm_hash)` gated on `Admin` role, and `initialize` already calls `metadata::grant_role(&env, &admin, &AdminRole::Admin)`.

**What still needs doing in credential-nft:**
- `contracts/credential-nft/src/events.rs` does **not** yet have `role_granted` or `role_revoked` event functions. The plan calls for adding them, and the existing `grant_role`/`revoke_role` functions in `lib.rs` already reference `events::role_granted` and `events::role_revoked` (so the contract would not compile without them — but see Discrepancy B below).
- The existing test module in `lib.rs` contains a mix of tests that correctly use the new signature (e.g. `test_pause_emits_paused_event` calls `client.pause(&admin)`) and tests near the bottom that still call `client.mint_credential(&learner, ...)` without the `&admin` caller argument (e.g. `test_mint_rejects_counter_overflow`, `test_minted_event_includes_metadata_uri`). These need updating (plan item 5 still applies to those).

**Corrected instruction for item 1:**
Skip the `metadata.rs` and `lib.rs` parts of item 1 — they are done. The only remaining sub-task from item 1 is adding `role_granted` and `role_revoked` to `contracts/credential-nft/src/events.rs`. The `grant_role` / `revoke_role` / `has_role` public functions in `lib.rs` should **not** call those event functions if the events don't exist yet (see Discrepancy B).

**Corrected instruction for item 2:**
Skip entirely. All wiring described in item 2 is already present in `lib.rs`.

---

### Discrepancy B — credential-nft `grant_role`/`revoke_role` call non-existent event functions

**Plan assumption (item 2):**
> The public `grant_role`/`revoke_role` functions should call `events::role_granted` / `events::role_revoked`.

**Actual code (lib.rs, already written):**
```rust
pub fn grant_role(env: Env, caller: Address, role: AdminRole, address: Address) {
    ...
    metadata::grant_role(&env, &address, &role);
    events::role_granted(&env, &address, &role);   // ← calls events not yet defined
}
pub fn revoke_role(env: Env, caller: Address, role: AdminRole, address: Address) {
    ...
    metadata::revoke_role(&env, &address, &role);
    events::role_revoked(&env, &address, &role);   // ← calls events not yet defined
}
```

The event functions `role_granted` and `role_revoked` do not exist in `contracts/credential-nft/src/events.rs`. This means the crate currently does **not** compile cleanly. Adding those two functions to `events.rs` is the only remaining step for credential-nft's storage/wiring layer.

**Corrected instruction:** Item 1 reduces to: add `role_granted(env, address, role)` and `role_revoked(env, address, role)` to `contracts/credential-nft/src/events.rs` with the shapes described in the plan. No other change is needed to `metadata.rs` or `lib.rs`.

---

### Discrepancy C — progress-tracker `upgrade` already exists (plan item 4, sub-step 4)

**Plan assumption (item 4, sub-step 4):**
> Add a new `upgrade` function — progress-tracker has no upgrade entrypoint today.

**Actual code:**
`contracts/progress-tracker/src/lib.rs` already contains:
```rust
pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
    Self::require_not_paused(&env);
    let admin: Address = env.storage().persistent().get(&ProgressTrackerDataKey::Admin)...;
    admin.require_auth();
    env.deployer().update_current_contract_wasm(new_wasm_hash.clone());
    types::write_entry(&env, &ProgressTrackerDataKey::WasmHash, &new_wasm_hash);
    // bumps Version and calls events::upgraded(...)
}
```
The `WasmHash` key also already exists in `ProgressTrackerDataKey`. The `events::upgraded` function already exists in `events.rs`.

**Corrected instruction for item 4, sub-step 4:** Do **not** add a new `upgrade` function. Instead, change the existing one's signature from `pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>)` to `pub fn upgrade(env: Env, caller: Address, new_wasm_hash: BytesN<32>)`, replace the `admin.require_auth()` block with `caller.require_auth()` + `has_role(caller, Admin)` check, and update the event call to match. The body logic (deployer call, WasmHash write, Version bump, events::upgraded) is already correct and should be kept as-is.

---

### Discrepancy D — progress-tracker `pause`/`unpause` are still old-style (plan item 4 is still needed)

**Plan assumption (item 4, sub-steps 2–3):**
> `pause(env: Env)` and `unpause(env: Env)` use `admin.require_auth()` and need updating to `(env, caller)` + Pauser check.

**Actual code:** Confirmed. `pause` and `unpause` still have no `caller` parameter and still read + auth against the raw admin address. Plan item 4 sub-steps 2 and 3 are accurate and still needed.

---

### Discrepancy E — progress-tracker `initialize` does not yet call `grant_role` (plan item 4, sub-step 1)

**Plan assumption (item 4, sub-step 1):**
> After `types::write_entry(&env, &ProgressTrackerDataKey::Admin, &admin)`, add `types::grant_role(&env, &admin, &types::AdminRole::Admin)`.

**Actual code:** Confirmed. `initialize` writes the `Admin` key but does not call `grant_role`. The `has_role` helper already has a backward-compat path (if address == admin, return true), so the contract will function correctly before this is added, but for consistency with `learn-token` and `credential-nft` the grant call should still be added.

---

### Discrepancy F — progress-tracker `events.rs` has no `role_granted`/`role_revoked` (plan item 3 is still needed)

**Plan assumption (item 3):**
> `events.rs` needs `role_granted` and `role_revoked`.

**Actual code:** Confirmed. `contracts/progress-tracker/src/events.rs` has `paused`, `unpaused`, `upgraded`, and all course/learner events, but no role events.

---

### Discrepancy G — progress-tracker `types.rs` RBAC is already done (plan item 3 partial)

**Plan assumption (item 3):**
> `types.rs` has no RBAC. Needs `AdminRole`, `RoleKey`, `Role(RoleKey)`, and `has_role`/`grant_role`/`revoke_role`.

**Actual code:** All of these are already present in `contracts/progress-tracker/src/types.rs`. `AdminRole { Admin, Minter, Pauser }`, `RoleKey`, `ProgressTrackerDataKey::Role(RoleKey)`, and `has_role`/`grant_role`/`revoke_role` functions are fully implemented.

**Corrected instruction for item 3:** The `types.rs` work is complete. Skip those sub-tasks. The remaining work for item 3 is: (a) add `role_granted`/`role_revoked` to `events.rs`, (b) add `NotAuthorized = 1` to `ContractError`, and (c) add `panic_with_error` to the `soroban_sdk` import in `lib.rs`.

---

### Discrepancy H — credential-nft test module has a mix of old and new call sites (plan item 5)

**Plan assumption (item 5):**
> All `mint_credential` call sites in the test module need `&admin` as the first arg.

**Actual code:** The tests already updated to match the new signature include `test_pause_emits_paused_event`, `test_unpause_emits_unpaused_event`, the double-pause/not-paused tests, `test_mint_and_verify`, `test_get_credentials_for`, and other tests near the top. However, tests deeper in the file (e.g., `test_mint_rejects_counter_overflow`, `test_minted_event_includes_metadata_uri`, and several others near the bottom of the test module) still call `client.mint_credential(&learner, ...)` without `&admin`. Plan item 5 still applies to those.

---

## 3. Final Ordered Task Checklist (Amended)

- [ ] 1. Add `role_granted` and `role_revoked` event functions to `contracts/credential-nft/src/events.rs`.

  **What**: The public `grant_role` and `revoke_role` functions in `lib.rs` already call `events::role_granted` and `events::role_revoked`, but those functions do not exist yet, so the crate does not compile. Add them now. Shape: topics `("role_granted"|"role_revoked", address.clone())`, data `(role_symbol,)` where role_str maps Admin→"Admin", Minter→"Minter", Pauser→"Pauser". The `AdminRole` type is in `crate::metadata`; import it or accept the type parameter. *(All `metadata.rs` and `lib.rs` work from the original plan item 1 and item 2 is already done — skip it.)*

  **Files**: `contracts/credential-nft/src/events.rs`

  **Verify**: `cargo check -p credential-nft` — compiles cleanly.

- [ ] 2. ~~Add RBAC types and storage helpers to `credential-nft/src/metadata.rs`~~ — ALREADY DONE. Skip.

- [ ] 3. ~~Update `credential-nft/src/lib.rs`: initialize, mint_credential, pause, unpause, upgrade, grant_role, revoke_role, has_role~~ — ALREADY DONE. Skip.

- [ ] 4. Add RBAC scaffolding to `progress-tracker/src/lib.rs` and `events.rs`; add `NotAuthorized` to `ContractError`. *(Corresponds to original plan item 3.)*

  **What**: The `types.rs` parts of item 3 are already done. What remains: (a) add `role_granted(env, address, role)` and `role_revoked(env, address, role)` to `contracts/progress-tracker/src/events.rs` using the same shape as in `credential-nft` — topics `("role_granted"|"role_revoked", address.clone())`, data `(Symbol::new(env, role_str),)`, importing `AdminRole` from `crate::types`; (b) add `NotAuthorized = 1` to `ContractError` in `lib.rs`; (c) add `panic_with_error` to the `soroban_sdk` import in `lib.rs`.

  **Files**: `contracts/progress-tracker/src/events.rs`, `contracts/progress-tracker/src/lib.rs`

  **Verify**: `cargo check -p progress-tracker` — compiles cleanly.

- [ ] 5. Update `progress-tracker/src/lib.rs`: initialize, pause, unpause, upgrade, add public grant_role/revoke_role/has_role. *(Corresponds to original plan item 4.)*

  **What**: Wire RBAC into progress-tracker's entrypoints. All five sub-steps from the original plan item 4, with one amendment for sub-step 4:

  1. **`initialize`**: After `types::write_entry(&env, &ProgressTrackerDataKey::Admin, &admin)`, add `types::grant_role(&env, &admin, &types::AdminRole::Admin);`

  2. **`pause`**: Change to `pub fn pause(env: Env, caller: Address)`. Replace the admin-read + `admin.require_auth()` block with `caller.require_auth()` + Pauser check. Update event call to `events::paused(&env, &caller, env.ledger().timestamp())`.

  3. **`unpause`**: Same change as `pause`.

  4. **`upgrade`** (AMENDED): Do **not** add a new function — one already exists. Change the existing signature from `pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>)` to `pub fn upgrade(env: Env, caller: Address, new_wasm_hash: BytesN<32>)`. Replace the `admin.require_auth()` block with `caller.require_auth()` + `if !types::has_role(&env, &caller, &types::AdminRole::Admin) { panic_with_error!(...) }`. Leave the deployer call, WasmHash write, Version bump, and `events::upgraded` call unchanged.

  5. **Add public RBAC entrypoints**: `grant_role`, `revoke_role`, `has_role` as described in the original plan. Each should also emit the corresponding `events::role_granted` / `events::role_revoked` event.

  **Files**: `contracts/progress-tracker/src/lib.rs`

  **Verify**: `cargo check -p progress-tracker` — compiles. Some test call sites will have wrong argument counts (fixed in item 6).

- [ ] 6. Fix remaining stale test call sites in `credential-nft/src/lib.rs` test module. *(Corresponds to original plan item 5, scope narrowed.)*

  **What**: The tests already updated to the new signature can be left alone. Find and fix the tests that still call `client.mint_credential` without `&admin` as the first argument. Known stale tests include at minimum: `test_mint_rejects_counter_overflow` (calls `client.mint_credential(&learner, ...)`) and `test_minted_event_includes_metadata_uri` (same). Audit the rest of the test module for any other old-style calls to `mint_credential`, `pause`, or `unpause` without a `caller` argument. For `pause`/`unpause`, all tests that call `client.pause(&admin)` are already correct; confirm none call `client.pause()` (no arg).

  Also audit workspace integration tests under `tests/` for credential-nft call sites:
  - `tests/integration/credential_flow.rs`
  - `tests/integration/full_flow.rs`
  - `tests/integration/security_auth_tests.rs`
  - `tests/integration/upgrade_tests.rs`
  - `tests/integration/pause_event_consistency.rs`
  - `tests/integration/pause_flow.rs`
  - `tests/unit/credential_tests.rs`

  **Files**: `contracts/credential-nft/src/lib.rs` (test module), affected files under `tests/`

  **Verify**: `cargo test -p credential-nft` — no compile errors or test failures.

- [ ] 7. Fix existing tests in `progress-tracker/src/lib.rs` and workspace integration tests for updated progress-tracker signatures. *(Corresponds to original plan item 6.)*

  **What**: `pause`/`unpause`/`upgrade` signatures change. Update all call sites:
  - `client.pause()` → `client.pause(&admin)`
  - `client.unpause()` → `client.unpause(&admin)`
  - `client.upgrade(hash)` → `client.upgrade(&admin, &hash)`

  Check in: `contracts/progress-tracker/src/lib.rs` test module (if any), and all files under `tests/` that call progress-tracker's pause/unpause/upgrade.

  **Files**: `contracts/progress-tracker/src/lib.rs` (test module), affected files under `tests/`

  **Verify**: `cargo test -p progress-tracker` — no compile errors or test failures.

- [ ] 8. Add RBAC tests to `credential-nft/src/lib.rs` test module. *(Original plan item 7, unchanged.)*

  **What**: Add the test cases listed in plan item 7. All helpers (`setup_contract`, `enrolled_and_completed`, etc.) already exist. Use `env.mock_all_auths()`. Test cases: `test_has_role_returns_false_before_grant`, `test_grant_role_minter_enables_has_role`, `test_revoke_role_removes_has_role`, `test_admin_always_has_all_roles`, `test_admin_can_grant_minter_who_can_mint`, `test_admin_role_holder_can_grant_roles`, `test_mint_requires_minter_or_admin_role`, `test_admin_can_mint_without_explicit_minter_grant`, `test_pause_requires_pauser_role`, `test_unpause_requires_pauser_role`, `test_pauser_can_pause_and_unpause`, `test_admin_can_pause_without_explicit_pauser_grant`, `test_upgrade_requires_admin_role`, `test_non_admin_cannot_grant_role`, `test_non_admin_cannot_revoke_role`.

  **Files**: `contracts/credential-nft/src/lib.rs`

  **Verify**: `cargo test -p credential-nft -- --test-output immediate 2>&1 | tail -20` — all new RBAC tests pass.

- [ ] 9. Add RBAC tests to `progress-tracker/src/lib.rs` test module. *(Original plan item 8, unchanged.)*

  **What**: A `#[cfg(test)]` module already exists in `contracts/progress-tracker/src/lib.rs`. Add the test cases listed in plan item 8: `test_has_role_returns_false_before_grant`, `test_grant_role_enables_has_role`, `test_revoke_role_removes_has_role`, `test_admin_has_all_roles_by_default`, `test_pause_requires_pauser_role`, `test_unpause_requires_pauser_role`, `test_pauser_can_pause_and_unpause`, `test_admin_can_pause_without_explicit_pauser_grant`, `test_upgrade_requires_admin_role`, `test_non_admin_cannot_grant_role`, `test_non_admin_cannot_revoke_role`, `test_admin_role_holder_can_grant_roles`.

  **Files**: `contracts/progress-tracker/src/lib.rs`

  **Verify**: `cargo test -p progress-tracker 2>&1 | tail -20` — all new tests pass.

- [ ] 10. Full workspace build and test pass; clippy and format check. *(Original plan item 9, unchanged.)*

  **What**: Final verification that the entire workspace compiles, all tests pass, no new clippy warnings, and formatting is consistent.

  Run in order:
  1. `cargo fmt --all`
  2. `cargo clippy --workspace --all-targets -- -D warnings`
  3. `cargo test --workspace`

  **Files**: none (verification only)

  **Verify**: `cargo test --workspace` exits with code 0 and reports zero failures.

---

## Summary of Actual Remaining Work

| File | Status | Remaining work |
|------|--------|---------------|
| `contracts/credential-nft/src/metadata.rs` | **DONE** | Nothing |
| `contracts/credential-nft/src/events.rs` | **PARTIAL** | Add `role_granted`, `role_revoked` |
| `contracts/credential-nft/src/lib.rs` | **DONE** (wiring) | Fix ~2 stale test call sites; add RBAC tests |
| `contracts/progress-tracker/src/types.rs` | **DONE** | Nothing |
| `contracts/progress-tracker/src/events.rs` | **PARTIAL** | Add `role_granted`, `role_revoked` |
| `contracts/progress-tracker/src/lib.rs` | **NOT DONE** | Add `NotAuthorized`, `panic_with_error`; update `initialize`, `pause`, `unpause`, `upgrade`; add `grant_role`/`revoke_role`/`has_role`; fix stale tests; add RBAC tests |
| `tests/**` | **PARTIAL** | Update credential-nft stale mint call sites; update progress-tracker pause/unpause/upgrade call sites |
