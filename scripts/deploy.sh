#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# deploy.sh — Deploy ChainLearn contracts to Stellar testnet or mainnet
# ──────────────────────────────────────────────────────────────────────────────
#
# Usage:
#   ./scripts/deploy.sh testnet                      # Deploy to testnet with cost confirmation
#   ./scripts/deploy.sh mainnet                      # Deploy to mainnet with cost confirmation
#   ./scripts/deploy.sh testnet --confirm            # Skip cost confirmation prompt (#602)
#   ./scripts/deploy.sh testnet --resume             # Resume from partial failure (#603)
#   ./scripts/deploy.sh testnet --estimate-only      # Show cost estimation and exit (#602)
#   ./scripts/deploy.sh testnet --dry-run            # Validate and build, but do not deploy
#
# Flags:
#   --confirm        Skip confirmation prompt after showing deployment cost estimation
#   --resume         Resume deployment from a previous partial deployment failure
#   --estimate-only  Display pre-deployment cost estimation and exit
#   --dry-run        Validate inputs, build and optimize WASM, print what would be
#                    deployed, then exit without running 'soroban contract deploy'
#   -h, --help       Display this help message
#
# Prerequisites:
#   - soroban CLI installed (v21+)
#   - jq installed
#   - python3 installed (only when STRIP_SPEC_DOCS=1, see optimize-wasm.sh)
#   - STELLAR_SECRET_KEY environment variable set
#   - Sufficient XLM for deployment fees
# ──────────────────────────────────────────────────────────────────────────────

NETWORK="testnet"
CONFIRM=false
RESUME=false
ESTIMATE_ONLY=false
DRY_RUN=false

while [ "$#" -gt 0 ]; do
    case "$1" in
        --confirm)
            CONFIRM=true
            shift
            ;;
        --resume)
            RESUME=true
            shift
            ;;
        --estimate-only)
            ESTIMATE_ONLY=true
            shift
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        testnet|mainnet)
            NETWORK="$1"
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [testnet|mainnet] [--confirm] [--resume] [--estimate-only] [--dry-run]"
            echo ""
            echo "Options:"
            echo "  --confirm        Skip interactive confirmation prompt after cost estimation (#602)"
            echo "  --resume         Resume deployment from previous partial state (#603)"
            echo "  --estimate-only  Show estimated deployment costs and exit (#602)"
            echo "  --dry-run        Validate inputs and build WASM without deploying"
            echo "  -h, --help       Show this help message"
            exit 0
            ;;
        *)
            echo "Error: Unknown argument '$1'."
            echo "Usage: $0 [testnet|mainnet] [--confirm] [--resume] [--estimate-only] [--dry-run]"
            exit 1
            ;;
    esac
done

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

# Verify secret key is available (not strictly required if only estimating costs)
STELLAR_SECRET_KEY="${STELLAR_SECRET_KEY:-${SOROBAN_SECRET_KEY:-}}"
if [ -z "${STELLAR_SECRET_KEY:-}" ] && [ "$ESTIMATE_ONLY" = false ]; then
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

# Verify soroban CLI is available when a deployment (or dry run) is requested
if [ "$ESTIMATE_ONLY" = false ] && ! command -v soroban &>/dev/null; then
    echo "Error: soroban CLI is required but not installed."
    exit 1
fi

echo "=== ChainLearn Contract Deployment ==="
[ "$DRY_RUN" = true ] && echo "Mode:     DRY RUN (nothing will be deployed)"
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
PARTIAL_STATE_FILE=".deploy-partial-${NETWORK}.json"

if [ -f "$DEPLOY_FILE" ] && [ "$RESUME" = false ]; then
    echo "Error: Deployment already exists in $DEPLOY_FILE"
    echo "To redeploy, please remove or back up this file first."
    exit 1
fi

# Check for unresumed partial state file
if [ -f "$PARTIAL_STATE_FILE" ] && [ "$RESUME" = false ]; then
    echo "Warning: A previous partial deployment was found in $PARTIAL_STATE_FILE."
    echo "Run with '--resume' to continue from the partial state:"
    echo "  $0 $NETWORK --resume $([ "$CONFIRM" = true ] && echo "--confirm")"
    echo "Or remove $PARTIAL_STATE_FILE to start a fresh deployment."
    exit 1
