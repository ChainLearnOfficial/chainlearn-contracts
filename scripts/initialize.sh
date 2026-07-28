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
#   - jq installed
#   - STELLAR_SECRET_KEY environment variable set
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
    echo "Ensure deploy.sh completed successfully and the file is not corrupted."
    exit 1
fi

for field in "$LEARN_TOKEN_ID" "$CREDENTIAL_NFT_ID" "$PROGRESS_TRACKER_ID"; do
    if [[ ! "$field" =~ ^C[A-Z0-9]{55,62}$ ]]; then
        echo "Error: Invalid Stellar contract ID in $DEPLOY_FILE: $field"
        exit 1
    fi
done

ADMIN_ADDRESS=$(soroban config identity address default 2>&1) || {
    echo "Error: Failed to resolve default identity."
    echo "Ensure 'soroban keys list' shows a default identity, or set STELLAR_SECRET_KEY."
    echo "Output: $ADMIN_ADDRESS"
    exit 1
}

echo "=== ChainLearn Contract Initialization ==="
echo "Network:           $NETWORK"
echo "Admin Address:     $ADMIN_ADDRESS"
echo ""

echo "Verifying RPC reachability..."
if ! curl -s --max-time 10 "$RPC_URL" > /dev/null; then
    echo "Error: RPC endpoint $RPC_URL is not reachable."
    exit 1
fi
echo "RPC reachable."
echo ""

# Invoke a read-only contract function and echo its result, stripping the
# quotes the CLI wraps around address returns.
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

# Confirm a contract stored the progress-tracker address it was initialized
# with. Catches a dropped or misspelled --progress_tracker argument here, rather
# than at the first claim_reward / mint_credential call in production (#31).
assert_progress_tracker_wired() {
    local label="$1"
    local contract_id="$2"
    local stored
    stored=$(read_contract_value "$contract_id" "progress_tracker")

    if [ "$stored" != "$PROGRESS_TRACKER_ID" ]; then
        echo "Error: $label is not wired to the progress-tracker."
        echo "  expected: $PROGRESS_TRACKER_ID"
        echo "  stored:   ${stored:-<unset>}"
        exit 1
    fi
    echo "  verified: $label -> progress-tracker $PROGRESS_TRACKER_ID"
}

# Initialization order matters: learn-token and credential-nft both take the
# progress-tracker's address and call into it at runtime, so the tracker is
# initialized first and the other two are wired to a live contract (#32).

# 1. Initialize progress-tracker (no dependencies)
echo "[1/3] Initializing progress-tracker..."
soroban contract invoke \
    --id "$PROGRESS_TRACKER_ID" \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    -- \
    initialize \
    --admin "$ADMIN_ADDRESS"
echo "  progress-tracker initialized with admin: $ADMIN_ADDRESS"

# The two dependent contracts are only wired up once the tracker answers, so a
# failed tracker initialization stops the run instead of cascading.
TRACKER_ADMIN=$(read_contract_value "$PROGRESS_TRACKER_ID" "admin")
if [ "$TRACKER_ADMIN" != "$ADMIN_ADDRESS" ]; then
    echo "Error: progress-tracker did not initialize."
    echo "  expected admin: $ADMIN_ADDRESS"
    echo "  reported admin: ${TRACKER_ADMIN:-<unset>}"
    exit 1
fi
echo "  verified: progress-tracker admin is $ADMIN_ADDRESS"

# 2. Initialize learn-token (depends on progress-tracker)
echo "[2/3] Initializing learn-token..."
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
    --decimal 7 \
    --progress_tracker "$PROGRESS_TRACKER_ID"
echo "  learn-token initialized with admin: $ADMIN_ADDRESS"
assert_progress_tracker_wired "learn-token" "$LEARN_TOKEN_ID"

# 3. Initialize credential-nft (depends on progress-tracker)
echo "[3/3] Initializing credential-nft..."
soroban contract invoke \
    --id "$CREDENTIAL_NFT_ID" \
    --source "$STELLAR_SECRET_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    -- \
    initialize \
    --admin "$ADMIN_ADDRESS" \
    --progress_tracker "$PROGRESS_TRACKER_ID"
echo "  credential-nft initialized with admin: $ADMIN_ADDRESS"
assert_progress_tracker_wired "credential-nft" "$CREDENTIAL_NFT_ID"

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
