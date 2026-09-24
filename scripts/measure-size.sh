#!/bin/bash
set -euo pipefail

echo "Building contracts for size measurement..."
cargo build --target wasm32-unknown-unknown --release --workspace

WASM_DIR="target/wasm32-unknown-unknown/release"
TARGET_LIMIT_KB=500
OPTIMIZE_SUGGESTION="Consider using 'soroban contract optimize' or reducing dependencies to lower contract size."

echo -e "\nContract\tSize (KB)\tStatus"
echo "------------------------------------------------"

if [ ! -d "$WASM_DIR" ]; then
    echo "Directory $WASM_DIR does not exist. Build may have failed."
    exit 1
fi

find "$WASM_DIR" -maxdepth 1 -name "*.wasm" ! -name "*.optimized.wasm" | while read -r file; do
    size=$(du -k "$file" | cut -f1)
    filename=$(basename "$file")
    if [ "$size" -gt "$TARGET_LIMIT_KB" ]; then
        status="OVER LIMIT"
        suggestion="$OPTIMIZE_SUGGESTION"
    else
        status="OK"
        suggestion=""
    fi
    echo -e "${filename}\t${size}\t${status}"
    if [ -n "$suggestion" ]; then
        echo "  -> $suggestion"
    fi
done

# Sizes after the optimization passes applied at deploy time (#344).
# See docs/wasm-size-audit.md for the full breakdown.
echo -e "\nAfter optimize-wasm.sh (what deploy.sh uploads):"
"$(dirname "$0")/optimize-wasm.sh"
echo -e "\nWith STRIP_SPEC_DOCS=1 (spec doc comments removed):"
"$(dirname "$0")/optimize-wasm.sh" --strip-docs
# Leave the default (docs kept) artifacts in place for deploy/verify.
"$(dirname "$0")/optimize-wasm.sh" >/dev/null
