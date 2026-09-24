#!/bin/bash
set -euo pipefail

echo "Running test coverage with cargo-tarpaulin..."

# Check if cargo-tarpaulin is installed
if ! command -v cargo-tarpaulin &> /dev/null && ! cargo install --list | grep -q tarpaulin; then
    echo "cargo-tarpaulin not found. You can install it using 'cargo install cargo-tarpaulin'"
    exit 1
fi

cargo tarpaulin \
  --out Html \
  --out Xml \
  --out Lcov \
  --exclude-files "tests/*" \
  --exclude-files "scripts/*" \
  --fail-under 80

echo "Coverage report generated in tarpaulin-report.html, cobertura.xml, and lcov.info"
