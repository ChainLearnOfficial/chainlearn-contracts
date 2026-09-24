#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# verify.sh — Verify deployed ChainLearn contracts match expected WASM hashes
# ──────────────────────────────────────────────────────────────────────────────
#
# Usage:
#   ./scripts/verify.sh testnet
#   ./scripts/verify.sh mainnet
#
# Prerequisites:
#   - soroban CLI installed (v21+)
#   - jq installed
#   - Contract must be deployed first (run deploy.sh)
# ──────────────────────────────────────────────────────────────────────────────

if ! command -v jq &>/dev/null; then
    echo "Error: jq is required but not installed."
    exit 1
fi

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

# Load deployment info
DEPLOY_FILE="deployments-${NETWORK}.json"
if [ ! -f "$DEPLOY_FILE" ]; then
    echo "Error: Deployment file '$DEPLOY_FILE' not found."
    echo "Run ./scripts/deploy.sh $NETWORK first."
    exit 1
fi

LEARN_TOKEN_ID=$(jq -r '.contracts.learn_token' "$DEPLOY_FILE")
CREDENTIAL_NFT_ID=$(jq -r '.contracts.credential_nft' "$DEPLOY_FILE")
PROGRESS_TRACKER_ID=$(jq -r '.contracts.progress_tracker' "$DEPLOY_FILE")

if [[ -z "$LEARN_TOKEN_ID" || -z "$CREDENTIAL_NFT_ID" || -z "$PROGRESS_TRACKER_ID" ]]; then
    echo "Error: One or more contract IDs are missing from $DEPLOY_FILE."
    exit 1
fi

echo "=== ChainLearn Contract Verification ==="
echo "Network: $NETWORK"
echo ""

echo "Verifying RPC reachability..."
if ! curl -s --max-time 10 "$RPC_URL" > /dev/null; then
    echo "Error: RPC endpoint $RPC_URL is not reachable."
    exit 1
fi
echo "RPC reachable."
echo ""

# Build all contracts to get local WASMs
echo "Building contracts locally..."
cargo build --release --target wasm32-unknown-unknown

# deploy.sh/upgrade.sh upload the optimized WASM (#344), so compare against
# that. Use the same STRIP_SPEC_DOCS setting that was used when deploying.
"$(dirname "$0")/optimize-wasm.sh"

verify_contract() {
    local label="$1"
    local contract_id="$2"
    local local_wasm="$3"
    local fetched_wasm="fetched_${label}.wasm"

    echo "Verifying $label ($contract_id)..."

    # Fetch deployed WASM
    soroban contract fetch \
        --id "$contract_id" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        --out-file "$fetched_wasm" > /dev/null 2>&1

    if [ ! -f "$fetched_wasm" ]; then
        echo "Error: Failed to fetch WASM for $label."
        exit 1
    fi

    # Compute hashes
    local local_hash
    local_hash=$(sha256sum "$local_wasm" | awk '{print $1}')
    local fetched_hash
    fetched_hash=$(sha256sum "$fetched_wasm" | awk '{print $1}')

    rm -f "$fetched_wasm"

    if [ "$local_hash" = "$fetched_hash" ]; then
        echo "  Match: $local_hash"
    else
        echo "  Mismatch!"
        echo "  Expected (local): $local_hash"
        echo "  Deployed:         $fetched_hash"
        exit 1
    fi
}

verify_contract "learn-token" "$LEARN_TOKEN_ID" "target/wasm32-unknown-unknown/release/learn_token.optimized.wasm"
verify_contract "credential-nft" "$CREDENTIAL_NFT_ID" "target/wasm32-unknown-unknown/release/credential_nft.optimized.wasm"
verify_contract "progress-tracker" "$PROGRESS_TRACKER_ID" "target/wasm32-unknown-unknown/release/progress_tracker.optimized.wasm"

echo ""
echo "=== Verification Successful ==="
echo "All deployed contracts match the local builds."
