#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# measure-size.sh — Measure ChainLearn contract WASM sizes
# ──────────────────────────────────────────────────────────────────────────────
#
# Usage:
#   ./scripts/measure-size.sh
#
# Builds all contracts in release mode, reports the WASM binary size of each,
# compares it against a soft size limit, and suggests optimizations for any
# contract that exceeds it.
# ──────────────────────────────────────────────────────────────────────────────

# Soroban charges rent/fees based on contract size; 64KiB is a comfortable
# soft ceiling well under the network's hard limit, used here to flag bloat early.
SIZE_LIMIT_BYTES="${SIZE_LIMIT_BYTES:-65536}"
OUT_DIR="target/wasm32-unknown-unknown/release"

CONTRACTS=(
    "learn_token:learn-token"
    "credential_nft:credential-nft"
    "progress_tracker:progress-tracker"
)

echo "=== Building contracts (release) ==="
cargo build --release --target wasm32-unknown-unknown
echo ""

human_size() {
    local bytes="$1"
    if [ "$bytes" -ge 1048576 ]; then
        awk -v b="$bytes" 'BEGIN { printf "%.2f MiB", b / 1048576 }'
    else
        awk -v b="$bytes" 'BEGIN { printf "%.2f KiB", b / 1024 }'
    fi
}

echo "=== ChainLearn Contract WASM Sizes ==="
printf "%-20s %12s %12s %s\n" "CONTRACT" "SIZE" "LIMIT" "STATUS"

OVER_LIMIT=0
for entry in "${CONTRACTS[@]}"; do
    wasm_name="${entry%%:*}"
    crate_name="${entry##*:}"
    wasm_file="$OUT_DIR/${wasm_name}.wasm"

    if [ ! -f "$wasm_file" ]; then
        echo "Error: expected wasm output missing: $wasm_file"
        exit 1
    fi

    size=$(stat -c%s "$wasm_file" 2>/dev/null || stat -f%z "$wasm_file")
    status="OK"
    if [ "$size" -gt "$SIZE_LIMIT_BYTES" ]; then
        status="OVER LIMIT"
        OVER_LIMIT=1
    fi

    printf "%-20s %12s %12s %s\n" "$crate_name" "$(human_size "$size")" "$(human_size "$SIZE_LIMIT_BYTES")" "$status"

    if [ "$status" = "OVER LIMIT" ]; then
        echo ""
        echo "  Suggestions for $crate_name:"
        echo "    - Confirm [profile.release] has opt-level=\"z\", lto=true, codegen-units=1, strip=\"symbols\""
        echo "    - Remove unused dependencies or feature flags pulling in extra code"
        echo "    - Avoid formatting/panic messages with dynamic strings (use soroban_sdk::panic_with_error!)"
        echo "    - Run 'wasm-opt -Oz' as a post-build pass if not already applied"
        echo "    - Split rarely-used admin functionality into a separate contract if size keeps growing"
        echo ""
    fi
done

echo ""
if [ "$OVER_LIMIT" -eq 1 ]; then
    echo "::warning::One or more contracts exceed the ${SIZE_LIMIT_BYTES}-byte soft size limit."
    exit 1
fi

echo "All contracts are within the ${SIZE_LIMIT_BYTES}-byte soft size limit."
