#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# coverage.sh — Measure test coverage for ChainLearn contracts
# ──────────────────────────────────────────────────────────────────────────────
#
# Usage:
#   ./scripts/coverage.sh [min-coverage-percent]
#
# Prerequisites:
#   - cargo-tarpaulin installed (cargo install cargo-tarpaulin)
#
# Output:
#   - Terminal summary of coverage per file
#   - coverage/tarpaulin-report.html — browsable report
#   - coverage/cobertura.xml — machine-readable report (CI-friendly)
# ──────────────────────────────────────────────────────────────────────────────

MIN_COVERAGE="${1:-70}"
OUT_DIR="coverage"

if ! command -v cargo-tarpaulin &>/dev/null; then
    echo "Error: cargo-tarpaulin is required but not installed."
    echo "Install it with: cargo install cargo-tarpaulin"
    exit 1
fi

mkdir -p "$OUT_DIR"

echo "=== ChainLearn Test Coverage ==="
echo "Minimum coverage threshold: ${MIN_COVERAGE}%"
echo ""

cargo tarpaulin \
    --workspace \
    --exclude-files "target/*" "tests/*" \
    --out Html --out Xml --out Stdout \
    --output-dir "$OUT_DIR" \
    --timeout 300 \
    | tee "$OUT_DIR/coverage-summary.txt"

echo ""
echo "Reports written to:"
echo "  $OUT_DIR/tarpaulin-report.html"
echo "  $OUT_DIR/cobertura.xml"
echo ""

TOTAL_COVERAGE=$(grep -oE '^[0-9]+\.[0-9]+% coverage' "$OUT_DIR/coverage-summary.txt" | tail -1 | grep -oE '^[0-9]+\.[0-9]+' || echo "0")

echo "=== Coverage Gaps (files below ${MIN_COVERAGE}%) ==="
awk -F'|' '/^\|\|/ {next} /^\S.*[0-9]+\/[0-9]+/ {print}' "$OUT_DIR/coverage-summary.txt" || true

if [ -z "$TOTAL_COVERAGE" ]; then
    echo "Warning: could not parse total coverage percentage from tarpaulin output."
    exit 0
fi

echo ""
echo "Total coverage: ${TOTAL_COVERAGE}%"

if awk -v cov="$TOTAL_COVERAGE" -v min="$MIN_COVERAGE" 'BEGIN { exit !(cov < min) }'; then
    echo "::error::Coverage ${TOTAL_COVERAGE}% is below the minimum threshold of ${MIN_COVERAGE}%"
    exit 1
fi

echo "Coverage meets the minimum threshold."
