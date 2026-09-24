#!/bin/bash
set -euo pipefail

echo "Building contracts for size measurement..."
cargo build --target wasm32-unknown-unknown --release

WASM_DIR="target/wasm32-unknown-unknown/release"
TARGET_LIMIT_KB=500
OPTIMIZE_SUGGESTION="Consider using 'soroban contract optimize' or reducing dependencies to lower contract size."

echo -e "\nContract\tSize (KB)\tStatus"
echo "------------------------------------------------"

if [ ! -d "$WASM_DIR" ]; then
    echo "Directory $WASM_DIR does not exist. Build may have failed."
    exit 1
fi

find "$WASM_DIR" -maxdepth 1 -name "*.wasm" | while read -r file; do
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
