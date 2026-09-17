# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Role-Based Admin Access Control**: Multi-admin architecture with granular `AdminRole` variants (SuperAdmin, Minter, CourseManager, Pauser) and delayed admin transfer mechanics (#369, #371).
- **Governance & Token Vesting**: `VestingSchedule` data structures with cliff enforcement, linear distribution curves, and governance proposal voting (#366, #384).
- **Contract Upgradeability**: Timelocked wasm upgrade entrypoint with hash verification and security checks (#362, #371).
- **Emergency Pause Mechanism**: Contract-wide emergency pause and unpause controls with event telemetry (#363, #384).
- **SEP-41 Token Burning & Supply Caps**: Added `burn()` and `burn_from()` entrypoints, configurable `set_max_supply()`, and `max_supply_updated` event (#368).
- **Simulation & Gas Estimation**: Added `preview_claim_reward()` and `estimate_claim_gas()` for pre-flight off-chain simulation without gas waste (#362, #371).
- **Batch Operations**: Added `submit_quiz_scores_batch()` in progress-tracker and batch claim reward processing (#371, #372).
- **Content Integrity Verification**: `enroll_checked()` entrypoint verifying course content SHA-256 hashes on enrollment (#363, #384).
- **Prerequisites & Course Versioning**: Module prerequisite chains, course versioning, quiz retake policies, and learner completion metrics (#364, #376).
- **Soulbound Rejection Enforcement**: Explicit typed `ContractError::Soulbound (1)` on credential `transfer()` calls (#374, #378).
- **Security & Invariant Test Suite**: End-to-end unit and integration suites covering arithmetic underflow/overflow, reentrancy guards, double-spending attacks, and authorization boundaries (#379, #383, #386).
- **Developer Documentation**: Comprehensive guides covering deployment (`docs/deployment-guide.md`), troubleshooting (`docs/troubleshooting.md`), testing (`docs/testing-guide.md`), and contributions (`CONTRIBUTING.md`).

### Changed
- **Cross-Contract Optimization**: Reduced cross-contract call overhead when checking quiz scores and credential eligibility (#375).
- **Storage Hygiene**: Moved token allowances from persistent to temporary storage with automated TTL extension (#110).
- **Credential Minting Validation**: Enforced strict validation matching caller score against on-chain course average score (#108).
- **Event Schemas**: Expanded event payloads in `module_completed`, `credential_minted`, and `claim_reward` with indexed topics for indexer efficiency (#123).

### Fixed
- Fixed integer division documentation in `README.md` to clarify floor rounding behavior (#130).
- Fixed signature documentation for `claim_reward(learner, course_id, quiz_id)` across README and examples (#124).
- Prevented duplicate reward claims across batch invocations with idempotent execution semantics.

## [1.0.0] - Initial Release

### Added
- **learn-token**: SEP-41 compliant fungible token contract with quiz reward minting
  - `initialize()` - Contract setup with admin and metadata
  - `mint()` - Mint new reward tokens (admin only)
  - `transfer()`, `approve()`, `balance()` - Standard token operations
  - `claim_reward()` - Learner reward minting based on quiz scores
  - Anti-fraud: Each quiz can only be claimed once per learner

- **credential-nft**: Non-transferable credential NFT contract for course completion
  - `initialize()` - Contract setup with admin and progress-tracker reference
  - `mint_credential()` - Mint credentials for completed courses (score-gated at 50+)
  - `verify_credential()` - Retrieve full credential information
  - `get_credentials_for()` - Paginated learner credential lookup
  - `get_credential_count()` - Total credentials per learner
  - `revoke_credential()` - Admin credential revocation
  - `transfer()` - Explicitly rejects all transfers (soulbound enforcement)
  - Reverse lookup: `get_credentials_by_course()` and `get_total_credentials_count()`

- **progress-tracker**: On-chain learning progress tracking and eligibility calculation
  - `initialize()` - Contract setup with admin
  - `create_course()` - Register courses with modules and quizzes
  - `enroll()` - Learner course enrollment
  - `complete_module()` - Mark module completion (sequential enforcement)
  - `submit_quiz_score()` - Record quiz results with duplicate prevention
  - `get_progress()` - Retrieve learner progress info
  - `get_quiz_score()` - Verify individual quiz results
  - `is_eligible_for_credential()` - Check credential eligibility
  - Weighted progress calculation: 70% modules + 30% quizzes
  - Credential eligibility event: `credential_eligible` emitted on false → true flip

- **shared**: Utility package with contract-wide constants
  - `MIN_CREDENTIAL_SCORE` (50) - Minimum passing score
  - `MAX_QUIZ_SCORE` (100) - Quiz score ceiling
  - `TOKEN_DECIMALS` (7) - Token decimal places
  - `BASE_REWARD_PER_POINT` (100) - Reward tokens per quiz point
  - `MAX_CREDENTIALS_PAGE_SIZE` (50) - Pagination limit

### Features
- Comprehensive test coverage for all contracts
- Event emission for indexing and off-chain tracking
- Admin authorization on state-changing operations
- Completion-gated credential minting with progress-tracker verification
- Soulbound (non-transferable) credentials permanently bound to earners
- Efficient storage: single reads, no duplicate data structures
- Sequential module ordering enforcement
- Double-claim prevention on token rewards and quiz submissions
