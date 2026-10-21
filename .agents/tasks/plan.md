# Implementation Plan â€” Issue #551: Port RBAC to credential-nft and progress-tracker

## Background (grounded in code read)

**Reference implementation** (`learn-token`):
- `contracts/learn-token/src/storage.rs` defines `AdminRole { Admin, Minter, Pauser }`, `RoleKey { address, role }`, and `TokenDataKey::Role(RoleKey)` as the persistent storage key. `has_role` returns true for the primary admin (backward compat), anyone with `AdminRole::Admin`, or an explicit `Role(RoleKey)` entry. `grant_role` / `revoke_role` set/remove `Role(RoleKey) â†’ bool` in persistent storage.
- `contracts/learn-token/src/lib.rs` exposes `grant_role(caller, address, role)` / `revoke_role(caller, address, role)` / `has_role(address, role)` as public contract functions. `grant_role`/`revoke_role` require `caller.require_auth()` + `has_role(caller, Admin)`, panicking with `ContractError::NotAuthorized` on failure. `pause(caller)` / `unpause(caller)` require `has_role(caller, Pauser)`. `mint(caller, to, amount)` requires `has_role(caller, Minter)`.
- `contracts/learn-token/src/events.rs` defines `role_granted(address, role)` and `role_revoked(address, role)` with topics `("role_granted"|"role_revoked", address)` and data `(role_symbol,)`.

**credential-nft current state**:
- `metadata.rs`: `CredentialDataKey` enum is the storage key type. `write_entry`/`remove_entry` track the storage-size counter â€” all persistent writes must go through them. No RBAC exists.
- `lib.rs`: `initialize(admin, progress_tracker)`. `mint_credential` currently requires both `admin.require_auth()` AND `to.require_auth()`. `pause`/`unpause` and `upgrade` require only `admin.require_auth()`. `ContractError` has `AlreadyInitialized = 0` and `Soulbound = 1`. No `panic_with_error!` import yet.
- `events.rs` already has `paused`/`unpaused` helpers.

**progress-tracker current state**:
- `types.rs`: `ProgressTrackerDataKey` enum is the key type. `write_entry`/`remove_entry` for storage-size tracking. No RBAC.
- `lib.rs`: `initialize(admin)`. `pause`/`unpause` use `admin.require_auth()`. No `upgrade` function exists (only an upgrade counter). All other admin-only functions use a raw admin check. `ContractError` has only `AlreadyInitialized = 0`. Imports already include `BytesN`.
- `events.rs` already has `paused`/`unpaused` helpers.

**Key design decisions**:
1. credential-nft `pause`/`unpause` signatures change from `(env: Env)` to `(env: Env, caller: Address)` â€” matching learn-token's pattern. This is a breaking API change but required to identify who is authorizing.
2. credential-nft `mint_credential` gains a `caller: Address` as the first argument (before `to`). `to.require_auth()` stays â€” the learner still authorizes.
3. progress-tracker `pause`/`unpause` signatures change from `(env: Env)` to `(env: Env, caller: Address)`.
4. progress-tracker gets a new `upgrade(caller, new_wasm_hash)` function gated on Admin role, matching the existing upgrade pattern in credential-nft.
5. credential-nft `upgrade` gains a `caller: Address` first parameter to match.
6. `NotAuthorized` error code: `2` in credential-nft (after Soulbound=1), `1` in progress-tracker (after AlreadyInitialized=0).
7. Role storage writes go through `write_entry`/`remove_entry` in each contract's respective helper module to keep the storage-size counter accurate.

**Build and test commands** (from CONTRIBUTING.md):
- Check: `cargo check --workspace`
- Test: `cargo test --workspace`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Format check: `cargo fmt --all -- --check`

---

## Plan

