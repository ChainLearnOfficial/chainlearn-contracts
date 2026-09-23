# ChainLearn Contracts

A Soroban smart contract workspace for a Stellar-based learning platform. ChainLearn enables on-chain learning progress tracking, quiz-based token rewards, and verifiable credential NFTs.

## Architecture

For a detailed architecture overview including diagrams and storage layout, see [docs/architecture.md](./docs/architecture.md).

The workspace contains three interconnected contracts and a shared utilities package:

### learn-token (SEP-41 Fungible Token)

The reward token for the platform. Implements the SEP-41 fungible token standard with additional reward logic:

- **Standard interface**: `initialize`, `mint`, `transfer`, `balance`, `total_supply`, `approve`, `allowance`
- **Reward system**: `claim_reward(learner, course_id, quiz_id)` looks up the learner's score for
  that quiz from progress-tracker and mints tokens proportional to it
- **Standard interface**: `initialize`, `mint`, `transfer`, `transfer_from`, `burn`, `burn_from`,
  `balance`, `total_supply`, `approve`, `allowance`
- **Burning**: `burn(from, amount)` and `burn_from(spender, from, amount)` destroy tokens and
  reduce `total_supply`; `burn_from` spends the caller's approved allowance
- **Reward system**: `claim_reward(learner, quiz_id, score)` mints tokens proportional to quiz score
- **Anti-fraud**: Each quiz reward can only be claimed once per learner
- **Reward formula**: `score * BASE_REWARD_PER_POINT` (100 tokens per point)

### credential-nft (Course Certificates)

Non-transferable (soulbound) credential NFTs that certify course completion:

- **Minting**: `mint_credential(to, course_id, score, metadata_uri)` -- score-gated at 50+
- **Completion-gated**: Minting calls `progress-tracker.is_eligible_for_credential()` and rejects
  learners who have not completed every module and quiz in the course
- **Score-verified**: Minting also calls `progress-tracker.get_course_score()` and rejects any
  `score` that does not match the learner's on-chain average, so a caller cannot inflate a
  credential
- **Verification**: `verify_credential(credential_id)` returns full credential info
- **Lookup**: `get_credentials_for(learner, start, limit)` returns one page of a learner's
  credential IDs; `get_credential_count(learner)` returns the total so callers can page
- **Revocation**: Admin can revoke credentials if needed
- **One per course**: Each learner can only receive one credential per course
- **Non-transferable**: `transfer()` is rejected -- credentials are permanently bound to the earner
- **Events**: `credential_minted(learner, course_id, credential_id, score, metadata_uri)` and
  `credential_revoked(learner, course_id, credential_id, admin)`

### progress-tracker (On-chain Progress)

Tracks learner enrollment, module completion, and quiz scores:

- **Enrollment**: `enroll(learner, course_id)`
- **Module tracking**: `complete_module(learner, course_id, module_id)`
- **Quiz scores**: `submit_quiz_score(learner, course_id, quiz_id, score)`
- **Progress view**: `get_progress(learner, course_id)` returns `ProgressInfo`
- **Verified scores**: `get_quiz_score(learner, course_id, quiz_id)` for one quiz and
  `get_course_score(learner, course_id)` for the course average -- the values learn-token and
  credential-nft check before minting
- **Eligibility**: Automatic credential eligibility calculation
- **Weighted progress**: 70% module completion + 30% quiz performance
- **Events**: `enrolled(learner, course_id, enrolled_at)`,
  `module_completed(learner, course_id, module_id, overall_progress)`,
  `quiz_submitted(learner, course_id, quiz_id, score)`, and `credential_eligible(learner, course_id)` --
  published once, the moment eligibility flips from false to true, so indexers don't have to poll
  `get_progress`

### shared (Utilities Package)

Common types and constants used across all contracts:

- `MIN_CREDENTIAL_SCORE` (50): Minimum score to mint a credential
- `MAX_QUIZ_SCORE` (100): Maximum possible quiz score
- `TOKEN_DECIMALS` (7): Token decimal places
- `BASE_REWARD_PER_POINT` (100): Tokens minted per quiz point
- `MAX_CREDENTIALS_PAGE_SIZE` (50): Maximum credentials returned by one paginated read

### Cross-Contract Dependencies