fi

# Build all contracts first so bytecode sizes are known for estimation and deployment
echo "[1/4] Building contracts..."
cargo build --release --target wasm32-unknown-unknown --workspace

# Shrink WASM before upload: install fees and code rent scale with size (#344).
# Set STRIP_SPEC_DOCS=1 to also strip doc comments from the embedded spec.
"$(dirname "$0")/optimize-wasm.sh"
echo "  Build complete."
echo ""

# ── Deployment Cost Estimation (#602) ─────────────────────────────────────────

WASM_DIR="target/wasm32-unknown-unknown/release"
LEARN_TOKEN_WASM="$WASM_DIR/learn_token.optimized.wasm"
CREDENTIAL_NFT_WASM="$WASM_DIR/credential_nft.optimized.wasm"
PROGRESS_TRACKER_WASM="$WASM_DIR/progress_tracker.optimized.wasm"

calc_file_size() {
    local file="$1"
    if [ -f "$file" ]; then
        stat -c%s "$file" 2>/dev/null || wc -c < "$file" | tr -d ' '
    else
        echo "0"
    fi
}

calc_upload_cost() {
    local size_bytes="$1"
    # Soroban upload fee: ~0.0001 XLM base tx fee + storage rent deposit (~0.001 XLM per KB)
    awk -v b="$size_bytes" 'BEGIN { printf "%.4f", 0.0001 + (b / 1024.0 * 0.001) }'
}

calc_deploy_cost() {
    # Instance deployment creation fee and rent deposit: ~0.0500 XLM per contract
    echo "0.0500"
}

calc_total_cost() {
    local upload="$1"
    local deploy="$2"
    awk -v u="$upload" -v d="$deploy" 'BEGIN { printf "%.4f", u + d }'
}

LT_BYTES=$(calc_file_size "$LEARN_TOKEN_WASM")
CN_BYTES=$(calc_file_size "$CREDENTIAL_NFT_WASM")
PT_BYTES=$(calc_file_size "$PROGRESS_TRACKER_WASM")

LT_KB=$(awk -v b="$LT_BYTES" 'BEGIN { printf "%.1f", b / 1024.0 }')
CN_KB=$(awk -v b="$CN_BYTES" 'BEGIN { printf "%.1f", b / 1024.0 }')
PT_KB=$(awk -v b="$PT_BYTES" 'BEGIN { printf "%.1f", b / 1024.0 }')

LT_UPLOAD_COST=$(calc_upload_cost "$LT_BYTES")
CN_UPLOAD_COST=$(calc_upload_cost "$CN_BYTES")
PT_UPLOAD_COST=$(calc_upload_cost "$PT_BYTES")

LT_DEPLOY_COST=$(calc_deploy_cost)
CN_DEPLOY_COST=$(calc_deploy_cost)
PT_DEPLOY_COST=$(calc_deploy_cost)

LT_TOTAL_COST=$(calc_total_cost "$LT_UPLOAD_COST" "$LT_DEPLOY_COST")
CN_TOTAL_COST=$(calc_total_cost "$CN_UPLOAD_COST" "$CN_DEPLOY_COST")
PT_TOTAL_COST=$(calc_total_cost "$PT_UPLOAD_COST" "$PT_DEPLOY_COST")

GRAND_TOTAL_COST=$(awk -v a="$LT_TOTAL_COST" -v b="$CN_TOTAL_COST" -v c="$PT_TOTAL_COST" 'BEGIN { printf "%.4f", a + b + c }')

