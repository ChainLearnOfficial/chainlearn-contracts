#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────────────────────
# cli_usage.sh — CLI examples for interacting with ChainLearn contracts
# ──────────────────────────────────────────────────────────────────────────────
#
# This script shows the soroban CLI commands for the complete learner journey.
# Replace the placeholder values before running.
#
# Prerequisites:
#   - soroban CLI v21+
#   - Contracts deployed and initialized
#   - STELLAR_SECRET_KEY environment variable set
#
# Usage:
#   bash examples/cli_usage.sh
# ──────────────────────────────────────────────────────────────────────────────

set -euo pipefail

# ── Configuration ──────────────────────────────────────────────────────

# Replace with your deployed contract IDs (from deployments-testnet.json)
PROGRESS_ID="C..."
TOKEN_ID="C..."
CREDENTIAL_ID="C..."

# Network settings
RPC_URL="https://soroban-testnet.stellar.org:443"
NETWORK_PASSPHRASE="Test SDF Network ; September 2015"

# Admin and learner addresses
ADMIN_ADDRESS="G..."
LEARNER_ADDRESS="G..."

# ── Helper Function ────────────────────────────────────────────────────

invoke() {
    local contract_id="$1"
    shift
    soroban contract invoke \
        --id "$contract_id" \
        --source "$STELLAR_SECRET_KEY" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        -- \
        "$@"
}

echo "=== ChainLearn CLI Examples ==="
echo ""

# ══════════════════════════════════════════════════════════════════════
# 1. ADMIN: Create a Course
# ══════════════════════════════════════════════════════════════════════

echo "[1] Creating course 'rust_101'..."

invoke "$PROGRESS_ID" create_course \
    --course_id "rust_101" \
    --total_modules 3 \
    --total_quizzes 2 \
    --module_ids '["mod_basics","mod_ownership","mod_traits"]' \
    --quiz_ids '["quiz_1","quiz_2"]'

echo "  Course created!"

# ══════════════════════════════════════════════════════════════════════
# 2. ADMIN: Configure Course (optional)
# ══════════════════════════════════════════════════════════════════════

echo "[2] Setting course difficulty to intermediate..."

invoke "$PROGRESS_ID" set_course_difficulty \
    --course_id "rust_101" \
    --difficulty 1

echo "[3] Setting course tags..."

invoke "$PROGRESS_ID" set_course_tags \
    --course_id "rust_101" \
    --tags '["rust","blockchain","stellar"]'

echo "  Course configured!"

# ══════════════════════════════════════════════════════════════════════
# 3. LEARNER: Enroll in a Course
# ══════════════════════════════════════════════════════════════════════

echo "[4] Enrolling learner in 'rust_101'..."

invoke "$PROGRESS_ID" enroll \
    --learner "$LEARNER_ADDRESS" \
    --course_id "rust_101"

echo "  Enrolled!"

# ══════════════════════════════════════════════════════════════════════
# 4. LEARNER: Complete Modules (must be in order)
# ══════════════════════════════════════════════════════════════════════

echo "[5] Completing module 'mod_basics'..."

invoke "$PROGRESS_ID" complete_module \
    --learner "$LEARNER_ADDRESS" \
    --course_id "rust_101" \
    --module_id "mod_basics"

echo "[6] Completing module 'mod_ownership'..."

invoke "$PROGRESS_ID" complete_module \
    --learner "$LEARNER_ADDRESS" \
    --course_id "rust_101" \
    --module_id "mod_ownership"

echo "[7] Completing module 'mod_traits'..."

invoke "$PROGRESS_ID" complete_module \
    --learner "$LEARNER_ADDRESS" \
    --course_id "rust_101" \
    --module_id "mod_traits"

echo "  All modules completed!"

# ══════════════════════════════════════════════════════════════════════
# 4b. LEARNER: Or batch complete modules
# ══════════════════════════════════════════════════════════════════════

# invoke "$PROGRESS_ID" batch_complete_module \
#     --learner "$LEARNER_ADDRESS" \
#     --course_id "rust_101" \
#     --module_ids '["mod_basics","mod_ownership","mod_traits"]'

# ══════════════════════════════════════════════════════════════════════
# 5. LEARNER: Submit Quiz Scores
# ══════════════════════════════════════════════════════════════════════

echo "[8] Submitting quiz score for 'quiz_1'..."

invoke "$PROGRESS_ID" submit_quiz_score \
    --learner "$LEARNER_ADDRESS" \
    --course_id "rust_101" \
    --quiz_id "quiz_1" \
    --score 85

echo "[9] Submitting quiz score for 'quiz_2'..."

invoke "$PROGRESS_ID" submit_quiz_score \
    --learner "$LEARNER_ADDRESS" \
    --course_id "rust_101" \
    --quiz_id "quiz_2" \
    --score 92

echo "  Quiz scores submitted!"

# ══════════════════════════════════════════════════════════════════════
# 5b. LEARNER: Or batch submit quiz scores
# ══════════════════════════════════════════════════════════════════════

# invoke "$PROGRESS_ID" batch_submit_quiz_score \
#     --learner "$LEARNER_ADDRESS" \
#     --course_id "rust_101" \
#     --quiz_scores '[["quiz_1",85],["quiz_2",92]]'

# ══════════════════════════════════════════════════════════════════════
# 6. LEARNER: Retake a Quiz (optional, must improve score)
# ══════════════════════════════════════════════════════════════════════