`learn-token` and `credential-nft` both call back into `progress-tracker` to verify on-chain
progress before releasing a reward or credential, so the `progress-tracker` contract ID must be
known and supplied when the other two are initialized:

- **learn-token**: `initialize(admin, name, symbol, decimal, progress_tracker)` stores the
  `progress-tracker` address. `claim_reward` calls `progress-tracker.get_quiz_score()` to read
  the learner's verified score before minting -- the score cannot be supplied directly by the caller.
- **credential-nft**: `initialize(admin, progress_tracker)` stores the same address.
  `mint_credential` calls `progress-tracker.is_eligible_for_credential()` and rejects the mint if
  the learner has not completed every module and quiz in the course.
- `./scripts/deploy.sh` deploys all three contracts and writes their contract IDs to
  `deployments-<network>.json`. `./scripts/initialize.sh` then reads that file and passes the
  `progress-tracker` contract ID as the `progress_tracker` argument to both
  `learn-token.initialize` and `credential-nft.initialize`.

## Directory Structure

```
chainlearn-contracts/
├── contracts/
│   ├── learn-token/          # SEP-41 fungible token
│   ├── credential-nft/       # NFT credentials
│   └── progress-tracker/     # Learning progress
├── packages/
│   └── shared/               # Shared types and constants
├── tests/
│   ├── integration/          # End-to-end flow tests
│   └── unit/                 # Contract unit tests
├── scripts/
│   ├── deploy.sh             # Deployment script
│   └── initialize.sh         # Post-deploy initialization
├── Cargo.toml                # Workspace root
└── README.md
```

## Prerequisites

- **Rust** 1.70+ with `wasm32-unknown-unknown` target
- **Soroban CLI** v21+
- **Stellar account** with XLM for deployment fees

Install the WASM target:

```bash
rustup target add wasm32-unknown-unknown
```

Install Soroban CLI:

```bash
cargo install --locked soroban-cli --version 21.0.0
```

## Build

Build all contracts for release:

```bash
cargo build --release --target wasm32-unknown-unknown
```

Build in development mode (faster, includes debug info):

```bash
cargo build --target wasm32-unknown-unknown
```

## Test

Run all tests:

```bash
cargo test
```

Run tests for a specific contract:

```bash
cargo test -p learn-token
cargo test -p credential-nft
cargo test -p progress-tracker
```

Run with output:

```bash
cargo test -- --nocapture
```

## Deploy

### Testnet

1. Set your secret key:

```bash
export STELLAR_SECRET_KEY="S..."
```

2. Deploy contracts:

```bash
./scripts/deploy.sh testnet
```

3. Initialize contracts:

```bash
./scripts/initialize.sh testnet
```

The script initializes in dependency order -- progress-tracker first, then learn-token and
credential-nft, which each store the tracker's address -- and verifies the wiring landed before
reporting success.

### Mainnet

```bash
./scripts/deploy.sh mainnet
./scripts/initialize.sh mainnet
```

## Usage Flow

### Creating a Course (Admin)

```bash
soroban contract invoke \
    --id <PROGRESS_TRACKER_ID> \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    create_course \
    --course_id "rust_101" \
    --total_modules 3 \
    --total_quizzes 2 \
    --module_ids '["mod_basics","mod_ownership","mod_traits"]'
```

### Learner Journey

1. **Enroll** in a course
2. **Complete modules** one by one
3. **Submit quiz scores** after each module section
4. **Claim token rewards** for each quiz
5. **Receive credential NFT** upon full completion

### Contract Interaction Example

```bash
# Enroll
soroban contract invoke --id <PROGRESS_ID> -- enroll \
    --learner <ADDRESS> --course_id "rust_101"

# Complete a module
soroban contract invoke --id <PROGRESS_ID> -- complete_module \
    --learner <ADDRESS> --course_id "rust_101" --module_id "mod_basics"

# Submit quiz score
soroban contract invoke --id <PROGRESS_ID> -- submit_quiz_score \
    --learner <ADDRESS> --course_id "rust_101" --quiz_id "quiz_1" --score 85

# Claim token reward
soroban contract invoke --id <TOKEN_ID> -- claim_reward \
    --learner <ADDRESS> --course_id "rust_101" --quiz_id "quiz_1"

# Mint credential (after completing all modules and quizzes)
soroban contract invoke --id <CREDENTIAL_ID> -- mint_credential \
    --to <ADDRESS> --course_id "rust_101" --score 80 \
    --metadata_uri "ipfs://Qm..."

# List a learner's credentials, one page at a time
soroban contract invoke --id <CREDENTIAL_ID> -- get_credential_count \
    --learner <ADDRESS>
soroban contract invoke --id <CREDENTIAL_ID> -- get_credentials_for \
    --learner <ADDRESS> --start 0 --limit 50
```

