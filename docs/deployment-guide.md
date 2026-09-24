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
   soroban contract invoke --id <PROGRESS_TRACKER_ID> --network testnet --source <STELLAR_SECRET_KEY> -- admin
   ```
3. **Cross-Contract Verification**: Ensure that the `learn-token` and `credential-nft` contracts return the correct `progress-tracker` ID.

## Troubleshooting

- **RPC Reachability Error**: Ensure the network RPC URL is correct and your internet connection is stable.
- **Insufficient Funds**: The deployment account must have enough XLM to cover transaction fees and minimum balance requirements.
- **Missing WASM Error**: Ensure you have successfully run `cargo build --release --target wasm32-unknown-unknown`.
- **Initialization Fails**: Ensure `deploy.sh` completed successfully and the `deployments-<network>.json` file is correctly formatted.
