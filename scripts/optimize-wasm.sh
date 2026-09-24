#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# optimize-wasm.sh — Shrink built contract WASM before install/deploy (#344)
# ──────────────────────────────────────────────────────────────────────────────
#
# Usage:
#   ./scripts/optimize-wasm.sh                 # wasm-opt pass only
#   ./scripts/optimize-wasm.sh --strip-docs    # also strip doc comments from the
#                                              # embedded contract spec
#   STRIP_SPEC_DOCS=1 ./scripts/optimize-wasm.sh
#
# Expects `cargo build --release --target wasm32-unknown-unknown` to have run.
# For each contract, writes `<name>.optimized.wasm` next to the raw build output.
#
# Passes:
#   1. `contract optimize` (binaryen wasm-opt) — dead-code elimination, code
#      folding and local/global reuse on the compiled code. Behavior is unchanged.
#   2. --strip-docs (opt-in) — blanks every `doc` string in the
#      `contractspecv0` custom section. Rust doc comments on contract functions
#      and types are embedded there verbatim and make up roughly a quarter to a
#      third of each binary. Stripping them does not change contract behavior
#      or the callable interface (names, argument and return types are kept),
#      but explorers and `contract bindings` generated from the on-chain WASM
#      will no longer show the docs. See docs/wasm-size-audit.md.
#
# Prerequisites:
#   - stellar (or soroban) CLI with the `opt` feature (default in release builds)
#   - python3 (for --strip-docs)
# ──────────────────────────────────────────────────────────────────────────────

WASM_DIR="target/wasm32-unknown-unknown/release"
CONTRACTS="learn_token credential_nft progress_tracker"
STRIP_DOCS="${STRIP_SPEC_DOCS:-0}"

for arg in "$@"; do
    case "$arg" in
        --strip-docs) STRIP_DOCS=1 ;;
        *)
            echo "Error: unknown argument '$arg'"
            exit 1
            ;;
    esac
done

if command -v stellar &>/dev/null; then
    CLI=stellar
elif command -v soroban &>/dev/null; then
    CLI=soroban
else
    echo "Error: stellar (or soroban) CLI is required but not installed."
    exit 1
fi

if [ "$STRIP_DOCS" = "1" ] && ! command -v python3 &>/dev/null; then
    echo "Error: python3 is required for --strip-docs."
    exit 1
fi

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

# Blank all `doc` fields in the contractspecv0 section of $1, writing to $2.
# The spec is decoded and re-encoded with the CLI's own XDR codec so every
# ScSpecEntry variant is handled; python only splices the WASM sections.
strip_spec_docs() {
    local in="$1"
    local out="$2"
    local spec="$TMP_DIR/spec.xdr"
    local stripped="$TMP_DIR/spec.stripped.xdr"

    python3 - extract "$in" "$spec" <<'PY'
import sys

def leb(b, i):
    r = s = 0
    while True:
        x = b[i]; i += 1; r |= (x & 0x7f) << s; s += 7
        if x < 0x80:
            return r, i

_, _, src, dst = sys.argv
b = open(src, "rb").read()
i = 8
while i < len(b):
    sid = b[i]; i += 1
    size, i = leb(b, i)
    end = i + size
    if sid == 0:
        n, j = leb(b, i)
        if b[j:j + n] == b"contractspecv0":
            open(dst, "wb").write(b[j + n:end])
            sys.exit(0)
    i = end
sys.exit("contractspecv0 section not found in " + src)
PY

    "$CLI" xdr decode --type ScSpecEntry --input stream "$spec" \
        | python3 -c '
import json, sys

def blank(x):
    if isinstance(x, dict):
        return {k: ("" if k == "doc" else blank(v)) for k, v in x.items()}
    if isinstance(x, list):
        return [blank(v) for v in x]
    return x

for line in sys.stdin:
    if line.strip():
        print(json.dumps(blank(json.loads(line))))
' > "$TMP_DIR/spec.jsonl"
    "$CLI" xdr encode --type ScSpecEntry --output stream "$TMP_DIR/spec.jsonl" > "$stripped"

    python3 - replace "$in" "$stripped" "$out" <<'PY'
import sys

def leb(b, i):
    r = s = 0
    while True:
        x = b[i]; i += 1; r |= (x & 0x7f) << s; s += 7
        if x < 0x80:
            return r, i

def enc(n):
    out = bytearray()
    while True:
        x = n & 0x7f; n >>= 7
        out.append(x | (0x80 if n else 0))
        if not n:
            return bytes(out)

_, _, src, spec, dst = sys.argv
b = open(src, "rb").read()
new_spec = open(spec, "rb").read()
name = b"contractspecv0"
out = bytearray(b[:8])
i = 8
while i < len(b):
    start = i
    sid = b[i]; i += 1
    size, i = leb(b, i)
    end = i + size
    if sid == 0:
        n, j = leb(b, i)
        if b[j:j + n] == name:
            body = enc(len(name)) + name + new_spec
            out += bytes([0]) + enc(len(body)) + body
            i = end
            continue
    out += b[start:end]
    i = end
open(dst, "wb").write(out)
PY
}

echo -e "Contract\tRaw (B)\tOptimized (B)\tReduction"
echo "--------------------------------------------------------"

for name in $CONTRACTS; do
    raw="$WASM_DIR/$name.wasm"
    optimized="$WASM_DIR/$name.optimized.wasm"
    if [ ! -f "$raw" ]; then
        echo "Error: $raw not found. Run cargo build --release --target wasm32-unknown-unknown first."
        exit 1
    fi

    "$CLI" contract optimize --wasm "$raw" --wasm-out "$optimized" >/dev/null 2>&1

    if [ "$STRIP_DOCS" = "1" ]; then
        strip_spec_docs "$optimized" "$TMP_DIR/$name.wasm"
        mv "$TMP_DIR/$name.wasm" "$optimized"
    fi

    raw_size=$(wc -c < "$raw")
    opt_size=$(wc -c < "$optimized")
    pct=$(( (raw_size - opt_size) * 100 / raw_size ))
    echo -e "${name}\t${raw_size}\t${opt_size}\t-${pct}%"
done

if [ "$STRIP_DOCS" = "1" ]; then
    echo ""
    echo "Spec doc comments stripped."
fi