# invoke "$PROGRESS_ID" retake_quiz \
#     --learner "$LEARNER_ADDRESS" \
#     --course_id "rust_101" \
#     --quiz_id "quiz_1" \
#     --new_score 95

# ══════════════════════════════════════════════════════════════════════
# 7. QUERY: Check Progress
# ══════════════════════════════════════════════════════════════════════

echo "[10] Checking learner progress..."

invoke "$PROGRESS_ID" get_progress \
    --learner "$LEARNER_ADDRESS" \
    --course_id "rust_101"

echo "[11] Checking completion percentage..."

invoke "$PROGRESS_ID" get_completion_percentage \
    --learner "$LEARNER_ADDRESS" \
    --course_id "rust_101"

echo "[12] Checking credential eligibility..."

invoke "$PROGRESS_ID" is_eligible_for_credential \
    --learner "$LEARNER_ADDRESS" \
    --course_id "rust_101"

echo "[13] Getting course score (average)..."

invoke "$PROGRESS_ID" get_course_score \
    --learner "$LEARNER_ADDRESS" \
    --course_id "rust_101"

# ══════════════════════════════════════════════════════════════════════
# 8. LEARNER: Claim Token Rewards
# ══════════════════════════════════════════════════════════════════════

echo "[14] Claiming reward for 'quiz_1' (85 * 100 = 8500 tokens)..."

invoke "$TOKEN_ID" claim_reward \
    --learner "$LEARNER_ADDRESS" \
    --course_id "rust_101" \
    --quiz_id "quiz_1"

echo "[15] Claiming reward for 'quiz_2' (92 * 100 = 9200 tokens)..."

invoke "$TOKEN_ID" claim_reward \
    --learner "$LEARNER_ADDRESS" \
    --course_id "rust_101" \
    --quiz_id "quiz_2"

echo "  Rewards claimed!"

# ══════════════════════════════════════════════════════════════════════
# 8b. LEARNER: Or batch claim rewards
# ══════════════════════════════════════════════════════════════════════

# invoke "$TOKEN_ID" batch_claim_reward \
#     --learner "$LEARNER_ADDRESS" \
#     --course_id "rust_101" \
#     --quiz_ids '["quiz_1","quiz_2"]'

# ══════════════════════════════════════════════════════════════════════
# 9. QUERY: Check Token Balance
# ══════════════════════════════════════════════════════════════════════

echo "[16] Checking token balance..."

invoke "$TOKEN_ID" balance \
    --address "$LEARNER_ADDRESS"

echo "[17] Viewing claim history..."

invoke "$TOKEN_ID" get_claim_history \
    --learner "$LEARNER_ADDRESS"

# ══════════════════════════════════════════════════════════════════════
# 10. ADMIN: Mint Credential NFT
# ══════════════════════════════════════════════════════════════════════

echo "[18] Minting credential NFT (score must match verified average: 88)..."

invoke "$CREDENTIAL_ID" mint_credential \
    --to "$LEARNER_ADDRESS" \
    --course_id "rust_101" \
    --score 88 \
    --metadata_uri "ipfs://Qm1234567890abcdef"

echo "  Credential minted!"

# ══════════════════════════════════════════════════════════════════════
# 11. QUERY: Verify Credential
# ══════════════════════════════════════════════════════════════════════

echo "[19] Verifying credential..."

invoke "$CREDENTIAL_ID" verify_credential \
    --credential_id 1

echo "[20] Checking credential count..."

invoke "$CREDENTIAL_ID" get_credential_count \
    --learner "$LEARNER_ADDRESS"

echo "[21] Listing learner credentials..."

invoke "$CREDENTIAL_ID" get_credentials_for \
    --learner "$LEARNER_ADDRESS" \
    --start 0 \
    --limit 50

# ══════════════════════════════════════════════════════════════════════
# 12. ADMIN: Emergency Controls
# ══════════════════════════════════════════════════════════════════════

# Pause the contract (prevents all state-changing operations):
# invoke "$PROGRESS_ID" pause --caller "$ADMIN_ADDRESS"
# invoke "$TOKEN_ID" pause --caller "$ADMIN_ADDRESS"

# Unpause:
# invoke "$PROGRESS_ID" unpause --caller "$ADMIN_ADDRESS"
# invoke "$TOKEN_ID" unpause --caller "$ADMIN_ADDRESS"

# ══════════════════════════════════════════════════════════════════════
# 13. ADMIN: Upgrade Contract
# ══════════════════════════════════════════════════════════════════════

# 1. Build and upload new WASM:
# cargo build --release --target wasm32-unknown-unknown
# WASM_HASH=$(soroban contract install \
#     --wasm target/wasm32-unknown-unknown/release/progress_tracker.wasm \
#     --source "$STELLAR_SECRET_KEY" \
#     --rpc-url "$RPC_URL" \
#     --network-passphrase "$NETWORK_PASSPHRASE")

# 2. Upgrade the contract (state is preserved):
# invoke "$PROGRESS_ID" upgrade --new_wasm_hash "$WASM_HASH"

# ══════════════════════════════════════════════════════════════════════
# Done
# ══════════════════════════════════════════════════════════════════════

echo ""
echo "=== CLI Examples Complete ==="
echo ""
echo "See scripts/deploy.sh and scripts/initialize.sh for deployment."
echo "See scripts/upgrade.sh for contract upgrades."
