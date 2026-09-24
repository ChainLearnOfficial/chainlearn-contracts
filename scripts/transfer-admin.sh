#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# transfer-admin.sh — Transfer ChainLearn contracts admin rights to a new address
# ──────────────────────────────────────────────────────────────────────────────
#
# Usage:
#   ./scripts/transfer-admin.sh testnet <new_admin_address>
#   ./scripts/transfer-admin.sh mainnet <new_admin_address>
#
# Prerequisites:
#   - soroban CLI installed (v21+)
#   - jq installed
#   - STELLAR_SECRET_KEY environment variable set (current admin)
#   - Contract must be deployed first (run deploy.sh)
# ──────────────────────────────────────────────────────────────────────────────

if ! command -v jq &>/dev/null; then
    echo "Error: jq is required but not installed."
    exit 1
fi

NETWORK="${1:-testnet}"
NEW_ADMIN="${2:-}"

if [ -z "$NEW_ADMIN" ]; then
    echo "Error: new admin address is required."
    echo "Usage: $0 $NETWORK <new_admin_address>"
    exit 1
fi

if [[ ! "$NEW_ADMIN" =~ ^G[A-Z0-9]{55}$ ]]; then
    echo "Error: Invalid Stellar public key for new admin: $NEW_ADMIN"
    exit 1
fi

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

# Extract contract IDs and validate they are non-empty
LEARN_TOKEN_ID=$(jq -r '.contracts.learn_token' "$DEPLOY_FILE")
CREDENTIAL_NFT_ID=$(jq -r '.contracts.credential_nft' "$DEPLOY_FILE")
PROGRESS_TRACKER_ID=$(jq -r '.contracts.progress_tracker' "$DEPLOY_FILE")

if [[ -z "$LEARN_TOKEN_ID" || -z "$CREDENTIAL_NFT_ID" || -z "$PROGRESS_TRACKER_ID" ]]; then
    echo "Error: One or more contract IDs are missing from $DEPLOY_FILE."
    exit 1
fi

for field in "$LEARN_TOKEN_ID" "$CREDENTIAL_NFT_ID" "$PROGRESS_TRACKER_ID"; do
    if [[ ! "$field" =~ ^C[A-Z0-9]{55,62}$ ]]; then
        echo "Error: Invalid Stellar contract ID in $DEPLOY_FILE: $field"
        exit 1
    fi
done

echo "=== ChainLearn Contract Admin Transfer ==="
echo "Network:           $NETWORK"
echo "New Admin Address: $NEW_ADMIN"
echo ""

echo "Verifying RPC reachability..."
if ! curl -s --max-time 10 "$RPC_URL" > /dev/null; then
    echo "Error: RPC endpoint $RPC_URL is not reachable."
    exit 1
fi
echo "RPC reachable."
echo ""

read_contract_value() {
    local contract_id="$1"
    local function_name="$2"
    soroban contract invoke \
        --id "$contract_id" \
        --source "$STELLAR_SECRET_KEY" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        -- \
        "$function_name" 2>/dev/null | tr -d '"'
}

transfer_and_verify() {
    local label="$1"
    local contract_id="$2"

    echo "Transferring admin for $label..."
    soroban contract invoke \
        --id "$contract_id" \
        --source "$STELLAR_SECRET_KEY" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        -- \
        transfer_admin \
        --new_admin "$NEW_ADMIN"

    local stored_admin

    if [ "$label" = "learn-token" ]; then
        # learn-token uses a two-step admin transfer, so we check pending_admin
        stored_admin=$(read_contract_value "$contract_id" "pending_admin")
        if [[ "$stored_admin" != *"$NEW_ADMIN"* ]]; then
            echo "Error: Failed to initiate admin transfer for $label."
            echo "  expected pending: $NEW_ADMIN"
            echo "  stored pending:   ${stored_admin:-<unset>}"
            exit 1
        fi
        echo "  verified: $label admin transfer initiated to $NEW_ADMIN"
    else
        stored_admin=$(read_contract_value "$contract_id" "admin")

        if [ "$stored_admin" != "$NEW_ADMIN" ]; then
            echo "Error: Failed to transfer admin for $label."
            echo "  expected: $NEW_ADMIN"
            echo "  stored:   ${stored_admin:-<unset>}"
            exit 1
        fi
        echo "  verified: $label admin is now $NEW_ADMIN"
    fi
}

transfer_and_verify "progress-tracker" "$PROGRESS_TRACKER_ID"
transfer_and_verify "learn-token" "$LEARN_TOKEN_ID"
transfer_and_verify "credential-nft" "$CREDENTIAL_NFT_ID"

echo ""
echo "=== Admin Transfer Complete ==="
echo ""
echo "All contracts have been updated to the new admin: $NEW_ADMIN"
