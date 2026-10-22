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