- [ ] 1. Add RBAC types and storage helpers to `credential-nft/src/metadata.rs`; add role events to `credential-nft/src/events.rs`; add `NotAuthorized` to `ContractError`.

  **What**: Port AdminRole enum, RoleKey struct, Role(RoleKey) storage key, and has_role/grant_role/revoke_role helpers into the credential-nft module. Add matching role events. Add NotAuthorized error variant.

  **`contracts/credential-nft/src/metadata.rs`**:
  - Add after `pub struct PendingAdminTransfer { ... }`:
    ```rust
    #[contracttype]
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum AdminRole { Admin, Minter, Pauser }

    #[contracttype]
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct RoleKey { pub address: Address, pub role: AdminRole }
    ```
  - Add `Role(RoleKey)` variant to `CredentialDataKey` (add after `AdminTransferDelay`).
  - Add storage helpers after `remove_entry`:
    - `pub fn has_role(env: &Env, address: &Address, role: &AdminRole) -> bool`: reads `CredentialDataKey::Admin`; returns true if `address == admin`. If `role != &AdminRole::Admin`, also checks `Role(RoleKey { address, role: AdminRole::Admin })`. Otherwise checks `Role(RoleKey { address, role })`.
    - `pub fn grant_role(env: &Env, address: &Address, role: &AdminRole)`: calls `write_entry(env, &CredentialDataKey::Role(RoleKey { address: address.clone(), role: role.clone() }), &true)`.
    - `pub fn revoke_role(env: &Env, address: &Address, role: &AdminRole)`: calls `remove_entry(env, &CredentialDataKey::Role(RoleKey { address: address.clone(), role: role.clone() }))`.

  **`contracts/credential-nft/src/events.rs`**:
  - Add `role_granted(env, address, role)` and `role_revoked(env, address, role)` functions. Topics: `("role_granted"|"role_revoked", address.clone())`. Data: `(Symbol::new(env, role_str),)` where role_str maps Admin->"Admin", Minter->"Minter", Pauser->"Pauser".

  **`contracts/credential-nft/src/lib.rs`**:
  - Add `NotAuthorized = 2` to `ContractError` enum (after `Soulbound = 1`).
  - Add `panic_with_error` to the `soroban_sdk` import list.

  **Files**: `contracts/credential-nft/src/metadata.rs`, `contracts/credential-nft/src/events.rs`, `contracts/credential-nft/src/lib.rs`

  **Verify**: `cargo check -p credential-nft` — compiles cleanly.

