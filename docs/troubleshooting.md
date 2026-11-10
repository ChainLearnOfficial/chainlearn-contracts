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

The project requires Rust 1.81.0+, the `wasm32-unknown-unknown` target, and
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
workspace declares Rust 1.81.0 as its minimum. After switching toolchains, run:

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

## WASM size and contract optimization

Soroban enforces a network-level maximum contract binary size (`contract_max_size_bytes`) and charges both upload fees and ongoing ledger rent per byte. If a build approaches the size limit or CI flags a WASM size regression, use the workflow below. For full benchmark tables and section breakdowns, see the [Contract WASM Size Audit](wasm-size-audit.md).

### Measuring WASM size

Run the size measurement script from the repository root:

```bash
./scripts/measure-size.sh
```

This script builds the release WASM binaries (`target/wasm32-unknown-unknown/release/*.wasm`), runs `./scripts/optimize-wasm.sh` both with and without `--strip-docs`, and prints a comparison table of raw, `wasm-opt`-optimized, and doc-stripped byte counts for `learn-token`, `credential-nft`, and `progress-tracker`.

To inspect which WASM sections (`code`, `contractspecv0`, `data`, `export`) or functions consume the most space, see the section breakdown methodology in [`docs/wasm-size-audit.md`](wasm-size-audit.md#where-the-bytes-go).

### Common causes of WASM size increases

1. **Verbose `///` doc comments on public contract items (`contractspecv0` bloat)**:
   `#[contractimpl]` and `#[contracttype]` embed every `///` doc comment on public functions, parameters, structs, and enums directly into the on-chain `contractspecv0` custom section (frequently 34–45% of the entire binary).
2. **Formatted string panics (`core::fmt` machinery)**:
   Using `panic!("{} ...", val)` or `Option::expect` pulls `core::fmt` formatting tables (~4.5 KB per contract) into the binary even though the Soroban host panic handler discards formatted strings.
3. **Unoptimized raw Cargo output or debug symbols**:
   Uploading `target/wasm32-unknown-unknown/release/<contract>.wasm` directly without running `stellar contract optimize` (`wasm-opt`) or without `strip = "symbols"` leaves 17–24% of removable dead code and symbol metadata in the binary.
4. **Unused dependencies or heavy feature flags**:
   Adding crates to `Cargo.toml` with default `std` or formatting features enabled increases code size across all monomorphized entry points.
5. **Large `#[contracttype]` structs and 128-bit math**:
   Monomorphized `FromVal`/`IntoVal` conversions for large structs and software-emulated `i128`/`u128` division (`compiler_builtins::u128_div_rem`) add significant code size.

### Optimization techniques

- **Always run the optimizer pass before deploying or verifying**:
  ```bash
  ./scripts/optimize-wasm.sh
  ```
  `deploy.sh`, `upgrade.sh`, and `verify.sh` automatically invoke `scripts/optimize-wasm.sh` to produce `<name>.optimized.wasm`.
- **Strip on-chain spec documentation when appropriate**:
  ```bash
  ./scripts/optimize-wasm.sh --strip-docs
  # or during deployment:
  STRIP_SPEC_DOCS=1 ./scripts/deploy.sh testnet
  ```
  Stripping doc strings from `contractspecv0` saves 10–26 KB per contract while preserving function names, argument types, and return types for `stellar contract invoke` and client bindings. Make sure `./scripts/verify.sh` is run with the same `STRIP_SPEC_DOCS` setting used at deployment time.
- **Prefer typed `#[contracterror]` enums over string panics**:
  Use `panic_with_error!(&env, ContractError::...)` instead of formatted `panic!(...)` strings to avoid pulling `core::fmt` into the WASM table.
- **Deduplicate shared contract logic and use Soroban host helpers**:
  Factor repeated validation/storage code into private helper functions and prefer host-provided methods (such as `Vec::first_index_of`) over guest-side loops.
- **Keep the workspace release profile intact**:
  Ensure `[profile.release]` in the root `Cargo.toml` retains `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, and `strip = "symbols"`.

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