## Data Types

### ProgressInfo

```rust
struct ProgressInfo {
    enrolled_at: u64,           // Timestamp of enrollment
    quizzes_submitted: u32,     // Number of quizzes submitted
    total_quiz_score: u64,      // Sum of submitted scores (average is derived)
    overall_progress: u32,      // Progress percentage (0-100)
    eligible_for_credential: bool,   // Qualifies for credential
}
```

Module completion and individual quiz submissions are not duplicated here:
they are stored once, under the `ModuleCompleted` and `QuizResult` storage
keys. Use `get_quiz_score(learner, course_id, quiz_id)` to read a single
result.

### CredentialInfo

```rust
struct CredentialInfo {
    learner: Address,           // Credential holder
    course_id: Symbol,          // Course identifier
    score: u32,                 // Final score (0-100)
    issued_at: u64,             // Issuance timestamp
    revoked: bool,              // Revocation status
    metadata_uri: Symbol,       // Off-chain metadata URI
}
```

### QuizResult

```rust
struct QuizResult {
    quiz_id: Symbol,            // Quiz identifier
    course_id: Symbol,          // Parent course
    score: u32,                 // Score achieved
    submitted_at: u64,          // Submission timestamp
}
```

## Progress Calculation

Overall progress is calculated as a weighted average using integer division:

- **Module completion**: 70% weight (proportion of modules completed)
- **Quiz performance**: 30% weight (average quiz score / 100)

Formula: `progress = (completed_modules * 70 / total_modules) + (avg_quiz_score * 30 / 100)`

**Note**: Integer division is used throughout; calculations are performed left-to-right with rounding down at each step.

## Credential Eligibility

A learner is eligible for a credential when:

1. All course modules are completed
2. All quizzes are submitted
3. Average quiz score >= 50 (MIN_CREDENTIAL_SCORE)

## Security Considerations

- **Auth**: All state-changing functions require authorization from the relevant party
- **Verified progress**: Credential minting is gated on the progress-tracker's eligibility check,
  so a credential cannot be self-issued without actually completing the course
- **Double-claim prevention**: Token rewards and quiz submissions are tracked to prevent duplicates
- **Score gating**: Credentials require a minimum passing score (50)
- **Admin controls**: Only the admin can create courses, mint tokens, and revoke credentials
- **Soulbound enforcement**: Credential NFTs are permanently bound to the earner; `transfer()` is rejected
  to prevent trading or theft of credentials

## API Reference

### progress-tracker

#### Initialization

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `initialize` | — | `admin: Address` | `Result<(), ContractError>` | Initialize the tracker. Can only be called once. |

#### Course Management (Admin)

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `create_course` | Admin | `course_id: Symbol, total_modules: u32, total_quizzes: u32, module_ids: Vec<Symbol>, quiz_ids: Vec<Symbol>` | — | Register a new course with modules and quizzes. |
| `archive_course` | Admin | `course_id: Symbol` | — | Archive a course, preventing new enrollments. |
| `set_course_content_hash` | Admin | `course_id: Symbol, content_hash: Symbol` | — | Set or update the content integrity hash. |
| `set_course_difficulty` | Admin | `course_id: Symbol, difficulty: u32` | — | Set difficulty (0=beginner, 1=intermediate, 2=advanced). |
| `set_course_tags` | Admin | `course_id: Symbol, tags: Vec<Symbol>` | — | Set tags for course categorization. |
| `update_course_version` | Admin | `course_id: Symbol` | — | Increment course content version. |
| `set_course_prerequisites` | Admin | `course_id: Symbol, prerequisites: Vec<Symbol>` | — | Set prerequisite courses. |

