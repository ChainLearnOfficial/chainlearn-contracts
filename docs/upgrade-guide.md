# Upgrade Guide

This document covers the preparation, procedure, verification, and rollback for upgrading ChainLearn smart contracts.

## Overview

ChainLearn contracts support in-place upgrades via Soroban's wasm replacement mechanism. Upgrades preserve all on-chain state (balances, progress, credentials) while replacing the executable code.

## Prerequisites

- **Rust** 1.70+ with `wasm32-unknown-unknown` target
- **Soroban CLI** v21+
- **Admin access** to the contracts being upgraded
- **Multi-sig partner** (for `learn-token` upgrades)

```bash
# Install prerequisites
rustup target add wasm32-unknown-unknown
cargo install --locked soroban-cli --version 21.0.0
```

## Preparation Steps

### 1. Review Changes

- Read the changelog for breaking changes
- Review all modified functions
- Check for storage layout changes
- Verify backward compatibility

### 2. Test Locally

```bash
# Run all tests
cargo test

# Run specific contract tests
cargo test -p learn-token
cargo test -p credential-nft
cargo test -p progress-tracker

# Build for deployment
cargo build --release --target wasm32-unknown-unknown
```

### 3. Check Current State

```bash
# Check current upgrade version
soroban contract invoke \
    --id <CONTRACT_ID> \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    upgrade_version

# Check current wasm hash
soroban contract invoke \
    --id <CONTRACT_ID> \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    wasm_hash
```

### 4. Backup Deployment Info

Save your current deployment configuration:

```bash
cat deployments-testnet.json
```

## Upgrade Procedure

### Option A: Single Admin Upgrade (progress-tracker, credential-nft)

```bash
# 1. Install new wasm
NEW_WASM_HASH=$(soroban contract install \
    --wasm target/wasm32-unknown-unknown/release/<contract_name>.wasm \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    --source "$STELLAR_SECRET_KEY")

echo "New WASM hash: $NEW_WASM_HASH"

# 2. Upgrade the contract
soroban contract invoke \
    --id <CONTRACT_ID> \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    upgrade \
    --new_wasm_hash "$NEW_WASM_HASH"
```

### Option B: Multi-Sig Upgrade (learn-token)

```bash
# 1. Install new wasm
NEW_WASM_HASH=$(soroban contract install \
    --wasm target/wasm32-unknown-unknown/release/learn_token.wasm \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    --source "$ADMIN_1_SECRET_KEY")

echo "New WASM hash: $NEW_WASM_HASH"

# 2. Multi-sig upgrade (requires two admin signatures)
soroban contract invoke \
    --id <LEARN_TOKEN_ID> \
    --source "$ADMIN_1_SECRET_KEY" \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    upgrade_multisig \
    --caller "$ADMIN_1_ADDRESS" \
    --co_signer "$ADMIN_2_ADDRESS" \
    --new_wasm_hash "$NEW_WASM_HASH"
```

### Option C: Using Deploy Script

```bash
# Re-deploy and upgrade all contracts
./scripts/deploy.sh testnet

# Re-initialize if needed (only if initialization logic changed)
./scripts/initialize.sh testnet
```

## Verification Steps

### 1. Verify Upgrade Version

```bash
soroban contract invoke \
    --id <CONTRACT_ID> \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    upgrade_version

# Should return: 1 (or incremented value)
```

### 2. Verify WASM Hash

```bash
soroban contract invoke \
    --id <CONTRACT_ID> \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    wasm_hash

# Should match the new hash you installed
```

### 3. Verify State Preservation

```bash
# Check token balances (learn-token)
soroban contract invoke \
    --id <LEARN_TOKEN_ID> \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    balance \
    --address <LEARNER_ADDRESS>

# Check progress (progress-tracker)
soroban contract invoke \
    --id <PROGRESS_TRACKER_ID> \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    get_progress \
    --learner <LEARNER_ADDRESS> \
    --course_id <COURSE_ID>

# Check credentials (credential-nft)
soroban contract invoke \
    --id <CREDENTIAL_NFT_ID> \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    verify_credential \
    --credential_id <CREDENTIAL_ID>
```

### 4. Verify Functionality

```bash
# Test a complete flow
# 1. Enroll in a course
soroban contract invoke \
    --id <PROGRESS_TRACKER_ID> \
    --source "$LEARNER_SECRET_KEY" \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    enroll \
    --learner <LEARNER_ADDRESS> \
    --course_id "test_course"

# 2. Claim a reward (if applicable)
soroban contract invoke \
    --id <LEARN_TOKEN_ID> \
    --source "$LEARNER_SECRET_KEY" \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    claim_reward \
    --learner <LEARNER_ADDRESS> \
    --course_id "test_course" \
    --quiz_id "test_quiz"
```

### 5. Verify Cross-Contract Wiring

```bash
# Verify progress-tracker address in learn-token
soroban contract invoke \
    --id <LEARN_TOKEN_ID> \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    progress_tracker

# Verify progress-tracker address in credential-nft
soroban contract invoke \
    --id <CREDENTIAL_NFT_ID> \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    progress_tracker
```

## Rollback Procedure

### If New Code Has Issues

If the upgrade introduces bugs or issues:

1. **Emergency Pause** (if available):
```bash
soroban contract invoke \
    --id <CONTRACT_ID> \
    --source "$ADMIN_SECRET_KEY" \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    pause
```

2. **Prepare Previous Version**:
```bash
# Checkout the previous version
git checkout <previous-tag-or-commit>

# Build the previous wasm
cargo build --release --target wasm32-unknown-unknown
```

3. **Install and Upgrade to Previous Version**:
```bash
OLD_WASM_HASH=$(soroban contract install \
    --wasm target/wasm32-unknown-unknown/release/<contract_name>.wasm \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    --source "$STELLAR_SECRET_KEY")

soroban contract invoke \
    --id <CONTRACT_ID> \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    upgrade \
    --new_wasm_hash "$OLD_WASM_HASH"
```

4. **Unpause** (if paused):
```bash
soroban contract invoke \
    --id <CONTRACT_ID> \
    --source "$ADMIN_SECRET_KEY" \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    -- \
    unpause
```

### Complete Redeployment (Nuclear Option)

If rollback is not possible:

1. Deploy fresh contracts
2. Re-initialize with same parameters
3. Migrate state (if needed) via custom migration script
4. Update all cross-contract references

## Upgrade Checklist

- [ ] Changes reviewed and approved
- [ ] All tests pass locally
- [ ] Backup of current deployment config
- [ ] Current upgrade version recorded
- [ ] New wasm built and tested
- [ ] Multi-sig partner available (for learn-token)
- [ ] Upgrade executed successfully
- [ ] Upgrade version incremented
- [ ] State preservation verified
- [ ] Cross-contract wiring verified
- [ ] End-to-end functionality tested
- [ ] Rollback plan prepared

## Related Documentation

- [Architecture](./architecture.md)
- [Security Model](./security.md)
- [Integration Guide](./integration-guide.md)