echo "=== Deployment Cost Estimation ==="
echo "Network: $NETWORK"
printf "%-18s %12s %18s %18s %16s\n" "Contract" "WASM Size" "WASM Upload (XLM)" "Deploy Cost (XLM)" "Total Est. (XLM)"
echo "------------------------------------------------------------------------------------------------"
printf "%-18s %10s KB %18s %18s %16s\n" "learn-token" "$LT_KB" "~$LT_UPLOAD_COST" "~$LT_DEPLOY_COST" "~$LT_TOTAL_COST"
printf "%-18s %10s KB %18s %18s %16s\n" "credential-nft" "$CN_KB" "~$CN_UPLOAD_COST" "~$CN_DEPLOY_COST" "~$CN_TOTAL_COST"
printf "%-18s %10s KB %18s %18s %16s\n" "progress-tracker" "$PT_KB" "~$PT_UPLOAD_COST" "~$PT_DEPLOY_COST" "~$PT_TOTAL_COST"
echo "------------------------------------------------------------------------------------------------"
echo "Total Estimated Deployment Fee: ~$GRAND_TOTAL_COST XLM"
echo "Recommended Account Balance:    ~$(awk -v g="$GRAND_TOTAL_COST" 'BEGIN { printf "%.2f", g + 1.0 }') XLM (includes minimum reserve)"
echo ""

if [ "$ESTIMATE_ONLY" = true ]; then
    echo "(--estimate-only specified: exiting without deploying)"
    exit 0
fi

# Dry run: all validation, build and optimization above has run; stop before deploying
if [ "$DRY_RUN" = true ]; then
    for wasm in "$LEARN_TOKEN_WASM" "$CREDENTIAL_NFT_WASM" "$PROGRESS_TRACKER_WASM"; do
        if [ ! -s "$wasm" ]; then
            echo "Error: expected optimized WASM not found or empty: $wasm"
            exit 1
        fi
    done
    echo "=== Dry Run: Would Deploy ==="
    echo "Network:  $NETWORK"
    echo "RPC URL:  $RPC_URL"
    echo "  - learn-token:      $LEARN_TOKEN_WASM ($LT_KB KB)"
    echo "  - credential-nft:   $CREDENTIAL_NFT_WASM ($CN_KB KB)"
    echo "  - progress-tracker: $PROGRESS_TRACKER_WASM ($PT_KB KB)"
    echo "Would write contract addresses to: $DEPLOY_FILE"
    echo ""
    echo "(--dry-run specified: exiting without deploying)"
    exit 0
fi

# Confirmation prompt if --confirm was not specified (#602)
if [ "$CONFIRM" = false ]; then
    read -r -p "Do you want to proceed with deployment to $NETWORK? [y/N]: " PROCEED_INPUT
    case "$PROCEED_INPUT" in
        [yY]|[yY][eE][sS])
            echo "Confirmation received. Proceeding with deployment..."
            ;;
        *)
            echo "Deployment cancelled by user."
            exit 0
            ;;
    esac
else
    echo "(--confirm specified: proceeding without prompt)"
fi
echo ""

# On mainnet, perform safety checks. On testnet, skip for faster iteration (#61).
DEPLOY_FLAGS=()
if [ "$NETWORK" = "mainnet" ]; then
    echo "  Mainnet deployment — running safety checks."
else
    DEPLOY_FLAGS+=(--ignore-checks)
fi

# ── State Tracking & Partial Failure Rollback Support (#603) ──────────────────

LEARN_TOKEN_ID=""
CREDENTIAL_NFT_ID=""
PROGRESS_TRACKER_ID=""

if [ "$RESUME" = true ] && [ -f "$PARTIAL_STATE_FILE" ]; then
    echo "Resuming deployment from partial state in $PARTIAL_STATE_FILE..."
    LEARN_TOKEN_ID=$(jq -r '.contracts.learn_token // empty' "$PARTIAL_STATE_FILE")
    CREDENTIAL_NFT_ID=$(jq -r '.contracts.credential_nft // empty' "$PARTIAL_STATE_FILE")
    PROGRESS_TRACKER_ID=$(jq -r '.contracts.progress_tracker // empty' "$PARTIAL_STATE_FILE")
fi

save_partial_state() {
    cat > "$PARTIAL_STATE_FILE" << EOF
{
  "network": "$NETWORK",
  "updated_at": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "contracts": {
    "learn_token": "${LEARN_TOKEN_ID:-}",
    "credential_nft": "${CREDENTIAL_NFT_ID:-}",
    "progress_tracker": "${PROGRESS_TRACKER_ID:-}"
  }
}
EOF
}

