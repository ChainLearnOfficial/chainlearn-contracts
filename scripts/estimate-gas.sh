#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# estimate-gas.sh — Estimate CPU instruction cost for common ChainLearn operations
# ──────────────────────────────────────────────────────────────────────────────
#
# Usage:
#   ./scripts/estimate-gas.sh [limit]
#   ./scripts/estimate-gas.sh --thresholds <file>
#
# Runs the existing gas_benchmarks test suite (tests/benchmarks/gas_benchmarks.rs),
# which simulates common operations against real contract instances and reports
# the CPU instructions the Soroban host budget records for each — the same
# metric fees are derived from. Results are formatted into a table and compared
# against configurable per-operation instruction thresholds.
#
# Thresholds are read from a config file (default: scripts/gas-thresholds.env).
# Each `GAS_LIMIT_<operation>=<insns>` line sets the maximum CPU instructions the
# named operation may report; `DEFAULT_GAS_LIMIT` is the fallback for operations
# without an explicit threshold (0 or unset = not gated). See the header of
# scripts/gas-thresholds.env for how to update the numbers.
#
# Passing a positional argument (e.g. `./scripts/estimate-gas.sh 100000000`)
# keeps the original single-limit behavior and gates every operation against
# that value instead of the config file.
#
# Note: Soroban's host CPU-instruction count underestimates on-chain WASM
# execution cost, so these are relative estimates for spotting regressions and
# expensive operations, not exact fee quotes.
# ──────────────────────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
THRESHOLDS_FILE="${SCRIPT_DIR}/gas-thresholds.env"
SINGLE_LIMIT=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --thresholds)
            [ "$#" -ge 2 ] || { echo "Error: --thresholds requires a file argument."; exit 1; }
            THRESHOLDS_FILE="$2"
            shift 2
            ;;
        -h|--help)
            sed -n '6,26p' "${BASH_SOURCE[0]}"
            exit 0
            ;;
        *)
            SINGLE_LIMIT="$1"
            shift
            ;;
    esac
done

if [ -n "$SINGLE_LIMIT" ]; then
    # Backwards-compatible single-limit mode: same limit for every operation.
    DEFAULT_GAS_LIMIT="$SINGLE_LIMIT"
elif [ -f "$THRESHOLDS_FILE" ]; then
    # shellcheck disable=SC1090
    source "$THRESHOLDS_FILE"
else
    echo "Error: threshold file '$THRESHOLDS_FILE' not found."
    exit 1
fi

DEFAULT_GAS_LIMIT="${DEFAULT_GAS_LIMIT:-0}"

# Resolve the threshold for an operation name (e.g. "create_course"), falling
# back to DEFAULT_GAS_LIMIT. Prints the numeric limit, or an empty string when
# the operation is not gated.
op_limit() {
    local op="$1"
    local var="GAS_LIMIT_${op}"
    local val="${!var:-}"
    if [ -n "$val" ]; then
        echo "$val"
    elif [ "$DEFAULT_GAS_LIMIT" != "0" ] && [ -n "$DEFAULT_GAS_LIMIT" ]; then
        echo "$DEFAULT_GAS_LIMIT"
    fi
}

RAW_OUTPUT=$(mktemp)
trap 'rm -f "$RAW_OUTPUT"' EXIT

echo "=== ChainLearn Gas Estimation ==="
if [ -n "$SINGLE_LIMIT" ]; then
    echo "Threshold source: single limit ($SINGLE_LIMIT CPU insns for all operations)"
else
    echo "Threshold source: $THRESHOLDS_FILE"
fi
echo ""
echo "Running gas benchmarks (via cargo test)..."
# Capture output without letting a failing test abort the script here, so the
# "no benchmark output found" check below can report the failure with context.
cargo test --test gas_benchmarks -- --nocapture --test-threads=1 2>&1 | tee "$RAW_OUTPUT" >/dev/null || true

echo ""
printf "%-45s %15s %15s %10s\n" "OPERATION" "CPU INSNS" "THRESHOLD" "STATUS"

OVER_LIMIT=0
FOUND=0
while IFS= read -r line; do
    op=$(echo "$line" | sed -E 's/^bench ([a-zA-Z0-9_→ ]+): ([0-9]+) CPU insns$/\1/')
    cost=$(echo "$line" | sed -E 's/^bench ([a-zA-Z0-9_→ ]+): ([0-9]+) CPU insns$/\2/')
    FOUND=1

    limit=$(op_limit "$op")
    status="OK"
    if [ -z "$limit" ]; then
        status="NOT GATED"
        limit="—"
    elif [ "$cost" -gt "$limit" ]; then
        status="OVER LIMIT"
        OVER_LIMIT=1
    fi
    printf "%-45s %15s %15s %10s\n" "$op" "$cost" "$limit" "$status"
done < <(grep -E '^bench .+: [0-9]+ CPU insns$' "$RAW_OUTPUT")

if [ "$FOUND" -eq 0 ]; then
    echo "Error: no benchmark output found. Did 'cargo test --test gas_benchmarks' fail?"
    echo "--- test output ---"
    cat "$RAW_OUTPUT"
    exit 1
fi

echo ""
if [ "$OVER_LIMIT" -eq 1 ]; then
    echo "::warning::One or more operations exceed their CPU instruction threshold."
    echo "Update scripts/gas-thresholds.env only if the increase is intentional."
    exit 1
fi

echo "All measured operations are within their CPU instruction thresholds."