- [ ] 2. Update `credential-nft/src/lib.rs`: update `initialize`, re-gate `mint_credential`/`pause`/`unpause`/`upgrade`, add public `grant_role`/`revoke_role`/`has_role` functions.

  **What**: Wire RBAC into credential-nft's contract entrypoints. Depends on item 1.

  **`contracts/credential-nft/src/lib.rs`**:

  1. **`initialize`**: After `metadata::write_entry(&env, &CredentialDataKey::Admin, &admin)`, add:
     `metadata::grant_role(&env, &admin, &metadata::AdminRole::Admin);`

  2. **`mint_credential`**: Change signature to add `caller: Address` as the first argument (before `to: Address`):
     `pub fn mint_credential(env: Env, caller: Address, to: Address, course_id: Symbol, score: u32, metadata_uri: Symbol) -> u64`
     Replace the admin block (reading admin + `admin.require_auth()`) with:
     ```rust
     caller.require_auth();
     if !metadata::has_role(&env, &caller, &metadata::AdminRole::Minter) {
         panic_with_error!(&env, ContractError::NotAuthorized);
     }
     ```
     Keep `to.require_auth()` unchanged.

  3. **`pause`**: Change to `pub fn pause(env: Env, caller: Address)`. Replace body's admin-auth block with:
     ```rust
     caller.require_auth();
     if !metadata::has_role(&env, &caller, &metadata::AdminRole::Pauser) {
         panic_with_error!(&env, ContractError::NotAuthorized);
     }
     ```
     Update the event call from `events::paused(&env, &admin, ...)` to `events::paused(&env, &caller, ...)`.

  4. **`unpause`**: Same change as `pause` — add `caller: Address`, replace auth with Pauser check, update event call.

  5. **`upgrade`**: Change to `pub fn upgrade(env: Env, caller: Address, new_wasm_hash: BytesN<32>)`. Replace admin-auth block with:
     ```rust
     caller.require_auth();
     if !metadata::has_role(&env, &caller, &metadata::AdminRole::Admin) {
         panic_with_error!(&env, ContractError::NotAuthorized);
     }
     ```

  6. **Add public RBAC entrypoints** (add in a new `// -- Role Management` section):
     ```rust
     pub fn grant_role(env: Env, caller: Address, address: Address, role: metadata::AdminRole) {
         Self::require_not_paused(&env);
         caller.require_auth();
         if !metadata::has_role(&env, &caller, &metadata::AdminRole::Admin) {
             panic_with_error!(&env, ContractError::NotAuthorized);
         }
         metadata::grant_role(&env, &address, &role);
         events::role_granted(&env, &address, &role);
     }

     pub fn revoke_role(env: Env, caller: Address, address: Address, role: metadata::AdminRole) {
         Self::require_not_paused(&env);
         caller.require_auth();
         if !metadata::has_role(&env, &caller, &metadata::AdminRole::Admin) {
             panic_with_error!(&env, ContractError::NotAuthorized);
         }
         metadata::revoke_role(&env, &address, &role);
         events::role_revoked(&env, &address, &role);
     }

     pub fn has_role(env: Env, address: Address, role: metadata::AdminRole) -> bool {
         metadata::has_role(&env, &address, &role)
     }
     ```

  **Files**: `contracts/credential-nft/src/lib.rs`

  **Verify**: `cargo check -p credential-nft` — compiles. Some existing test call sites will now have wrong argument counts (they will be fixed in item 5).

- [ ] 3. Add RBAC types and storage helpers to `progress-tracker/src/types.rs`; add role events to `progress-tracker/src/events.rs`; add `NotAuthorized` to `ContractError`.

  **What**: Same port as item 1, but for progress-tracker. Independent of items 1 and 2.

  **`contracts/progress-tracker/src/types.rs`**:
  - Add after `pub struct PendingAdminTransfer { ... }` and the delay constants:
    ```rust
    #[contracttype]
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum AdminRole { Admin, Minter, Pauser }

    #[contracttype]
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct RoleKey { pub address: Address, pub role: AdminRole }
    ```
  - Add `Role(RoleKey)` variant to `ProgressTrackerDataKey` (add after `TagIndex(Symbol)`).
  - Add storage helpers after `remove_entry`:
    - `pub fn has_role(env: &Env, address: &Address, role: &AdminRole) -> bool`: reads `ProgressTrackerDataKey::Admin`; returns true if `address == admin`. If `role != &AdminRole::Admin`, checks `Role(RoleKey { address, role: AdminRole::Admin })`. Otherwise checks `Role(RoleKey { address, role })`.
    - `pub fn grant_role(env: &Env, address: &Address, role: &AdminRole)`: calls `write_entry(env, &ProgressTrackerDataKey::Role(RoleKey { address: address.clone(), role: role.clone() }), &true)`.
    - `pub fn revoke_role(env: &Env, address: &Address, role: &AdminRole)`: calls `remove_entry(env, &ProgressTrackerDataKey::Role(RoleKey { address: address.clone(), role: role.clone() }))`.

  **`contracts/progress-tracker/src/events.rs`**:
  - Add `role_granted(env, address, role)` and `role_revoked(env, address, role)` with the same shape as learn-token: topics `("role_granted"|"role_revoked", address.clone())`, data `(Symbol::new(env, role_str),)`.

  **`contracts/progress-tracker/src/lib.rs`**:
  - Add `NotAuthorized = 1` to `ContractError` enum.
  - Add `panic_with_error` to the `soroban_sdk` import (currently not imported).

  **Files**: `contracts/progress-tracker/src/types.rs`, `contracts/progress-tracker/src/events.rs`, `contracts/progress-tracker/src/lib.rs`

  **Verify**: `cargo check -p progress-tracker` — compiles cleanly.

