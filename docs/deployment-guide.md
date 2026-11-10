# Deployment Guide

This document outlines the procedure for deploying ChainLearn contracts to different networks. Deploying contracts requires careful execution to ensure a secure and functional production environment.

## Prerequisites

Before deploying, ensure the following are available:

1. **Soroban CLI**: Version 21+ must be installed.
2. **jq**: Must be installed for JSON processing.
3. **Stellar Admin Secret Key**: Export your secret key:
   ```bash
   export STELLAR_SECRET_KEY="S..."
   ```
4. **Build Target**: Contracts must be compiled for WASM.
   ```bash
   cargo build --release --target wasm32-unknown-unknown
   ```

## Deployment Steps

Deployments are automated using the `scripts/deploy.sh` script.

To deploy all contracts to the testnet:
```bash
./scripts/deploy.sh testnet
```

To deploy all contracts to the mainnet:
```bash
./scripts/deploy.sh mainnet
```

### What the deploy script does:
1. Builds the WASM files for all contracts.
2. Validates network connectivity and admin credentials.
3. Deploys `learn-token`, `credential-nft`, and `progress-tracker` to the network.
4. Generates a `deployments-<network>.json` file containing the contract IDs.

### Initialization

After deployment, the contracts must be initialized and linked together using the `scripts/initialize.sh` script.

```bash
./scripts/initialize.sh testnet
```
This script configures cross-contract references (e.g., wiring the `progress-tracker` to the `learn-token`).

## Verification Steps

After initialization, verify the deployment:

1. **Check Deployment File**: Ensure `deployments-<network>.json` exists and contains valid Stellar Contract IDs starting with `C`.
2. **On-Chain Verification**: Use the Soroban CLI to invoke read functions on the deployed contracts to verify their state.
   ```bash
   soroban contract invoke \
       --id <PROGRESS_TRACKER_ID> \
       --rpc-url <RPC_URL> \
       --network-passphrase "<NETWORK_PASSPHRASE>" \
       --source <STELLAR_SECRET_KEY> \
       -- admin
   ```
3. **Cross-Contract Verification**: Ensure that the `learn-token` and `credential-nft` contracts return the correct `progress-tracker` ID.

## Troubleshooting

- **RPC Reachability Error**: Ensure the network RPC URL is correct and your internet connection is stable.
- **Insufficient Funds**: The deployment account must have enough XLM to cover transaction fees and minimum balance requirements.
- **Missing WASM Error**: Ensure you have successfully run `cargo build --release --target wasm32-unknown-unknown`.
- **Initialization Fails**: Ensure `deploy.sh` completed successfully and the `deployments-<network>.json` file is correctly formatted.
 
## Deployment JSON Format

Each deployment writes a deployments-<network>.json file at the repository root. The
file is a map of contract names to deployment records.

A record has this shape:

~~~json
{
  "learn-token": {
    "contract_id": "C...",
    "wasm_hash": "hex-encoded hash",
    "network": "testnet",
    "deployed_at": "2026-10-22T12:00:00Z"
  }
}
~~~

### Fields

- contract_id: the Stellar contract address beginning with C.
- wasm_hash: the hash of the WASM artifact used for the deployment.
- network: the target network name, such as testnet or mainnet.
- deployed_at: the UTC timestamp recorded after deployment.
- The contract name is the JSON property name and must match the name used by
  the deployment and initialization scripts.

The exact set of contract names can grow as new protocol modules are added, but
each record should retain the same fields so scripts can resolve addresses and
verify artifacts consistently.

## Upgrade History

When a contract is upgraded, preserve the current record and append an upgrades
array:

~~~json
{
  "progress-tracker": {
    "contract_id": "C...",
    "wasm_hash": "new-hash",
    "network": "testnet",
    "deployed_at": "2026-10-22T12:00:00Z",
    "upgrades": [
      {
        "wasm_hash": "previous-hash",
        "upgraded_at": "2026-10-01T12:00:00Z"
      }
    ]
  }
}
~~~

Newest deployment metadata remains at the top level. Each history entry records
the previous artifact hash and the UTC time at which that version was replaced.

## Recovery From a Corrupted File

1. Stop deployment and initialization commands before editing the file.
2. Copy the damaged file to a separate backup for incident review.
3. Validate that the file is valid JSON and that every contract_id has the
   expected C... format.
4. Recover the last known-good file from version control or the deployment
   artifact archive.
5. Compare every contract address and WASM hash against on-chain verification
   before running an upgrade or initialization.
6. If no trusted backup exists, recreate the file manually from verified
   deployment records rather than guessing an address.
7. Run the normal verification step for the target network after recovery.

Do not delete a deployment record merely because a contract is not currently
used by the initialization script. Historical records are needed to audit
upgrades and to prevent a later deployment from being mistaken for the original.

## Manual Creation

To create a deployment file manually, create an empty JSON object and add one
verified record per contract:

~~~json
{
  "learn-token": {
    "contract_id": "C...",
    "wasm_hash": "verified-artifact-hash",
    "network": "testnet",
    "deployed_at": "2026-10-22T12:00:00Z"
  }
}
~~~

Use the exact contract IDs and artifact hashes returned by the deployment
commands. Keep the file name aligned with the network: for example,
deployments-testnet.json and deployments-mainnet.json. Never commit secret
keys, signing material, or full transaction envelopes to the deployment file.

## Mainnet Safety Considerations

Deploying or upgrading contracts on Stellar Mainnet is an high-impact operational action. Unlike testnet, mainnet transactions incur real XLM costs, state changes are permanent on the public ledger, and downstream integrators depend on stable contract addresses and interfaces.

### Mainnet Pre-Deployment Checklist

