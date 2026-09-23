#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# upgrade.sh — Upgrade ChainLearn contracts while preserving state
# ──────────────────────────────────────────────────────────────────────────────
#
# Usage:
#   ./scripts/upgrade.sh testnet                    # Upgrade all contracts
#   ./scripts/upgrade.sh testnet learn-token        # Upgrade only learn-token
#   ./scripts/upgrade.sh testnet credential-nft     # Upgrade only credential-nft
#   ./scripts/upgrade.sh testnet progress-tracker   # Upgrade only progress-tracker
#
# Prerequisites:
#   - soroban CLI installed (v21+)
#   - jq installed
#   - STELLAR_SECRET_KEY environment variable set
#   - Contracts must be deployed and initialized (run deploy.sh + initialize.sh)
#   - New WASM targets must be built (cargo build --release --target wasm32-unknown-unknown)
# ──────────────────────────────────────────────────────────────────────────────

NETWORK="${1:-testnet}"
CONTRACT_FILTER="${2:-all}"

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

# Verify jq is installed
if ! command -v jq &>/dev/null; then
    echo "Error: jq is required but not installed."
    echo "Install it with: sudo apt install jq  (Debian/Ubuntu)"
    echo "                 brew install jq      (macOS)"
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

# Validate contract IDs are non-empty
if [[ -z "$LEARN_TOKEN_ID" || -z "$CREDENTIAL_NFT_ID" || -z "$PROGRESS_TRACKER_ID" ]]; then
    echo "Error: One or more contract IDs are missing from $DEPLOY_FILE."
    exit 1
fi

for field in "$LEARN_TOKEN_ID" "$CREDENTIAL_NFT_ID" "$PROGRESS_TRACKER_ID"; do
    if [[ ! "$field" =~ ^C[A-Z0-9]{55,62}$ ]]; then
        echo "Error: Invalid Stellar contract ID: $field"
        exit 1
    fi
done

echo "=== ChainLearn Contract Upgrade ==="
echo "Network:  $NETWORK"
echo "RPC URL:  $RPC_URL"
echo "Filter:   $CONTRACT_FILTER"
echo ""

echo "Verifying RPC reachability..."
if ! curl -s --max-time 10 "$RPC_URL" > /dev/null; then
    echo "Error: RPC endpoint $RPC_URL is not reachable."
    exit 1
fi
echo "RPC reachable."
echo ""

# ── Step 1: Build new WASM ────────────────────────────────────────────

echo "[1/5] Building contracts..."
cargo build --release --target wasm32-unknown-unknown
echo "  Build complete."
echo ""

# ── Step 2: Install new WASM and get hashes ───────────────────────────

DEPLOY_FLAGS=()
if [ "$NETWORK" = "mainnet" ]; then
    echo "  Mainnet upgrade — running safety checks."
else
    DEPLOY_FLAGS+=(--ignore-checks)
fi

install_wasm() {
    local wasm_file="$1"
    local label="$2"
    echo "  Installing $label WASM..."
    local hash
    hash=$(soroban contract install \
        --wasm "$wasm_file" \
        --source "$STELLAR_SECRET_KEY" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        "${DEPLOY_FLAGS[@]+"${DEPLOY_FLAGS[@]}"}")
    echo "    WASM hash: $hash"
    echo "$hash"
}

upgrade_contract() {
    local contract_id="$1"
    local wasm_hash="$2"
    local label="$3"

    echo "  Upgrading $label ($contract_id)..."

    # Check current upgrade version before upgrading
    local current_version
    current_version=$(soroban contract invoke \
        --id "$contract_id" \
        --source "$STELLAR_SECRET_KEY" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        -- \
        upgrade_version 2>/dev/null || echo "0")
    echo "    Current version: $current_version"

    soroban contract invoke \
        --id "$contract_id" \
        --source "$STELLAR_SECRET_KEY" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        -- \
        upgrade \
        --new_wasm_hash "$wasm_hash"

    # Verify the upgrade landed
    local new_version
    new_version=$(soroban contract invoke \
        --id "$contract_id" \
        --source "$STELLAR_SECRET_KEY" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        -- \
        upgrade_version 2>/dev/null || echo "?")
    echo "    New version: $new_version"

    local stored_hash
    stored_hash=$(soroban contract invoke \
        --id "$contract_id" \
        --source "$STELLAR_SECRET_KEY" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        -- \
        wasm_hash 2>/dev/null | tr -d '"')

    if [ "$stored_hash" = "$wasm_hash" ]; then
        echo "    Verified: WASM hash matches."
    else
        echo "    Warning: WASM hash verification returned: $stored_hash"
        echo "    Expected: $wasm_hash"
    fi
}

