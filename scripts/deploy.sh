#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# deploy.sh — Deploy ChainLearn contracts to Stellar testnet or mainnet
# ──────────────────────────────────────────────────────────────────────────────
#
# Usage:
#   ./scripts/deploy.sh testnet
#   ./scripts/deploy.sh mainnet
#
# Prerequisites:
#   - soroban CLI installed (v21+)
#   - STELLAR_SECRET_KEY environment variable set
#   - Sufficient XLM for deployment fees
# ──────────────────────────────────────────────────────────────────────────────

NETWORK="${1:-testnet}"

if [ "$NETWORK" = "testnet" ]; then
    RPC_URL="https://soroban-testnet.stellar.org:443"
    NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
elif [ "$NETWORK" = "mainnet" ]; then
    RPC_URL="https://soroban-rpc.mainnet.stellar.gateway.fm:443"
    NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015"
else
    echo "Error: Unknown network '$NETWORK'. Use 'testnet' or 'mainnet'."
    exit 1
fi

# Verify secret key is available
if [ -z "${STELLAR_SECRET_KEY:-}" ]; then
    echo "Error: STELLAR_SECRET_KEY environment variable is not set."
    echo "Export your Stellar secret key before running this script."
    exit 1
fi

echo "=== ChainLearn Contract Deployment ==="
echo "Network:  $NETWORK"
echo "RPC URL:  $RPC_URL"
echo ""

# Build all contracts
echo "[1/4] Building contracts..."
cargo build --release --target wasm32-unknown-unknown

# Deploy learn-token
echo "[2/4] Deploying learn-token..."
LEARN_TOKEN_ID=$(soroban contract deploy \
    --wasm target/wasm32-unknown-unknown/release/learn_token.wasm \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --ignore-checks)
echo "  learn-token deployed: $LEARN_TOKEN_ID"

# Deploy credential-nft
echo "[3/4] Deploying credential-nft..."
CREDENTIAL_NFT_ID=$(soroban contract deploy \
    --wasm target/wasm32-unknown-unknown/release/credential_nft.wasm \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --ignore-checks)
echo "  credential-nft deployed: $CREDENTIAL_NFT_ID"

# Deploy progress-tracker
echo "[4/4] Deploying progress-tracker..."
PROGRESS_TRACKER_ID=$(soroban contract deploy \
    --wasm target/wasm32-unknown-unknown/release/progress_tracker.wasm \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --ignore-checks)
echo "  progress-tracker deployed: $PROGRESS_TRACKER_ID"

# Write deployment info to file
DEPLOY_FILE="deployments-${NETWORK}.json"
cat > "$DEPLOY_FILE" << EOF
{
  "network": "$NETWORK",
  "deployed_at": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "contracts": {
    "learn_token": "$LEARN_TOKEN_ID",
    "credential_nft": "$CREDENTIAL_NFT_ID",
    "progress_tracker": "$PROGRESS_TRACKER_ID"
  }
}
EOF

echo ""
echo "=== Deployment Complete ==="
echo "Contract addresses saved to: $DEPLOY_FILE"
echo ""
echo "Next step: Run ./scripts/initialize.sh $NETWORK to initialize contracts."