- [ ] 4. Update `progress-tracker/src/lib.rs`: update `initialize`, re-gate `pause`/`unpause`, add `upgrade`, add public `grant_role`/`revoke_role`/`has_role` functions.

  **What**: Wire RBAC into progress-tracker's contract entrypoints. Depends on item 3.

  **`contracts/progress-tracker/src/lib.rs`**:

  1. **`initialize`**: After `types::write_entry(&env, &ProgressTrackerDataKey::Admin, &admin)`, add:
     `types::grant_role(&env, &admin, &types::AdminRole::Admin);`

  2. **`pause`**: Change to `pub fn pause(env: Env, caller: Address)`. Replace the admin-read + `admin.require_auth()` block with:
     ```rust
     caller.require_auth();
     if !types::has_role(&env, &caller, &types::AdminRole::Pauser) {
         panic_with_error!(&env, ContractError::NotAuthorized);
     }
     ```
     Update the event call: `events::paused(&env, &caller, env.ledger().timestamp())`.

  3. **`unpause`**: Same change — add `caller: Address`, replace auth with Pauser check, update event call.

  4. **Add `upgrade` function** (new — progress-tracker has no upgrade entrypoint today):
     ```rust
     pub fn upgrade(env: Env, caller: Address, new_wasm_hash: BytesN<32>) {
         Self::require_not_paused(&env);
         caller.require_auth();
         if !types::has_role(&env, &caller, &types::AdminRole::Admin) {
             panic_with_error!(&env, ContractError::NotAuthorized);
         }
         env.deployer().update_current_contract_wasm(new_wasm_hash.clone());
         types::write_entry(&env, &ProgressTrackerDataKey::WasmHash, &new_wasm_hash);
         let mut version: u32 = env
             .storage()
             .persistent()
             .get(&ProgressTrackerDataKey::Version)
             .unwrap_or(0);
         version += 1;
         types::write_entry(&env, &ProgressTrackerDataKey::Version, &version);
         env.events()
             .publish((Symbol::new(&env, "upgraded"),), (new_wasm_hash, version));
     }
     ```
     `BytesN` is already in the import list.

  5. **Add public RBAC entrypoints**:
     ```rust
     pub fn grant_role(env: Env, caller: Address, address: Address, role: types::AdminRole) {
         Self::require_not_paused(&env);
         caller.require_auth();
         if !types::has_role(&env, &caller, &types::AdminRole::Admin) {
             panic_with_error!(&env, ContractError::NotAuthorized);
         }
         types::grant_role(&env, &address, &role);
         events::role_granted(&env, &address, &role);
     }

     pub fn revoke_role(env: Env, caller: Address, address: Address, role: types::AdminRole) {
         Self::require_not_paused(&env);
         caller.require_auth();
         if !types::has_role(&env, &caller, &types::AdminRole::Admin) {
             panic_with_error!(&env, ContractError::NotAuthorized);
         }
         types::revoke_role(&env, &address, &role);
         events::role_revoked(&env, &address, &role);
     }

     pub fn has_role(env: Env, address: Address, role: types::AdminRole) -> bool {
         types::has_role(&env, &address, &role)
     }
     ```

  **Files**: `contracts/progress-tracker/src/lib.rs`

  **Verify**: `cargo check -p progress-tracker` — compiles. Some existing test call sites will need updating (fixed in item 6).

