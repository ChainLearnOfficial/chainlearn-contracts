# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- MIT `LICENSE` file, referenced from the README and declared in Cargo metadata (#341)
- `CODE_OF_CONDUCT.md` with community standards, reporting and enforcement guidelines (#342)
- GitHub issue templates for bug reports and feature requests, linked from the README (#343)
- `scripts/optimize-wasm.sh` and `docs/wasm-size-audit.md`: deploy/upgrade/verify now use wasm-opt optimized WASM (−17% to −24%), with optional spec doc stripping (up to −48%) (#344)
- `CONTRIBUTING.md` with development setup, code style, testing requirements,
  pull request process, and security-reporting guidance (#340)
- Troubleshooting guide covering setup, build/test, deployment, invocation,
  debugging, and FAQ guidance (`docs/troubleshooting.md`) (#338)
- Monitoring guide covering health checks, operational metrics, alerting rules,
  and incident response (`docs/monitoring-guide.md`) (#337)
- Rustdoc examples on all public contract functions for easier integration
- CHANGELOG.md for tracking version history and breaking changes
- `transfer()` method on credential-nft contract that explicitly rejects transfers, enforcing soulbound credentials
- Documentation improvements for soulbound credential enforcement
- `contract_metadata()` on all three contracts, returning the contract's name and version, stored on `initialize()` (#107)
- `course_exists()` on progress-tracker, letting other contracts validate a `course_id` cheaply (#108)

### Changed
- credential-nft: `revoke_credential` and `revoke_credential_with_reason` share a single implementation and prune indexes with host-side `first_index_of` (#344)

### Removed
- Unused `check_allowance_expired_readonly` helper in learn-token storage (#344)

### Fixed
- `mint_credential()` now rejects `course_id`s that were never registered via `create_course`, instead of only failing indirectly through the eligibility check (#108)
- `is_credential_valid()` no longer deserializes the full `CredentialInfo` struct; it checks existence and a dedicated `revoked` flag instead (#109)
- learn-token allowances now live in temporary storage instead of persistent storage, matching their short-lived, self-expiring nature (#110)
- README progress formula documentation now accurately reflects integer division implementation (#130)
- Credential transfer mechanism now enforces non-transferability (soulbound) (#127)
- `module_completed` event now includes `overall_progress` so indexers don't have to follow up with a `get_progress` call (#123)
- README now documents the actual `claim_reward(learner, course_id, quiz_id)` signature (#124)
- README now documents the cross-contract dependency between `learn-token`/`credential-nft` and `progress-tracker` (#125)
- README now documents that `initialize.sh` passes the `progress-tracker` address to `learn-token` (#126)

### Breaking Changes
- None.

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
