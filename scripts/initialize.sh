#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# initialize.sh — Initialize ChainLearn contracts after deployment
# ──────────────────────────────────────────────────────────────────────────────
#
# Usage:
#   ./scripts/initialize.sh testnet
#   ./scripts/initialize.sh mainnet
#
# Prerequisites:
#   - soroban CLI installed (v21+)
#   - STELLAR_SECRET_KEY environment variable set
#   - Contract must be deployed first (run deploy.sh)
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
    exit 1
fi

# Load deployment info
DEPLOY_FILE="deployments-${NETWORK}.json"
if [ ! -f "$DEPLOY_FILE" ]; then
    echo "Error: Deployment file '$DEPLOY_FILE' not found."
    echo "Run ./scripts/deploy.sh $NETWORK first."
    exit 1
fi

# Extract contract IDs
LEARN_TOKEN_ID=$(jq -r '.contracts.learn_token' "$DEPLOY_FILE")
CREDENTIAL_NFT_ID=$(jq -r '.contracts.credential_nft' "$DEPLOY_FILE")
PROGRESS_TRACKER_ID=$(jq -r '.contracts.progress_tracker' "$DEPLOY_FILE")

ADMIN_ADDRESS=$(soroban config identity address default)

echo "=== ChainLearn Contract Initialization ==="
echo "Network:           $NETWORK"
echo "Admin Address:     $ADMIN_ADDRESS"
echo ""

# Initialize learn-token
echo "[1/3] Initializing learn-token..."
soroban contract invoke \
    --id "$LEARN_TOKEN_ID" \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    -- \
    initialize \
    --admin "$ADMIN_ADDRESS" \
    --name "ChainLearn Token" \
    --symbol "CLRN" \
    --decimals 7
echo "  learn-token initialized with admin: $ADMIN_ADDRESS"

# Initialize credential-nft
echo "[2/3] Initializing credential-nft..."
soroban contract invoke \
    --id "$CREDENTIAL_NFT_ID" \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    -- \
    initialize \
    --admin "$ADMIN_ADDRESS"
echo "  credential-nft initialized with admin: $ADMIN_ADDRESS"

# Initialize progress-tracker
echo "[3/3] Initializing progress-tracker..."
soroban contract invoke \
    --id "$PROGRESS_TRACKER_ID" \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    -- \
    initialize \
    --admin "$ADMIN_ADDRESS"
echo "  progress-tracker initialized with admin: $ADMIN_ADDRESS"

echo ""
echo "=== Initialization Complete ==="
echo ""
echo "All contracts are initialized and ready to use."
echo ""
echo "Contract addresses:"
echo "  learn-token:      $LEARN_TOKEN_ID"
echo "  credential-nft:   $CREDENTIAL_NFT_ID"
echo "  progress-tracker: $PROGRESS_TRACKER_ID"
echo ""
echo "Next steps:"
echo "  1. Create a course: invoke progress-tracker create_course"
echo "  2. Learners can enroll and track progress"
echo "  3. Upon completion, mint credentials and claim token rewards"