- [ ] 5. Fix existing tests in `credential-nft/src/lib.rs` and workspace integration tests to match the updated call signatures.

  **What**: The signature changes in items 1-2 break every existing call to `pause`, `unpause`, `mint_credential`, and `upgrade` in credential-nft. Update all call sites. Depends on items 1-2.

  **`contracts/credential-nft/src/lib.rs` `#[cfg(test)]` module**:
  - `test_pause_emits_paused_event`: change `client.pause()` to `client.pause(&admin)`.
  - `test_unpause_emits_unpaused_event`: change `client.pause()` / `client.unpause()` to `client.pause(&admin)` / `client.unpause(&admin)`.
  - `test_pause_rejects_double_pause`: `client.pause()` x2 ? `client.pause(&admin)` x2.
  - `test_unpause_rejects_when_not_paused`: `client.unpause()` ? `client.unpause(&admin)`.
  - `test_mint_and_verify` and all other tests calling `client.mint_credential(...)`: add `&admin` as the first argument — `client.mint_credential(&admin, &learner, ...)`.
  - Any `client.upgrade(...)` calls in tests: add `&admin` as first argument.

  **Workspace integration tests** — search for calls to these functions in:
  - `tests/integration/credential_flow.rs`
  - `tests/integration/full_flow.rs`
  - `tests/integration/security_auth_tests.rs`
  - `tests/integration/upgrade_tests.rs`
  - `tests/integration/pause_event_consistency.rs`
  - `tests/integration/pause_flow.rs`
  - `tests/unit/credential_tests.rs`
  - Any other file under `tests/` that imports `credential-nft`.

  For each call to `credential_client.pause()` ? `credential_client.pause(&admin)`, `credential_client.unpause()` ? `credential_client.unpause(&admin)`, `credential_client.mint_credential(learner, ...)` ? `credential_client.mint_credential(&admin, &learner, ...)`, `credential_client.upgrade(hash)` ? `credential_client.upgrade(&admin, &hash)`.

  **Files**: `contracts/credential-nft/src/lib.rs` (test module), all affected files under `tests/`

  **Verify**: `cargo test -p credential-nft` and `cargo test --workspace 2>&1 | grep -E "FAILED|error"` — no compile errors or test failures outside the new RBAC tests (which do not exist yet).

- [ ] 6. Fix existing tests in `progress-tracker/src/lib.rs` and workspace integration tests to match updated call signatures.

  **What**: The signature changes in items 3-4 break existing calls to `pause` and `unpause` in progress-tracker. Update all call sites. Depends on items 3-4.

  **`contracts/progress-tracker/src/lib.rs` `#[cfg(test)]` module** (if any tests exist there):
  - Change `client.pause()` ? `client.pause(&admin)`, `client.unpause()` ? `client.unpause(&admin)`.

  **Workspace integration tests** — search for `progress_tracker_client.pause()` / `.unpause()` in:
  - `tests/integration/pause_flow.rs`
  - `tests/integration/pause_event_consistency.rs`
  - `tests/integration/full_flow.rs`
  - `tests/integration/security_auth_tests.rs`
  - `tests/unit/progress_tests.rs`
  - Any other file that calls progress-tracker pause/unpause.

  For each: `tracker_client.pause()` ? `tracker_client.pause(&admin)`, `tracker_client.unpause()` ? `tracker_client.unpause(&admin)`.

  **Files**: `contracts/progress-tracker/src/lib.rs` (test module), all affected files under `tests/`

  **Verify**: `cargo test -p progress-tracker` and `cargo test --workspace` — all existing tests pass.