Complete every item below before running `./scripts/deploy.sh mainnet` or `./scripts/upgrade.sh mainnet`:

- [ ] **Quality Gates Green**:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo deny check` and `cargo audit`
- [ ] **WASM Size & Gas Verification**:
  - `./scripts/measure-size.sh` confirms all three optimized binaries (`learn-token`, `credential-nft`, `progress-tracker`) are well within Soroban's `contract_max_size_bytes` limit.
  - `./scripts/estimate-gas.sh` passes all CPU instruction thresholds in `scripts/gas-thresholds.env`.
  - Confirm whether `STRIP_SPEC_DOCS=1` is enabled for the release and document the choice so `./scripts/verify.sh mainnet` uses the matching flag.
- [ ] **Testnet Dry-Run Parity**:
  - The exact release commit has been deployed (`./scripts/deploy.sh testnet`), initialized (`./scripts/initialize.sh testnet`), verified (`./scripts/verify.sh testnet`), and health-checked (`./scripts/monitor.sh testnet`) on testnet.
- [ ] **Admin & Key Readiness**:
  - Mainnet deployer and admin accounts are funded, verified, and backed by hardware or multisig controls.
  - `deployments-mainnet.json` from the previous release is backed up and clean in version control.

### Cost Estimation Guidance

Mainnet deployment and operation incur three distinct XLM cost categories on Soroban:

1. **WASM Code Upload & Byte Cost**:
   Uploading each contract's `.optimized.wasm` binary charges network inclusion fees plus ledger write fees proportional to binary size (~193 KB total across all three contracts by default, or ~136 KB with `STRIP_SPEC_DOCS=1`; see [`docs/wasm-size-audit.md`](wasm-size-audit.md)).
2. **Contract Instance & Code Ledger Rent**:
   Both the uploaded WASM code entry and each deployed contract instance (`progress-tracker`, `learn-token`, `credential-nft`) require ongoing ledger rent (TTL extensions). Keep extra XLM headroom on the operations account for periodic TTL bump transactions.
3. **Initialization & Invocation Execution Costs**:
   `initialize.sh` invokes `initialize` on all three contracts and wires cross-contract addresses. Estimate CPU instruction and read/write footprint beforehand using:
   ```bash
   ./scripts/measure-size.sh
   ./scripts/estimate-gas.sh
   ```
   Maintain at least **50–100 XLM** of uncommitted buffer in the deployer account above the minimum account reserve to absorb fee-surge multipliers during network congestion.

### Rollback Planning

Because Soroban contract storage is persistent on-chain, a mainnet rollback must be planned before any upgrade or deployment:

1. **Preserve Previous WASM Hashes**:
   Verify that `deployments-mainnet.json` records the currently active `wasm_hash` for every contract (and retains prior entries in the `upgrades` array) before running an upgrade.
2. **Immediate Emergency Mitigation (Pause)**:
   If a post-deployment anomaly is detected, immediately pause state-changing operations on affected contracts while investigating rather than pushing an untested hotfix.
3. **Contract Code Rollback (`upgrade.sh`)**:
   If an upgrade introduces a regression and storage layout compatibility has been preserved, roll back by re-uploading or referencing the previous verified `wasm_hash` recorded in `deployments-mainnet.json`'s `upgrades` history and verifying on-chain bytecode parity with `./scripts/verify.sh mainnet`.
4. **Storage Schema Compatibility Rule**:
   Never deploy a mainnet upgrade that mutates existing storage key encodings in place without a tested migration path, as incompatible storage changes prevent clean WASM code rollback.

### Communication and Coordination Plan

Coordinate every mainnet deployment or upgrade across engineering, operations, and downstream consumers:

1. **Pre-Deployment Notice (T-24h)**:
   Announce the scheduled maintenance window, target commit SHA, expected contract upgrades, and whether any ABI (`contractspecv0`) or event schema changes are included.
2. **Execution Window (T-0)**:
   - Freeze merges to `main` until deployment and verification complete.
   - Run `./scripts/deploy.sh mainnet` (and `./scripts/initialize.sh mainnet` for initial deployments).
   - Run `./scripts/verify.sh mainnet` and `./scripts/monitor.sh mainnet` immediately after completion.
3. **Post-Deployment Handoff**:
   - Commit the updated `deployments-mainnet.json` (never include secret keys) and tag the release in Git.
   - Share the verified `contract_id`s and `wasm_hash`es with frontend, indexer, and API operators.
   - Monitor RPC health and contract events for at least 60 minutes post-deployment.

### Multi-Operator Deployment Guidance

To eliminate single-point-of-failure key risk on mainnet, separate duties across multiple operators:

1. **Separation of Deployer and Admin Authority**:
   Use a dedicated, short-lived or low-privilege deployer account for uploading WASM bytes (`deploy.sh`), and transfer or configure contract administrative authority (`admin`) to a multi-signature account rather than a single developer workstation key.
2. **Two-Operator Verification Protocol**:
   - **Operator A** executes the build and deployment from a clean checkout of the tagged commit and records the resulting `deployments-mainnet.json`.
   - **Operator B** independently builds the tagged commit on a separate machine and runs `./scripts/verify.sh mainnet` and `./scripts/monitor.sh mainnet` to confirm that the on-chain WASM hashes, admin addresses, and cross-contract links (`progress-tracker` address stored in `learn-token` and `credential-nft`) match the expected release artifacts.
3. **Secret Hygiene**:
   Inject signing credentials exclusively via ephemeral environment variables (`STELLAR_SECRET_KEY`) or hardware-backed Soroban identity profiles (`stellar keys`). Revoke or rotate temporary deployment keys once control is verified on the admin multisig.

