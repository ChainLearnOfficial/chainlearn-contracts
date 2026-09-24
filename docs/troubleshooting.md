# Troubleshooting Guide

This guide covers common local-development, build, test, deployment, and
contract-interaction problems. Start with the command output: Soroban and Cargo
errors usually identify the contract, argument, or environment that failed.

## Quick diagnostics

Run these checks from the repository root before investigating a specific issue:

```bash
rustc --version
cargo --version
rustup target list --installed
soroban --version
cargo test --workspace
```

The project requires Rust 1.70+, the `wasm32-unknown-unknown` target, and
Soroban CLI v21+ for deployment and network interaction. For deployed
environments, run `./scripts/monitor.sh testnet` or
`./scripts/monitor.sh mainnet` to check RPC reachability, contract
initialization, admin configuration, and learn-token supply access.

## Build and test errors

### `can't find crate for core` or missing `wasm32-unknown-unknown`

The WebAssembly target is not installed for the active Rust toolchain.

```bash
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
```

If it still fails, confirm `rustup show active-toolchain` points to the
toolchain where you installed the target, then install it again for that
toolchain.

### Dependency, compiler, or Soroban SDK version conflict

Use the repository lockfile and the supported Rust version rather than updating
dependencies ad hoc. Check the active compiler with `rustc --version`; the
workspace declares Rust 1.70 as its minimum. After switching toolchains, run:

```bash
cargo clean
cargo test --workspace
```

Only change `Cargo.toml` or `Cargo.lock` as part of an intentional dependency
update, with its tests and security review.

### A test panics with a contract error

Run the smallest failing suite with output enabled:

```bash
cargo test --test <test_name> -- --nocapture
# or
cargo test -p <package_name> -- --nocapture
```

Compare the test’s setup with the entrypoint’s authorization and validation
rules. In particular, verify that test clients use the expected signer, that
contracts are initialized before use, and that any cross-contract dependency is
registered in the Soroban test environment. See the [testing guide](testing-guide.md)
for test setup and mock patterns.

### Formatting or Clippy fails

Use the same commands as CI:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

Do not silence a warning without understanding its contract or security impact.
Prefer a small code correction and add a regression test if the warning exposed
incorrect behavior.

## Deployment and invocation errors

### `Deployment file ... not found`

`monitor.sh` and `initialize.sh` read `deployments-testnet.json` or
`deployments-mainnet.json`. Deploy the selected network first:

```bash
./scripts/deploy.sh testnet
./scripts/initialize.sh testnet
```

Use the same network name for deployment, initialization, monitoring, and every
manual invocation. Deployment files identify the three contracts that must be
wired together.

### RPC unreachable, timeout, or wrong network

Check your internet connection and confirm the RPC URL and network passphrase
match the target network. The monitor script performs this reachability check:

```bash
./scripts/monitor.sh testnet
```

Retry transient RPC errors. For persistent errors, verify the endpoint’s status
and avoid submitting state-changing transactions until the intended network and
contract IDs are confirmed.

### Authorization failed

State-changing methods require the relevant account’s authorization. Confirm
that `--source` references the expected account, the account has sufficient XLM
for fees, and the caller has the required role. For example, course management,
token minting, and credential revocation require admin authority; learner
actions require the learner’s authorization.

Never paste a secret key into a command history, issue, pull request, or log.
Use an environment variable such as `STELLAR_SECRET_KEY` for local scripts and
rotate a key that may have been exposed.

### Contract reports `NotInitialized` or health check reports no admin

Initialize contracts after deployment and in dependency order:

```bash
./scripts/initialize.sh testnet
./scripts/monitor.sh testnet
```

The progress tracker must be initialized before learn-token and credential-nft,
because both store and query its contract address. Do not re-run initialization
against a contract that is already initialized; initialization is one-time.

### Reward claim or credential mint is rejected

For a reward, ensure the learner submitted the referenced quiz score and has not
already claimed the reward for that quiz. For a credential, ensure the learner
completed every module, submitted every quiz, and has an average score of at
least 50. The supplied credential score must match the score verified from the
progress tracker. Review the [README](../README.md#credential-eligibility) for
the eligibility rules.

## Debugging tips

- Reproduce against testnet or the local test environment before making a
  mainnet change.
- Run a single test with `--nocapture` and minimize it to the failing contract
  call, signer, and inputs.
- Inspect generated WASM and size after a release build with
  `./scripts/measure-size.sh`; use `./scripts/estimate-gas.sh` to catch relative
  CPU-instruction regressions.
- For a deployed system, record the network, contract ID, ledger or transaction
  identifier, invoked function, sanitized arguments, and exact error. Do not
  include secret keys.
- Check on-chain read methods such as `is_initialized`, `admin`, and
  `total_supply` before assuming a deployment is healthy; `monitor.sh` invokes
  these checks for all three contracts.

## FAQ

### Which commands must pass before I open a pull request?

Run the release WASM build, workspace tests, Clippy with warnings denied, and
format check. The exact commands are in [CONTRIBUTING.md](../CONTRIBUTING.md).

### Can I deploy directly to mainnet while developing?

No. Validate changes with unit and integration tests, then use testnet and the
deployment procedures first. Mainnet deployment requires the operational and
security review appropriate for an immutable on-chain change.

### Why does `monitor.sh` require `jq`?

It reads contract IDs from the network deployment JSON file. Install `jq`, then
rerun the command. The script also requires Soroban CLI and a prior deployment.

### Where should I report a vulnerability?

Use the private reporting route described in the [security guide](security.md),
not a public issue or pull request.
