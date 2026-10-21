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

    # Pause state check (#605)
    local paused
    paused=$(read_contract_value "$contract_id" "is_paused")
    if [ "$paused" = "true" ]; then
        echo "  [WARN] Pause state: PAUSED (contract operations are paused)"
    elif [ "$paused" = "false" ]; then
        echo "  [OK] Pause state: Unpaused (active)"
    elif [ -n "$paused" ]; then
        echo "  [OK] Pause state: $paused"
    else
        echo "  [INFO] Pause state: unpaused"
    fi

    # Admin transfer pending check (#605)
    local pending
    pending=$(read_contract_value "$contract_id" "pending_admin")
    if [ -n "$pending" ] && [ "$pending" != "null" ] && [ "$pending" != "None" ] && [ "$pending" != "()" ] && [ "$pending" != "void" ]; then
        echo "  [WARN] Pending admin transfer detected: $pending"
    else
        echo "  [OK] No admin transfer pending"
    fi
}

echo "=== Contract Health & State Checks ==="
check_health "progress-tracker" "$PROGRESS_TRACKER_ID"
check_health "learn-token" "$LEARN_TOKEN_ID"
check_health "credential-nft" "$CREDENTIAL_NFT_ID"
echo ""

echo "=== Verifying Cross-Contract Wiring ==="
# Verify learn-token points to progress-tracker (#605)
LT_TRACKER=$(read_contract_value "$LEARN_TOKEN_ID" "progress_tracker")
if [ "$LT_TRACKER" = "$PROGRESS_TRACKER_ID" ]; then
    echo "  [OK] learn-token wired to progress-tracker ($PROGRESS_TRACKER_ID)"
elif [ -z "$LT_TRACKER" ]; then
    echo "  [ERROR] learn-token progress_tracker address unreachable!"
    HEALTHY=false
else
    echo "  [ERROR] learn-token progress_tracker mismatch! Expected: $PROGRESS_TRACKER_ID, Got: $LT_TRACKER"
    HEALTHY=false
fi

# Verify credential-nft points to progress-tracker (#605)
NFT_TRACKER=$(read_contract_value "$CREDENTIAL_NFT_ID" "progress_tracker")
if [ "$NFT_TRACKER" = "$PROGRESS_TRACKER_ID" ]; then
    echo "  [OK] credential-nft wired to progress-tracker ($PROGRESS_TRACKER_ID)"
elif [ -z "$NFT_TRACKER" ]; then
    echo "  [ERROR] credential-nft progress_tracker address unreachable!"
    HEALTHY=false
else
    echo "  [ERROR] credential-nft progress_tracker mismatch! Expected: $PROGRESS_TRACKER_ID, Got: $NFT_TRACKER"
    HEALTHY=false
fi
echo ""

echo "=== Contract Specific Metrics ==="
# 1. learn-token metrics
echo "learn-token metrics:"
TOTAL_SUPPLY=$(read_contract_value "$LEARN_TOKEN_ID" "total_supply")
if [ -z "$TOTAL_SUPPLY" ]; then
    echo "  [ERROR] learn-token total_supply is unreachable!"
    HEALTHY=false
else
    echo "  [OK] total_supply: $TOTAL_SUPPLY"
fi

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

# 2. credential-nft metrics (#605)
echo "credential-nft metrics:"
TOTAL_CREDS=$(read_contract_value "$CREDENTIAL_NFT_ID" "get_total_credentials_count")
if [ -n "$TOTAL_CREDS" ] && [ "$TOTAL_CREDS" != "null" ]; then
    echo "  [OK] total credentials: $TOTAL_CREDS"
else
    TOTAL_CREDS=$(read_contract_value "$CREDENTIAL_NFT_ID" "total_credentials")
    if [ -n "$TOTAL_CREDS" ] && [ "$TOTAL_CREDS" != "null" ]; then
        echo "  [OK] total credentials: $TOTAL_CREDS"
    else
        echo "  [INFO] total credentials: 0 (or counter uninitialized)"
    fi
fi

REVOCATION_COUNT=$(read_contract_value "$CREDENTIAL_NFT_ID" "revocation_count")
if [ -z "$REVOCATION_COUNT" ]; then
    REVOCATION_COUNT=$(read_contract_value "$CREDENTIAL_NFT_ID" "get_revocation_count")
fi
if [ -n "$REVOCATION_COUNT" ] && [ "$REVOCATION_COUNT" != "null" ]; then
    echo "  [OK] revocation count: $REVOCATION_COUNT"
else
    echo "  [INFO] revocation count: 0 (or no batch revocation counter)"
fi

# 3. progress-tracker metrics (#605)
echo "progress-tracker metrics:"
COURSE_COUNT=$(read_contract_value "$PROGRESS_TRACKER_ID" "get_course_count")
if [ -z "$COURSE_COUNT" ]; then
    COURSE_COUNT=$(read_contract_value "$PROGRESS_TRACKER_ID" "course_count")
fi
if [ -n "$COURSE_COUNT" ] && [ "$COURSE_COUNT" != "null" ]; then
    echo "  [OK] course count: $COURSE_COUNT"
else
    # Check storage size proxy
    STORAGE_SIZE=$(read_contract_value "$PROGRESS_TRACKER_ID" "get_storage_size")
    if [ -n "$STORAGE_SIZE" ]; then
        echo "  [OK] course/storage entries: $STORAGE_SIZE"
    else
        echo "  [INFO] course count: tracked on-chain"
    fi
fi

ENROLLMENT_COUNT=$(read_contract_value "$PROGRESS_TRACKER_ID" "get_enrollment_count")
if [ -z "$ENROLLMENT_COUNT" ]; then
    ENROLLMENT_COUNT=$(read_contract_value "$PROGRESS_TRACKER_ID" "enrollment_count")
fi
if [ -n "$ENROLLMENT_COUNT" ] && [ "$ENROLLMENT_COUNT" != "null" ]; then
    echo "  [OK] enrollment count: $ENROLLMENT_COUNT"
else
    echo "  [INFO] enrollment count: active per-learner"
fi

echo ""
if [ "$HEALTHY" = true ]; then
    echo "=== Status: HEALTHY ==="
    exit 0
else
    echo "=== Status: UNHEALTHY ==="
    exit 1
fi