- [ ] 7. Add RBAC tests to `credential-nft/src/lib.rs` `#[cfg(test)]` module.

  **What**: Add the tests required by the issue. All tests use the existing `setup_contract`, `create_course`, `complete_course`, `enrolled_and_completed` helpers already in the test module. Use `env.mock_all_auths()` to keep test setup simple — exactly as existing tests do. Depends on items 1-2 and 5.

  Test cases to add (each as a separate `#[test]` function):

  **grant/revoke round-trips and has_role correctness**:
  - `test_has_role_returns_false_before_grant`: generate minter address; assert `!client.has_role(&minter, &AdminRole::Minter)`.
  - `test_grant_role_minter_enables_has_role`: grant Minter to address; assert `client.has_role(&minter, &AdminRole::Minter)`.
  - `test_revoke_role_removes_has_role`: grant then revoke Minter; assert `!client.has_role(&minter, &AdminRole::Minter)`.
  - `test_admin_always_has_all_roles`: assert admin address (from `setup_contract`) has `has_role(Admin)`, `has_role(Minter)`, and `has_role(Pauser)` all true without any explicit grant.

  **Admin can delegate**:
  - `test_admin_can_grant_minter_who_can_mint`: grant Minter role to `minter` via `client.grant_role(&admin, &minter, &AdminRole::Minter)`; call `client.mint_credential(&minter, &learner, ...)` — assert it returns a credential ID.
  - `test_admin_role_holder_can_grant_roles`: grant Admin role to `sub_admin`; verify `sub_admin` can in turn grant Minter to another address.

  **Mint gated on Minter OR Admin**:
  - `test_mint_requires_minter_or_admin_role`: generate `nobody` (no roles); call `client.try_mint_credential(&nobody, &learner, ...)` — assert it returns `Err(_)`.
  - `test_admin_can_mint_without_explicit_minter_grant`: call `client.mint_credential(&admin, &learner, ...)` without granting Minter explicitly — assert success (admin always has all roles).

  **Pause/unpause gated on Pauser OR Admin**:
  - `test_pause_requires_pauser_role`: `client.try_pause(&nobody)` — assert `Err(_)`.
  - `test_unpause_requires_pauser_role`: pause with admin; `client.try_unpause(&nobody)` — assert `Err(_)`.
  - `test_pauser_can_pause_and_unpause`: grant Pauser to `pauser`; `client.pause(&pauser)` — assert `is_paused()`; `client.unpause(&pauser)` — assert not paused.
  - `test_admin_can_pause_without_explicit_pauser_grant`: `client.pause(&admin)` without granting Pauser — assert succeeds.

  **Upgrade gated on Admin**:
  - `test_upgrade_requires_admin_role`: `client.try_upgrade(&nobody, &wasm_hash)` — assert `Err(_)`.

  **Non-admin cannot grant/revoke**:
  - `test_non_admin_cannot_grant_role`: grant Minter to `minter`; `client.try_grant_role(&minter, &other, &AdminRole::Minter)` — assert `Err(_)`.
  - `test_non_admin_cannot_revoke_role`: grant Minter; `client.try_revoke_role(&nobody, &minter, &AdminRole::Minter)` — assert `Err(_)`.

  **Files**: `contracts/credential-nft/src/lib.rs`

  **Verify**: `cargo test -p credential-nft -- --test-output immediate 2>&1 | tail -20` — all new RBAC tests pass, no regressions.