#### Learner Progress

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `enroll` | Learner | `learner: Address, course_id: Symbol` | — | Enroll in a course. |
| `enroll_checked` | Learner | `learner: Address, course_id: Symbol, expected_content_hash: Option<Symbol>` | — | Enroll with content hash verification. |
| `complete_module` | Learner | `learner: Address, course_id: Symbol, module_id: Symbol` | — | Mark a module complete (sequential ordering enforced). |
| `batch_complete_module` | Learner | `learner: Address, course_id: Symbol, module_ids: Vec<Symbol>` | — | Complete multiple modules atomically. |
| `submit_quiz_score` | Learner | `learner: Address, course_id: Symbol, quiz_id: Symbol, score: u32` | — | Submit a quiz score (0-100). |
| `batch_submit_quiz_score` | Learner | `learner: Address, course_id: Symbol, quiz_scores: Vec<(Symbol, u32)>` | — | Submit multiple quiz scores (partial failures skipped). |
| `retake_quiz` | Learner | `learner: Address, course_id: Symbol, quiz_id: Symbol, new_score: u32` | — | Retake a quiz with a higher score. |

#### Queries

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `get_progress` | — | `learner: Address, course_id: Symbol` | `ProgressInfo` | Full progress for a learner in a course. |
| `get_completion_percentage` | — | `learner: Address, course_id: Symbol` | `u32` | Progress percentage (0-100) only. |
| `export_progress` | — | `learner: Address, course_id: Symbol` | `ProgressExport` | Complete progress snapshot including all quiz results. |
| `get_quiz_score` | — | `learner: Address, course_id: Symbol, quiz_id: Symbol` | `u32` | Verified score for one quiz. |
| `get_course_score` | — | `learner: Address, course_id: Symbol` | `u32` | Average quiz score across all submitted quizzes. |
| `is_eligible_for_credential` | — | `learner: Address, course_id: Symbol` | `bool` | Whether the learner qualifies for a credential. |
| `get_course` | — | `course_id: Symbol` | `Course` | Course configuration. |
| `get_course_difficulty` | — | `course_id: Symbol` | `u32` | Difficulty level of a course. |
| `get_course_created_at` | — | `course_id: Symbol` | `u64` | Immutable creation timestamp. |
| `get_course_tags` | — | `course_id: Symbol` | `Vec<Symbol>` | Tags for a course. |
| `get_courses_by_tag` | — | `tag: Symbol` | `Vec<Symbol>` | All courses with a given tag. |
| `get_learner_stats` | — | `learner: Address` | `LearnerStats` | Aggregate stats across all courses. |

#### Metadata

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `contract_metadata` | — | — | `VersionedContractMetadata` | Contract name, version, and upgrade count. |
| `contract_version` | — | — | `u32` | On-chain upgrade counter. |
| `is_initialized` | — | — | `bool` | Whether `initialize()` has been called. |
| `get_storage_size` | — | — | `u64` | Number of persistent storage entries. |

### learn-token

#### Initialization

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `initialize` | — | `admin: Address, name: SorobanString, symbol: SorobanString, decimal: u32, progress_tracker: Address, max_supply: i128` | `Result<(), ContractError>` | Initialize the token. Can only be called once. |

#### SEP-41 Standard Interface

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `name` | — | — | `SorobanString` | Token name. |
| `symbol` | — | — | `SorobanString` | Token symbol. |
| `decimals` | — | — | `u32` | Number of decimal places. |
| `total_supply` | — | — | `i128` | Current total supply. |
| `balance` | — | `address: Address` | `i128` | Token balance of an address. |
| `total_minted_to` | — | `address: Address` | `i128` | Cumulative amount ever minted to an address. |
| `transfer` | Owner | `from: Address, to: Address, amount: i128` | — | Transfer tokens. |
| `transfer_from` | Spender | `spender: Address, from: Address, to: Address, amount: i128` | — | Transfer on behalf of another address. |
| `approve` | Owner | `owner: Address, spender: Address, amount: i128, expiration_ledger: u32` | — | Approve a spender. |
| `increase_allowance` | Owner | `owner: Address, spender: Address, additional_amount: i128, expiration_ledger: u32` | — | Increase allowance (front-run safe). |
| `allowance` | — | `owner: Address, spender: Address` | `i128` | Remaining allowance. |
| `burn` | Owner | `from: Address, amount: i128` | — | Burn tokens. |
| `burn_from` | Spender | `spender: Address, from: Address, amount: i128` | — | Burn via allowance. |
| `mint` | Minter | `caller: Address, to: Address, amount: i128` | — | Mint new tokens (admin/minter only). |
| `prune_expired_allowance` | — | `owner: Address, spender: Address` | `bool` | Remove an expired allowance from storage. |
| `cleanup_expired_allowances` | — | `owner: Address` | `u32` | Remove all expired allowances for an owner. |

