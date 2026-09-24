#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# monitor.sh — Monitor ChainLearn contracts health
# ──────────────────────────────────────────────────────────────────────────────
#
# Usage:
#   ./scripts/monitor.sh testnet
#   ./scripts/monitor.sh mainnet
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

echo "=== ChainLearn Contract Health Monitor ==="
echo "Network: $NETWORK"
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
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        -- \
        "$function_name" 2>/dev/null | tr -d '"'
}

HEALTHY=true

check_health() {
    local label="$1"
    local contract_id="$2"

    echo "Checking $label ($contract_id)..."

    local initialized
    initialized=$(read_contract_value "$contract_id" "is_initialized")
    if [ "$initialized" != "true" ]; then
        echo "  [ERROR] Not initialized!"
        HEALTHY=false
    else
        echo "  [OK] Initialized"
    fi

    local admin
    admin=$(read_contract_value "$contract_id" "admin")
    if [ -z "$admin" ]; then
        echo "  [ERROR] Admin not set or unreachable!"
        HEALTHY=false
    else
        echo "  [OK] Admin: $admin"
    fi
}

check_health "progress-tracker" "$PROGRESS_TRACKER_ID"
check_health "learn-token" "$LEARN_TOKEN_ID"
check_health "credential-nft" "$CREDENTIAL_NFT_ID"

echo "Checking specific metrics..."
# Check learn-token total_supply
TOTAL_SUPPLY=$(read_contract_value "$LEARN_TOKEN_ID" "total_supply")
if [ -z "$TOTAL_SUPPLY" ]; then
    echo "  [ERROR] learn-token total_supply is unreachable!"
    HEALTHY=false
else
    echo "  [OK] learn-token total_supply: $TOTAL_SUPPLY"
fi

# Check learn-token admin balance
ADMIN_ADDR=$(read_contract_value "$LEARN_TOKEN_ID" "admin")
if [ -n "$ADMIN_ADDR" ]; then
    ADMIN_BALANCE=$(soroban contract invoke \
        --id "$LEARN_TOKEN_ID" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        -- \
        balance --id "$ADMIN_ADDR" 2>/dev/null | tr -d '"')
    
    if [ -z "$ADMIN_BALANCE" ]; then
        echo "  [ERROR] admin balance is unreachable!"
        HEALTHY=false
    else
        echo "  [OK] admin balance: $ADMIN_BALANCE"
    fi
fi

echo ""
if [ "$HEALTHY" = true ]; then
    echo "=== Status: HEALTHY ==="
    exit 0
else
    echo "=== Status: UNHEALTHY ==="
    exit 1
fi