- [ ] 8. Add RBAC tests to `progress-tracker/src/lib.rs` `#[cfg(test)]` module (or a new test module file).

  **What**: Add the required RBAC tests for progress-tracker. Progress-tracker's existing test module is in `lib.rs`. If no `#[cfg(test)]` module exists there, add one. Depends on items 3-4 and 6.

  First check: grep `#[cfg(test)]` in `contracts/progress-tracker/src/lib.rs` to confirm if a test module exists. If not, add one following the same pattern as credential-nft (using `Env::default()`, `mock_all_auths()`, helpers for setup).

  Test cases to add:

  **grant/revoke round-trips and has_role correctness**:
  - `test_has_role_returns_false_before_grant`: assert `!client.has_role(&pauser, &AdminRole::Pauser)`.
  - `test_grant_role_enables_has_role`: grant Pauser; assert `client.has_role(&pauser, &AdminRole::Pauser)`.
  - `test_revoke_role_removes_has_role`: grant then revoke Pauser; assert false.
  - `test_admin_has_all_roles_by_default`: assert initialized admin has Admin, Minter, and Pauser all true.

  **Pause/unpause gated on Pauser OR Admin**:
  - `test_pause_requires_pauser_role`: `client.try_pause(&nobody)` — assert `Err(_)`.
  - `test_unpause_requires_pauser_role`: pause via admin; `client.try_unpause(&nobody)` — assert `Err(_)`.
  - `test_pauser_can_pause_and_unpause`: grant Pauser; assert pause/unpause round-trip succeeds.
  - `test_admin_can_pause_without_explicit_pauser_grant`: `client.pause(&admin)` without explicit Pauser grant — succeeds.

  **Upgrade gated on Admin**:
  - `test_upgrade_requires_admin_role`: `client.try_upgrade(&nobody, &wasm_hash)` — assert `Err(_)`.

  **Non-admin cannot grant/revoke**:
  - `test_non_admin_cannot_grant_role`: grant Pauser to `pauser`; `client.try_grant_role(&pauser, &other, &AdminRole::Pauser)` — assert `Err(_)`.
  - `test_non_admin_cannot_revoke_role`: `client.try_revoke_role(&nobody, &pauser, &AdminRole::Pauser)` — assert `Err(_)`.

  **Admin can delegate**:
  - `test_admin_role_holder_can_grant_roles`: grant Admin to `sub_admin`; verify `sub_admin` can grant Pauser to `pauser`.

  **Files**: `contracts/progress-tracker/src/lib.rs`

  **Verify**: `cargo test -p progress-tracker 2>&1 | tail -20` — all new tests pass.

- [ ] 9. Full workspace build and test pass; clippy and format check.

  **What**: Final verification that the entire workspace compiles, all tests pass, no new clippy warnings, and formatting is consistent. Depends on all previous items.

  Run in order:
  1. `cargo fmt --all` — auto-format all changed files.
  2. `cargo clippy --workspace --all-targets -- -D warnings` — confirm no new warnings. Expected sources of lint: `AdminRole` derives `PartialEq` and `Eq` but is also `#[contracttype]` (same pattern as learn-token, which already passes); no new issues expected.
  3. `cargo test --workspace` — all tests pass. The integration test suite includes `admin_role_flow` (for learn-token), `security_auth_tests`, `pause_flow`, and `credential_flow` — all must remain green.

  **Files**: none (verification only)

  **Verify**: `cargo test --workspace` exits with code 0 and reports zero failures.

---

## Summary of all files changed

| File | Change |
|------|--------|
| `contracts/credential-nft/src/metadata.rs` | Add `AdminRole`, `RoleKey`, `Role(RoleKey)` variant, `has_role`/`grant_role`/`revoke_role` helpers |
| `contracts/credential-nft/src/events.rs` | Add `role_granted`, `role_revoked` event functions |
| `contracts/credential-nft/src/lib.rs` | Add `NotAuthorized = 2` to `ContractError`; add `panic_with_error` import; update `initialize`, `mint_credential`, `pause`, `unpause`, `upgrade`; add public `grant_role`/`revoke_role`/`has_role`; add RBAC tests |
| `contracts/progress-tracker/src/types.rs` | Add `AdminRole`, `RoleKey`, `Role(RoleKey)` variant, `has_role`/`grant_role`/`revoke_role` helpers |
| `contracts/progress-tracker/src/events.rs` | Add `role_granted`, `role_revoked` event functions |
| `contracts/progress-tracker/src/lib.rs` | Add `NotAuthorized = 1` to `ContractError`; add `panic_with_error` import; update `initialize`, `pause`, `unpause`; add `upgrade`; add public `grant_role`/`revoke_role`/`has_role`; add RBAC tests |
| `tests/**` | Update call sites for `pause`, `unpause`, `mint_credential`, `upgrade` in credential-nft and progress-tracker clients |