#### ChainLearn Rewards

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `claim_reward` | Learner | `learner: Address, course_id: Symbol, quiz_id: Symbol` | — | Claim token reward for a quiz. Score verified on-chain. |
| `batch_claim_reward` | Learner | `learner: Address, course_id: Symbol, quiz_ids: Vec<Symbol>` | `Vec<Symbol>` | Claim rewards for multiple quizzes. Partial failures skipped. |
| `estimate_claim_gas` | — | `learner: Address, course_id: Symbol, quiz_id: Symbol` | `ClaimEstimate` | Preview a claim without executing (read-only). |
| `get_claim_history` | — | `learner: Address` | `Vec<ClaimRecord>` | Full reward claim history for a learner. |

#### Transfer Restrictions

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `set_transfer_restriction` | Admin | `restriction: TransferRestriction` | — | Set transfer restriction (None/WhitelistOnly/Cooldown/MaxAmount). |
| `get_transfer_restriction` | — | — | `TransferRestriction` | Current transfer restriction. |
| `add_to_whitelist` | Admin | `address: Address` | — | Add address to whitelist. |
| `remove_from_whitelist` | Admin | `address: Address` | — | Remove address from whitelist. |
| `is_whitelisted` | — | `address: Address` | `bool` | Check whitelist status. |

#### Snapshots

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `snapshot` | Admin | `ledger_height: u32` | — | Create a balance snapshot. |
| `balance_at` | — | `address: Address, ledger_height: u32` | `i128` | Balance at a specific snapshot. |

#### Admin

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `admin` | — | — | `Address` | Admin address. |
| `has_role` | — | `address: Address, role: AdminRole` | `bool` | Check role assignment. |
| `grant_role` | Admin | `caller: Address, address: Address, role: AdminRole` | — | Grant a role. |
| `revoke_role` | Admin | `caller: Address, address: Address, role: AdminRole` | — | Revoke a role. |
| `add_admin` | Admin | `caller: Address, admin_info: AdminInfo` | — | Add admin with role. |
| `remove_admin` | Admin | `caller: Address, admin_info: AdminInfo` | — | Remove admin. |
| `get_admins` | — | — | `Vec<AdminInfo>` | List all admins and roles. |
| `execute_multisig_op` | Multi-Admin | `caller: Address, co_signer: Address, operation: Symbol` | — | Execute a critical operation requiring two admins. |
| `upgrade_multisig` | Multi-Admin | `caller: Address, co_signer: Address, new_wasm_hash: BytesN<32>` | — | Upgrade contract with multi-sig. |
| `upgrade` | Admin | `new_wasm_hash: BytesN<32>` | — | Upgrade contract WASM (single admin). |
| `wasm_hash` | — | — | `Option<BytesN<32>>` | Current WASM hash. |
| `upgrade_version` | — | — | `u32` | Number of upgrades performed. |

#### Admin Transfer (Delayed)

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `transfer_admin` | Admin | `new_admin: Address` | — | Initiate delayed admin transfer. |
| `accept_admin` | Pending | — | — | Accept admin role after delay. |
| `cancel_admin_transfer` | Admin | — | — | Cancel pending admin transfer. |
| `pending_admin` | — | — | `Option<PendingAdminTransfer>` | Pending transfer details. |
| `admin_transfer_delay` | — | — | `u64` | Current delay in seconds. |
| `set_admin_transfer_delay` | Admin | `delay_seconds: u64` | — | Set the delay. |

#### Configuration

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `progress_tracker` | — | — | `Address` | Progress-tracker contract address. |
| `set_progress_tracker` | Admin | `new_progress_tracker: Address` | — | Update progress-tracker address. |
| `max_supply` | — | — | `i128` | Maximum supply cap. |
| `set_max_supply` | Admin | `new_max_supply: i128` | — | Update supply cap (max 2x increase). |
| `contract_metadata` | — | — | `ContractMetadata` | Contract name and version. |
| `is_initialized` | — | — | `bool` | Whether initialized. |
| `get_storage_size` | — | — | `u32` | Number of persistent storage entries. |

