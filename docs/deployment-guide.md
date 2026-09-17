# ChainLearn Smart Contracts Deployment Guide

This guide covers end-to-end instructions for deploying and verifying ChainLearn smart contracts (`learn-token`, `credential-nft`, and `progress-tracker`) on the Stellar network (Testnet and Mainnet).

---

## Table of Contents
1. [Prerequisites](#prerequisites)
2. [Network Configurations](#network-configurations)
3. [Building the Contracts](#building-the-contracts)
4. [Automated Deployment](#automated-deployment)
5. [Manual Step-by-Step Deployment](#manual-step-by-step-deployment)
6. [Post-Deployment Initialization](#post-deployment-initialization)
7. [On-Chain Verification](#on-chain-verification)
8. [Troubleshooting & FAQ](#troubleshooting--faq)

---

## Prerequisites

Before deploying, ensure you have the required tooling installed:
- **Rust Toolchain**: `1.79.0+`
  ```bash
  rustup target add wasm32-unknown-unknown
  ```
- **Soroban CLI**: `v21.0.0+`
  ```bash
  cargo install --locked soroban-cli
  ```
- **System Utilities**: `jq` and `curl`
  ```bash
  # Debian / Ubuntu
  sudo apt install jq curl
  # macOS
  brew install jq curl
  ```
- **Funded Stellar Secret Key**:
  - Export your deployer account secret key:
    ```bash
    export STELLAR_SECRET_KEY="S..."
    ```
  - For Testnet, fund your account using Friendbot:
    ```bash
    curl "https://friendbot.stellar.org?addr=<YOUR_PUBLIC_KEY>"
    ```

---

## Network Configurations

| Parameter | Testnet | Mainnet |
| :--- | :--- | :--- |
| **RPC Endpoint** | `https://soroban-testnet.stellar.org:443` | `https://soroban-rpc.mainnet.stellar.gateway.fm:443` |
| **Network Passphrase** | `Test SDF Network ; September 2015` | `Public Global Stellar Network ; September 2015` |
| **Explorer** | [Stellar Expert Testnet](https://stellar.expert/explorer/testnet) | [Stellar Expert Public](https://stellar.expert/explorer/public) |

---

## Building the Contracts

Compile all contracts in release mode for WebAssembly:
```bash
cargo build --release --target wasm32-unknown-unknown
```

This outputs the compiled bytecode artifacts:
- `target/wasm32-unknown-unknown/release/learn_token.wasm`
- `target/wasm32-unknown-unknown/release/credential_nft.wasm`
- `target/wasm32-unknown-unknown/release/progress_tracker.wasm`

---

## Automated Deployment

The project provides an automated script `./scripts/deploy.sh` that compiles, deploys, and records contract addresses.

### Deploy to Testnet
```bash
./scripts/deploy.sh testnet
```

### Deploy to Mainnet
```bash
./scripts/deploy.sh mainnet
```

The script produces a deployment record in `deployments-<network>.json`:
```json
{
  "network": "testnet",
  "deployed_at": "2026-09-17T09:00:00Z",
  "contracts": {
    "learn_token": "CA...",
    "credential_nft": "CB...",
    "progress_tracker": "CC..."
  }
}
```

---

## Manual Step-by-Step Deployment

If deploying manually using the Soroban CLI:

### 1. Deploy Learn Token
```bash
soroban contract deploy \
    --wasm target/wasm32-unknown-unknown/release/learn_token.wasm \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE"
```

### 2. Deploy Credential NFT
```bash
soroban contract deploy \
    --wasm target/wasm32-unknown-unknown/release/credential_nft.wasm \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE"
```

### 3. Deploy Progress Tracker
```bash
soroban contract deploy \
    --wasm target/wasm32-unknown-unknown/release/progress_tracker.wasm \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE"
```

---

## Post-Deployment Initialization

Once contracts are deployed, entrypoints must be initialized in topological order:

1. **Initialize Token Contract**:
   ```bash
   soroban contract invoke --id <LEARN_TOKEN_ID> -- initialize \
       --admin <ADMIN_ADDRESS> --name "ChainLearn Token" --symbol "LEARN" --decimals 7
   ```

2. **Initialize Credential NFT**:
   ```bash
   soroban contract invoke --id <CREDENTIAL_NFT_ID> -- initialize \
       --admin <ADMIN_ADDRESS> --name "ChainLearn Credential" --symbol "CLCRED"
   ```

3. **Initialize Progress Tracker**:
   ```bash
   soroban contract invoke --id <PROGRESS_TRACKER_ID> -- initialize \
       --admin <ADMIN_ADDRESS> \
       --token_contract <LEARN_TOKEN_ID> \
       --credential_contract <CREDENTIAL_NFT_ID>
   ```

---

## On-Chain Verification

Verify that deployed contracts are operational and responsive:

1. **Check Token Supply and Metadata**:
   ```bash
   soroban contract invoke --id <LEARN_TOKEN_ID> -- decimals
   ```

2. **Check Course Registration**:
   ```bash
   soroban contract invoke --id <PROGRESS_TRACKER_ID> -- get_course_count
   ```

3. **Inspect Contract State**:
   ```bash
   soroban contract read --id <PROGRESS_TRACKER_ID> --durability instance
   ```

---

## Troubleshooting & FAQ

### 1. `Error: RPC endpoint is not reachable`
- Check internet connectivity and verify network status.
- Ensure the RPC URL is responsive:
  ```bash
  curl -I https://soroban-testnet.stellar.org:443
  ```

### 2. `HostError: Error(Budget, ExceededLimit)`
- Increase transaction resource limits or optimize contract WASM size using `wasm-opt`:
  ```bash
  wasm-opt -Oz -o optimized.wasm target/wasm32-unknown-unknown/release/learn_token.wasm
  ```

### 3. `Error: InsufficientBalance`
- The deployer account does not have sufficient XLM for transaction fees and contract storage reserves.
- Fund the account with additional XLM before retrying.

### 4. `Deployment already exists in deployments-<network>.json`
- Remove or rename the existing `deployments-<network>.json` file if you intend to perform a fresh redeployment.