# ── Step 3: Upgrade contracts ─────────────────────────────────────────

echo "[2/5] Installing and upgrading contracts..."

LEARN_TOKEN_HASH=""
CREDENTIAL_NFT_HASH=""
PROGRESS_TRACKER_HASH=""

# Install and upgrade learn-token
if [ "$CONTRACT_FILTER" = "all" ] || [ "$CONTRACT_FILTER" = "learn-token" ]; then
    LEARN_TOKEN_HASH=$(install_wasm "target/wasm32-unknown-unknown/release/learn_token.wasm" "learn-token")
    upgrade_contract "$LEARN_TOKEN_ID" "$LEARN_TOKEN_HASH" "learn-token"
    echo ""
else
    echo "  Skipping learn-token (filtered out)."
fi

# Install and upgrade credential-nft
if [ "$CONTRACT_FILTER" = "all" ] || [ "$CONTRACT_FILTER" = "credential-nft" ]; then
    CREDENTIAL_NFT_HASH=$(install_wasm "target/wasm32-unknown-unknown/release/credential_nft.wasm" "credential-nft")
    upgrade_contract "$CREDENTIAL_NFT_ID" "$CREDENTIAL_NFT_HASH" "credential-nft"
    echo ""
else
    echo "  Skipping credential-nft (filtered out)."
fi

# Install and upgrade progress-tracker
if [ "$CONTRACT_FILTER" = "all" ] || [ "$CONTRACT_FILTER" = "progress-tracker" ]; then
    PROGRESS_TRACKER_HASH=$(install_wasm "target/wasm32-unknown-unknown/release/progress_tracker.wasm" "progress-tracker")
    upgrade_contract "$PROGRESS_TRACKER_ID" "$PROGRESS_TRACKER_HASH" "progress-tracker"
    echo ""
else
    echo "  Skipping progress-tracker (filtered out)."
fi

# ── Step 4: Verify state preservation ─────────────────────────────────

echo "[3/5] Verifying state preservation..."

# Verify progress-tracker is still wired correctly in learn-token
if [ "$CONTRACT_FILTER" = "all" ] || [ "$CONTRACT_FILTER" = "learn-token" ]; then
    TOKEN_TRACKER=$(soroban contract invoke \
        --id "$LEARN_TOKEN_ID" \
        --source "$STELLAR_SECRET_KEY" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        -- \
        progress_tracker 2>/dev/null | tr -d '"')

    if [ "$TOKEN_TRACKER" = "$PROGRESS_TRACKER_ID" ]; then
        echo "  learn-token -> progress-tracker wiring: OK"
    else
        echo "  Error: learn-token progress-tracker mismatch!"
        echo "    Expected: $PROGRESS_TRACKER_ID"
        echo "    Got:      $TOKEN_TRACKER"
        exit 1
    fi
fi

# Verify progress-tracker is still wired correctly in credential-nft
if [ "$CONTRACT_FILTER" = "all" ] || [ "$CONTRACT_FILTER" = "credential-nft" ]; then
    CRED_TRACKER=$(soroban contract invoke \
        --id "$CREDENTIAL_NFT_ID" \
        --source "$STELLAR_SECRET_KEY" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        -- \
        progress_tracker 2>/dev/null | tr -d '"')

    if [ "$CRED_TRACKER" = "$PROGRESS_TRACKER_ID" ]; then
        echo "  credential-nft -> progress-tracker wiring: OK"
    else
        echo "  Error: credential-nft progress-tracker mismatch!"
        echo "    Expected: $PROGRESS_TRACKER_ID"
        echo "    Got:      $CRED_TRACKER"
        exit 1
    fi