#### Emergency Controls

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `pause` | Pauser | `caller: Address` | — | Pause all state-changing operations. |
| `unpause` | Pauser | `caller: Address` | — | Resume operations. |
| `is_paused` | — | — | `bool` | Whether paused. |

#### Vesting

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `create_vesting_schedule` | Admin | `beneficiary: Address, total_amount: i128, cliff_timestamp: u64, duration_seconds: u64` | — | Create a vesting schedule. |
| `claim_vested_tokens` | Beneficiary | `beneficiary: Address` | — | Claim vested tokens. |
| `get_vesting_schedule` | — | `beneficiary: Address` | `Option<VestingSchedule>` | Vesting schedule details. |

#### Governance

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `create_proposal` | Admin | `description: String, choices: u32, start_time: u64, end_time: u64` | `u64` | Create a governance proposal. |
| `vote` | Token Holder | `proposal_id: u64, choice: u32` | — | Cast a vote (voting power = token balance). |
| `execute_proposal` | — | `proposal_id: u64` | — | Execute a passed proposal. |
| `get_proposal` | — | `proposal_id: u64` | `Option<Proposal>` | Proposal details. |

### credential-nft

#### Initialization

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `initialize` | — | `admin: Address, progress_tracker: Address` | `Result<(), ContractError>` | Initialize the contract. Can only be called once. |

#### Credential Management

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `mint_credential` | Admin + Learner | `to: Address, course_id: Symbol, score: u32, metadata_uri: Symbol` | `u64` | Mint a credential NFT. Score-gated (>=50) and verified on-chain. |
| `verify_credential` | — | `credential_id: u64` | `CredentialInfo` | Verify a credential and return its info. |
| `verify_credential_with_display` | — | `credential_id: u64` | `CredentialVerification` | Verify with optional display properties. |
| `revoke_credential` | Admin | `credential_id: u64` | — | Revoke a credential. |
| `revoke_credential_with_reason` | Admin | `credential_id: u64, reason: Symbol` | — | Revoke with a reason. |
| `get_revocation_reason` | — | `credential_id: u64` | `Option<Symbol>` | Revocation reason, if any. |
| `renew_credential` | Admin | `credential_id: u64, new_expiry: u32` | — | Renew expiration (0 = no expiration). |
| `is_credential_valid` | — | `credential_id: u64` | `bool` | Check if valid (exists, not revoked, not expired). |

#### Display Properties

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `set_credential_display` | Admin | `credential_id: u64, image_url: Option<Symbol>, description: Option<Symbol>, issuer_name: Option<Symbol>` | — | Set display properties. |
| `get_credential_display` | — | `credential_id: u64` | `Option<CredentialDisplay>` | Get display properties. |

#### Lookups

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `get_credentials_for` | — | `learner: Address, start: u32, limit: u32` | `Vec<u64>` | Paginated credential IDs for a learner. |
| `get_credential_count` | — | `learner: Address` | `u32` | Total credentials a learner holds. |
| `get_total_credentials_count` | — | — | `u64` | Total credentials issued across all learners. |
| `get_credentials_by_course` | — | `course_id: Symbol` | `Vec<u64>` | All credential IDs for a course. |

#### Soulbound Transfer

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `transfer` | — | `from: Address, to: Address, credential_id: u64` | `Result<(), ContractError>` | Always returns `Err(Soulbound)`. Credentials are non-transferable. |

#### Certificate Generation

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `generate_certificate` | Learner | `learner: Address, course_id: Symbol` | `Symbol` | Generate a course completion certificate URI. |
| `get_certificate_uri` | — | `learner: Address, course_id: Symbol` | `Option<Symbol>` | Query a generated certificate URI. |

#### Admin

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `admin` | — | — | `Address` | Admin address. |
| `progress_tracker` | — | — | `Address` | Progress-tracker address. |
| `transfer_admin` | Admin | `new_admin: Address` | — | Transfer admin rights. |
| `contract_metadata` | — | — | `ContractMetadata` | Contract name and version. |
| `is_initialized` | — | — | `bool` | Whether initialized. |
| `get_storage_size` | — | — | `u64` | Number of persistent storage entries. |

#### Emergency Controls

| Function | Auth | Parameters | Returns | Description |
|---|---|---|---|---|
| `emergency_pause` | Admin | — | — | Pause all state-changing operations. |
| `unpause` | Admin | — | — | Resume operations. |

