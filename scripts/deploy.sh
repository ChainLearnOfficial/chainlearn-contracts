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
#   - jq installed
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

# Verify jq is installed (#59)
if ! command -v jq &>/dev/null; then
    echo "Error: jq is required but not installed."
    echo "Install it with: sudo apt install jq  (Debian/Ubuntu)"
    echo "                 brew install jq      (macOS)"
    exit 1
fi

echo "=== ChainLearn Contract Deployment ==="
echo "Network:  $NETWORK"
echo "RPC URL:  $RPC_URL"
echo ""

echo "Verifying RPC reachability..."
if ! curl -s --max-time 10 "$RPC_URL" > /dev/null; then
    echo "Error: RPC endpoint $RPC_URL is not reachable."
    exit 1
fi
echo "RPC reachable."
echo ""

DEPLOY_FILE="deployments-${NETWORK}.json"
if [ -f "$DEPLOY_FILE" ]; then
    echo "Error: Deployment already exists in $DEPLOY_FILE"
    echo "To redeploy, please remove this file first."
    exit 1
fi

# Build all contracts
echo "[1/4] Building contracts..."
cargo build --release --target wasm32-unknown-unknown

# On mainnet, perform safety checks. On testnet, skip for faster iteration (#61).
DEPLOY_FLAGS=()
if [ "$NETWORK" = "mainnet" ]; then
    echo "  Mainnet deployment — running safety checks."
else
    DEPLOY_FLAGS+=(--ignore-checks)
fi

# Deploy learn-token
echo "[2/4] Deploying learn-token..."
LEARN_TOKEN_ID=$(soroban contract deploy \
    --wasm target/wasm32-unknown-unknown/release/learn_token.wasm \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    "${DEPLOY_FLAGS[@]+"${DEPLOY_FLAGS[@]}"}")
echo "  learn-token deployed: $LEARN_TOKEN_ID"

# Deploy credential-nft
echo "[3/4] Deploying credential-nft..."
CREDENTIAL_NFT_ID=$(soroban contract deploy \
    --wasm target/wasm32-unknown-unknown/release/credential_nft.wasm \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    "${DEPLOY_FLAGS[@]+"${DEPLOY_FLAGS[@]}"}")
echo "  credential-nft deployed: $CREDENTIAL_NFT_ID"

# Deploy progress-tracker
echo "[4/4] Deploying progress-tracker..."
PROGRESS_TRACKER_ID=$(soroban contract deploy \
    --wasm target/wasm32-unknown-unknown/release/progress_tracker.wasm \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    "${DEPLOY_FLAGS[@]+"${DEPLOY_FLAGS[@]}"}")
echo "  progress-tracker deployed: $PROGRESS_TRACKER_ID"

# Write deployment info to file and validate (#60)
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

# Validate the written JSON is parseable
if ! jq empty "$DEPLOY_FILE" 2>/dev/null; then
    echo "Error: Failed to write valid JSON to $DEPLOY_FILE"
    echo "Contents:"
    cat "$DEPLOY_FILE"
    exit 1
fi

# Sanity-check that all contract IDs look like valid Stellar addresses
for field in learn_token credential_nft progress_tracker; do
    value=$(jq -r ".contracts.$field" "$DEPLOY_FILE")
    if [[ ! "$value" =~ ^C[A-Z0-9]{55,62}$ ]]; then
        echo "Warning: Contract ID for $field does not look like a valid Stellar contract address: $value"
    fi
done

echo ""
echo "=== Deployment Complete ==="
echo "Contract addresses saved to: $DEPLOY_FILE"
echo ""
echo "Next step: Run ./scripts/initialize.sh $NETWORK to initialize contracts."
