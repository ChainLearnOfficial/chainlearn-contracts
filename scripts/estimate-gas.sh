#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# estimate-gas.sh — Estimate CPU instruction cost for common ChainLearn operations
# ──────────────────────────────────────────────────────────────────────────────
#
# Usage:
#   ./scripts/estimate-gas.sh [limit]
#
# Runs the existing gas_benchmarks test suite (tests/benchmarks/gas_benchmarks.rs),
# which simulates common operations against real contract instances and reports
# the CPU instructions the Soroban host budget records for each — the same
# metric fees are derived from. Results are formatted into a table and compared
# against a configurable per-operation instruction limit.
#
# Note: Soroban's host CPU-instruction count underestimates on-chain WASM
# execution cost, so these are relative estimates for spotting regressions and
# expensive operations, not exact fee quotes.
# ──────────────────────────────────────────────────────────────────────────────

LIMIT="${1:-100000000}"
RAW_OUTPUT=$(mktemp)
trap 'rm -f "$RAW_OUTPUT"' EXIT

echo "=== ChainLearn Gas Estimation ==="
echo "Per-operation CPU instruction limit: $LIMIT"
echo ""
echo "Running gas benchmarks..."
cargo test --test gas_benchmarks -- --nocapture --test-threads=1 2>&1 | tee "$RAW_OUTPUT" >/dev/null

echo ""
printf "%-45s %15s %10s\n" "OPERATION" "CPU INSNS" "STATUS"

OVER_LIMIT=0
FOUND=0
while IFS= read -r line; do
    op=$(echo "$line" | sed -E 's/^bench ([a-zA-Z0-9_→ ]+): ([0-9]+) CPU insns$/\1/')
    cost=$(echo "$line" | sed -E 's/^bench ([a-zA-Z0-9_→ ]+): ([0-9]+) CPU insns$/\2/')
    FOUND=1

    status="OK"
    if [ "$cost" -gt "$LIMIT" ]; then
        status="OVER LIMIT"
        OVER_LIMIT=1
    fi
    printf "%-45s %15s %10s\n" "$op" "$cost" "$status"
done < <(grep -E '^bench .+: [0-9]+ CPU insns$' "$RAW_OUTPUT")

if [ "$FOUND" -eq 0 ]; then
    echo "Error: no benchmark output found. Did 'cargo test --test gas_benchmarks' fail?"
    echo "--- test output ---"
    cat "$RAW_OUTPUT"
    exit 1
fi

echo ""
if [ "$OVER_LIMIT" -eq 1 ]; then
    echo "::warning::One or more operations exceed the $LIMIT CPU instruction limit."
    exit 1
fi

echo "All measured operations are within the $LIMIT CPU instruction limit."