## Data Types

### ProgressInfo

```rust
struct ProgressInfo {
    enrolled_at: u64,                    // Timestamp of enrollment
    modules_completed_bitmap: u64,       // Bitmap of completed module indices
    quizzes_submitted: u32,              // Number of quizzes submitted
    total_quiz_score: u64,               // Sum of submitted scores (average is derived)
    overall_progress: u32,               // Progress percentage (0-100)
    eligible_for_credential: bool,       // Qualifies for credential
    completed_version: Option<u32>,      // Course version when eligibility was reached
}
```

Module completion and individual quiz submissions are not duplicated here:
they are stored once, under the `ModuleCompleted` and `QuizResult` storage
keys. Use `get_quiz_score(learner, course_id, quiz_id)` to read a single
result.

### Course

```rust
struct Course {
    course_id: Symbol,                   // Unique course identifier
    total_modules: u32,                  // Number of modules
    total_quizzes: u32,                  // Number of quizzes
    module_ids: Vec<Symbol>,             // Ordered module identifiers
    quiz_ids: Vec<Symbol>,               // Quiz identifiers
    archived: bool,                      // Whether archived
    content_hash: Symbol,                // Off-chain content hash (optional)
    prerequisites: Vec<Symbol>,          // Required courses
    version: u32,                        // Content version
    updated_at: u64,                     // Last modification timestamp
    created_at: u64,                     // Immutable creation timestamp
    difficulty: u32,                     // 0=beginner, 1=intermediate, 2=advanced
    tags: Vec<Symbol>,                   // Categorization tags
}
```

### CredentialInfo

```rust
struct CredentialInfo {
    learner: Address,                    // Credential holder
    course_id: Symbol,                   // Course identifier
    score: u32,                          // Final score (0-100)
    issued_at: u64,                      // Issuance timestamp
    revoked: bool,                       // Revocation status
    metadata_uri: Symbol,                // Off-chain metadata URI
    expires_at: u32,                     // Expiration ledger (0 = no expiration)
}
```

### QuizResult

```rust
struct QuizResult {
    quiz_id: Symbol,                     // Quiz identifier
    course_id: Symbol,                   // Parent course
    score: u32,                          // Score achieved
    submitted_at: u64,                   // Submission timestamp
}
```

### LearnerStats

```rust
struct LearnerStats {
    courses_enrolled: u32,               // Total enrolled courses
    courses_completed: u32,              // Courses with credential eligibility
    total_quizzes_submitted: u32,        // Quizzes submitted across all courses
    total_quiz_score: u64,               // Sum of all quiz scores
    average_score: u32,                  // Average quiz score (floored)
    total_rewards_earned: i128,          // Reward tokens at BASE_REWARD_PER_POINT per point
}
```

### ClaimRecord

```rust
struct ClaimRecord {
    course_id: Symbol,                   // Course the quiz belonged to
    quiz_id: Symbol,                     // Quiz that was claimed
    amount: i128,                        // Reward amount minted
    timestamp: u64,                      // Claim timestamp
}
```

### ContractMetadata

```rust
struct ContractMetadata {
    name: SorobanString,                 // Contract name (e.g. "learn-token")
    version: SorobanString,              // Semantic version (e.g. "1.0.0")
}
```

## Events

### progress-tracker

| Event | Topics | Data | Description |
|---|---|---|---|
| `course_created` | — | `(course_id, total_modules, total_quizzes, module_ids)` | New course registered. |
| `enrolled` | — | `(learner, course_id, enrolled_at)` | Learner enrolled. |
| `module_completed` | — | `(learner, course_id, module_id, overall_progress)` | Module marked complete. |
| `quiz_submitted` | — | `(learner, course_id, quiz_id, score)` | Quiz score submitted. |
| `quiz_retaken` | — | `(learner, course_id, quiz_id, previous_score, new_score)` | Quiz retaken with higher score. |
| `credential_eligible` | — | `(learner, course_id)` | Learner became eligible for credential. |
| `course_archived` | — | `(course_id,)` | Course archived. |
| `content_hash_set` | — | `(course_id, content_hash)` | Content hash updated. |
| `course_difficulty_set` | — | `(course_id, difficulty)` | Difficulty updated. |
| `course_tags_set` | — | `(course_id, tags)` | Tags updated. |

### learn-token

