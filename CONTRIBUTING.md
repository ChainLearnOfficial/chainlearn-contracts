# Contributing to ChainLearn Contracts

Thank you for helping improve ChainLearn. This repository contains Soroban smart
contracts, so every change should be reviewed with security, compatibility, and
on-chain upgradeability in mind.

## Development setup

### Prerequisites

- Rust **1.70 or later** (the workspace minimum supported Rust version is 1.70)
- The `wasm32-unknown-unknown` Rust target
- Git
- Soroban CLI **v21+** when you need to deploy or invoke a contract locally or
  on a network

Install the Rust target and, if needed, the Soroban CLI:

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked soroban-cli --version 21.0.0
```

### Clone and verify the workspace

```bash
git clone https://github.com/ChainLearnOfficial/chainlearn-contracts.git
cd chainlearn-contracts
cargo build --release --target wasm32-unknown-unknown
cargo test --workspace
```

The release build produces the contract WASM artifacts in
`target/wasm32-unknown-unknown/release/`. Do not commit generated `target/`
files or deployment secrets.

### Repository layout

- `contracts/learn-token/` — SEP-41 reward token contract.
- `contracts/credential-nft/` — soulbound course credential contract.
- `contracts/progress-tracker/` — learner progress and course state.
- `packages/shared/` — common types and constants shared by contracts.
- `tests/unit/` and `tests/integration/` — workspace test suites.
- `docs/` — architecture, deployment, security, and operational documentation.

Read the [README](README.md), [architecture guide](docs/architecture.md), and
[security guide](docs/security.md) before changing a contract interface or
storage behavior.

## Code style and contract expectations

1. Format Rust before committing:

   ```bash
   cargo fmt --all
   ```

2. Run Clippy with the same warnings-as-errors policy used by CI:

   ```bash
   cargo clippy --workspace --all-targets -- -D warnings
   ```

3. Keep changes focused. Use descriptive names, small functions where practical,
   and Rustdoc for public contract APIs and non-obvious behavior.
4. Preserve authorization checks on every state-changing entrypoint. Be explicit
   about the required signer and test unauthorized calls.
5. Treat storage layouts, public function signatures, events, and error values
   as compatibility-sensitive. Document migrations and upgrade implications when
   changing them.
6. Avoid unchecked arithmetic and unbounded storage growth. Validate user input,
   enforce project limits, and prefer the project’s existing error and storage
   patterns.
7. Update relevant documentation and `CHANGELOG.md` in the same pull request,
   including a **Breaking Changes** entry when users must alter integrations,
   configuration, or deployed state.

## Testing requirements

All pull requests must pass the checks run by CI:

```bash
cargo build --release --target wasm32-unknown-unknown
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Add or update tests for every behavior change. Contract changes must cover the
successful path, authorization failures, validation and boundary conditions,
and relevant error paths. Changes that cross contract boundaries must include
an integration test covering the affected flow.

For coverage work, install `cargo-tarpaulin` and run:

```bash
./scripts/coverage.sh
```

The testing guide sets a 90% instruction-coverage target for core contracts and
requires explicit edge-case, authorization, overflow, and cross-contract tests.
Use `./scripts/estimate-gas.sh` when a change may affect execution cost, and
`./scripts/measure-size.sh` when it may affect WASM size.

## Pull request process

1. Check existing issues and pull requests, then open an issue for a new feature,
   security concern, or behavior change before investing in a large patch.
2. Create a branch from the current `main` branch and make one focused change.
3. Make the code, tests, documentation, and changelog updates together. Do not
   include unrelated formatting, generated files, secrets, or deployment keys.
4. Run the required local checks above and resolve all failures before opening a
   pull request.
5. Open a pull request against `main` with a concise title and a description
   that includes:
   - what changed and why;
   - affected contracts, public APIs, storage, events, and deployment behavior;
   - tests and checks run;
   - security considerations and any gas or WASM-size impact;
   - linked issue(s); and
   - a clearly labeled **Breaking Changes** section, or `None` when there are
     no breaking changes.
6. Respond to review feedback with follow-up commits. Keep the pull request
   green, and do not merge until required review and CI checks have completed.

## Reporting security issues

Do not disclose a suspected vulnerability in a public issue. Follow the private
reporting instructions in the [security guide](docs/security.md) so maintainers
can investigate and coordinate a fix responsibly.