handle_deployment_failure() {
    local exit_code="$?"
    local failed_contract="$1"
    echo ""
    echo "================================================================================"
    echo "DEPLOYMENT FAILED during $failed_contract deployment!"
    echo "================================================================================"
    echo "Partial deployment state saved to: $PARTIAL_STATE_FILE"
    echo ""
    echo "Deployed contracts so far:"
    echo "  - learn-token:      ${LEARN_TOKEN_ID:-<not deployed>}"
    echo "  - credential-nft:   ${CREDENTIAL_NFT_ID:-<not deployed>}"
    echo "  - progress-tracker: ${PROGRESS_TRACKER_ID:-<not deployed>}"
    echo ""
    echo "State & Rollback Notice:"
    echo "  Stellar contracts cannot be automatically deleted or rolled back on-chain."
    echo "  The contracts deployed before this failure remain valid on the network."
    echo ""
    echo "Next Steps:"
    echo "  1. To resume deployment from this point once issues are resolved, run:"
    echo "       $0 $NETWORK --resume $([ "$CONFIRM" = true ] && echo "--confirm")"
    echo "  2. To abandon this deployment and start clean, discard the partial state:"
    echo "       rm -f $PARTIAL_STATE_FILE"
    echo "================================================================================"
    exit "$exit_code"
}

# Deploy learn-token
echo "[2/4] Deploying learn-token..."
if [ -n "$LEARN_TOKEN_ID" ] && [[ "$LEARN_TOKEN_ID" =~ ^C[A-Z0-9]{55,62}$ ]]; then
    echo "  learn-token already deployed (resumed): $LEARN_TOKEN_ID"
else
    trap 'handle_deployment_failure "learn-token"' ERR
    LEARN_TOKEN_ID=$(soroban contract deploy \
        --wasm "$LEARN_TOKEN_WASM" \
        --source "$STELLAR_SECRET_KEY" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        "${DEPLOY_FLAGS[@]+"${DEPLOY_FLAGS[@]}"}")
    save_partial_state
    echo "  learn-token deployed: $LEARN_TOKEN_ID"
fi

# Deploy credential-nft
echo "[3/4] Deploying credential-nft..."
if [ -n "$CREDENTIAL_NFT_ID" ] && [[ "$CREDENTIAL_NFT_ID" =~ ^C[A-Z0-9]{55,62}$ ]]; then
    echo "  credential-nft already deployed (resumed): $CREDENTIAL_NFT_ID"
else
    trap 'handle_deployment_failure "credential-nft"' ERR
    CREDENTIAL_NFT_ID=$(soroban contract deploy \
        --wasm "$CREDENTIAL_NFT_WASM" \
        --source "$STELLAR_SECRET_KEY" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        "${DEPLOY_FLAGS[@]+"${DEPLOY_FLAGS[@]}"}")
    save_partial_state
    echo "  credential-nft deployed: $CREDENTIAL_NFT_ID"
fi

# Deploy progress-tracker
echo "[4/4] Deploying progress-tracker..."
if [ -n "$PROGRESS_TRACKER_ID" ] && [[ "$PROGRESS_TRACKER_ID" =~ ^C[A-Z0-9]{55,62}$ ]]; then
    echo "  progress-tracker already deployed (resumed): $PROGRESS_TRACKER_ID"
else
    trap 'handle_deployment_failure "progress-tracker"' ERR
    PROGRESS_TRACKER_ID=$(soroban contract deploy \
        --wasm "$PROGRESS_TRACKER_WASM" \
        --source "$STELLAR_SECRET_KEY" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        "${DEPLOY_FLAGS[@]+"${DEPLOY_FLAGS[@]}"}")
    save_partial_state
    echo "  progress-tracker deployed: $PROGRESS_TRACKER_ID"
fi

# Clear error trap now that all contracts are deployed
trap - ERR

# Write final deployment info to file and validate (#60)
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

# Remove partial state file upon complete success (#603)
rm -f "$PARTIAL_STATE_FILE"

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