| Event | Topics | Data | Description |
|---|---|---|---|
| `reward` | `[reward, learner, course_id]` | `(quiz_id, score, reward_amount)` | Reward claimed. |
| `transfer` | `[transfer, from, to]` | `(amount,)` | Tokens transferred. |
| `transfer_from` | `[transfer_from, from, to]` | `(spender, amount)` | Delegated transfer. |
| `burn` | `[burn, from]` | `(amount,)` | Tokens burned. |
| `burn_from` | `[burn_from, from]` | `(spender, amount)` | Delegated burn. |
| `mint` | `[mint, to]` | `(amount,)` | Tokens minted. |
| `approve` | `[approve, owner, spender]` | `(amount, expiration_ledger)` | Allowance set. |
| `allowance_expired` | `[allowance_expired, owner, spender]` | `(expiration_ledger,)` | Expired allowance accessed. |
| `restriction_updated` | `[restriction_updated]` | `(restriction,)` | Transfer restriction changed. |
| `whitelist_updated` | `[whitelist_updated, address]` | `(added,)` | Whitelist changed. |
| `snapshot_created` | `[snapshot_created]` | `(ledger_height,)` | Balance snapshot created. |
| `upgraded` | `[upgraded]` | `(new_wasm_hash, upgrade_version)` | Contract upgraded. |
| `role_granted` | `[role_granted, address]` | `(role,)` | Admin role granted. |
| `role_revoked` | `[role_revoked, address]` | `(role,)` | Admin role revoked. |
| `paused` | `[paused]` | `(admin, timestamp)` | Contract paused. |
| `unpaused` | `[unpaused]` | `(admin, timestamp)` | Contract unpaused. |
| `vesting_created` | `[vesting_created, beneficiary]` | `(total_amount, cliff_timestamp, duration_seconds)` | Vesting schedule created. |
| `vesting_claimed` | `[vesting_claimed, beneficiary]` | `(claimed_amount, total_claimed)` | Vested tokens claimed. |
| `proposal_created` | `[proposal_created]` | `(proposal_id, start_time, end_time)` | Governance proposal created. |
| `vote_cast` | `[vote_cast, voter]` | `(proposal_id, choice, voting_power)` | Vote cast. |
| `proposal_executed` | `[proposal_executed]` | `(proposal_id, winning_choice, winning_votes)` | Proposal executed. |
| `max_supply_updated` | `[max_supply_updated]` | `(old_max_supply, new_max_supply)` | Supply cap changed. |
| `admin_transfer_initiated` | `[admin_transfer_initiated, new_admin]` | `(current_admin, initiated_at, accept_after)` | Admin transfer started. |
| `admin_transfer_accepted` | `[admin_transfer_accepted, new_admin]` | `(previous_admin,)` | Admin transfer accepted. |
| `admin_transfer_cancelled` | `[admin_transfer_cancelled, new_admin]` | `(current_admin,)` | Admin transfer cancelled. |
| `admin_transfer_delay_updated` | `[admin_transfer_delay_updated]` | `(old_delay_seconds, new_delay_seconds)` | Delay changed. |

### credential-nft

| Event | Topics | Data | Description |
|---|---|---|---|
| `credential_minted` | — | `(learner, course_id, credential_id, score, metadata_uri)` | Credential minted. |
| `credential_revoked` | — | `(learner, course_id, credential_id, admin)` | Credential revoked. |
| `credential_revoked` (with reason) | — | `(learner, course_id, credential_id, admin, reason)` | Credential revoked with reason. |
| `credential_renewed` | — | `(credential_id, new_expiry)` | Credential expiration renewed. |
| `credential_metadata_updated` | — | `(credential_id, new_metadata_uri)` | Metadata URI updated. |
| `credential_display_set` | — | `(credential_id,)` | Display properties set. |
| `certificate_generated` | `[certificate_generated, learner, course_id]` | `(cert_uri,)` | Certificate URI generated. |
## Documentation

- [Architecture](./docs/architecture.md) - Contract relationships, data flow, and storage layout
- [Security Model](./docs/security.md) - Authorization, trust boundaries, and threat mitigation
- [Upgrade Guide](./docs/upgrade-guide.md) - Procedures for upgrading contracts
- [Integration Guide](./docs/integration-guide.md) - SDK setup and contract interaction patterns

## License

MIT