fi

# Verify admin is still set correctly
if [ "$CONTRACT_FILTER" = "all" ] || [ "$CONTRACT_FILTER" = "progress-tracker" ]; then
    TRACKER_ADMIN=$(soroban contract invoke \
        --id "$PROGRESS_TRACKER_ID" \
        --source "$STELLAR_SECRET_KEY" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        -- \
        admin 2>/dev/null | tr -d '"')

    if [ -n "$TRACKER_ADMIN" ]; then
        echo "  progress-tracker admin: $TRACKER_ADMIN (preserved)"
    else
        echo "  Warning: Could not read progress-tracker admin."
    fi
fi

# ── Step 5: Record upgrade in deployment file ─────────────────────────

echo ""
echo "[4/5] Updating deployment file..."

UPGRADE_TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Read existing deployment file and merge upgrade info
DEPLOY_CONTENT=$(cat "$DEPLOY_FILE")

# Add upgrade info if not present, or update existing
if echo "$DEPLOY_CONTENT" | jq -e '.upgrades' > /dev/null 2>&1; then
    # Append to existing upgrades array
    UPDATED=$(echo "$DEPLOY_CONTENT" | jq --arg ts "$UPGRADE_TIMESTAMP" \
        --arg lt_hash "$LEARN_TOKEN_HASH" \
        --arg cn_hash "$CREDENTIAL_NFT_HASH" \
        --arg pt_hash "$PROGRESS_TRACKER_HASH" \
        --arg filter "$CONTRACT_FILTER" \
        '.upgrades += [{"timestamp": $ts, "filter": $filter, "learn_token_hash": $lt_hash, "credential_nft_hash": $cn_hash, "progress_tracker_hash": $pt_hash}]')
else
    # Create upgrades array
    UPDATED=$(echo "$DEPLOY_CONTENT" | jq --arg ts "$UPGRADE_TIMESTAMP" \
        --arg lt_hash "$LEARN_TOKEN_HASH" \
        --arg cn_hash "$CREDENTIAL_NFT_HASH" \
        --arg pt_hash "$PROGRESS_TRACKER_HASH" \
        --arg filter "$CONTRACT_FILTER" \
        '. + {"upgrades": [{"timestamp": $ts, "filter": $filter, "learn_token_hash": $lt_hash, "credential_nft_hash": $cn_hash, "progress_tracker_hash": $pt_hash}]}')
fi

echo "$UPDATED" | jq empty 2>/dev/null
if [ $? -eq 0 ]; then
    echo "$UPDATED" > "$DEPLOY_FILE"
    echo "  Deployment file updated: $DEPLOY_FILE"
else
    echo "  Warning: Failed to update deployment file. Current file preserved."
fi

# ── Step 6: Summary ──────────────────────────────────────────────────

echo ""
echo "[5/5] Upgrade complete!"
echo ""
echo "=== Upgrade Summary ==="
echo "Network:    $NETWORK"
echo "Timestamp:  $UPGRADE_TIMESTAMP"
echo ""

if [ -n "$LEARN_TOKEN_HASH" ]; then
    echo "learn-token:"
    echo "  Contract: $LEARN_TOKEN_ID"
    echo "  New WASM: $LEARN_TOKEN_HASH"
fi

if [ -n "$CREDENTIAL_NFT_HASH" ]; then
    echo "credential-nft:"
    echo "  Contract: $CREDENTIAL_NFT_ID"
    echo "  New WASM: $CREDENTIAL_NFT_HASH"
fi

if [ -n "$PROGRESS_TRACKER_HASH" ]; then
    echo "progress-tracker:"
    echo "  Contract: $PROGRESS_TRACKER_ID"
    echo "  New WASM: $PROGRESS_TRACKER_HASH"
fi

echo ""
echo "State is preserved across upgrades."
echo "Verify by running a few contract reads (get_progress, balance, etc.)."
