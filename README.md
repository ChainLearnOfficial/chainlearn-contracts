# ChainLearn Contracts

A Soroban smart contract workspace for a Stellar-based learning platform. ChainLearn enables on-chain learning progress tracking, quiz-based token rewards, and verifiable credential NFTs.

## Architecture

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

## License

MIT
