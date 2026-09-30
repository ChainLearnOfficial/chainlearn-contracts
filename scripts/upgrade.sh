#!/usr/bin/env bash
set -euo pipefail

# ... (existing content up to line 305 remains unchanged)

# Line 306-312 replacement:
if echo "$UPDATED" | jq empty 2>/dev/null; then
    echo "$UPDATED" > "$DEPLOY_FILE"
    echo "  Deployment file updated: $DEPLOY_FILE"
else
    echo "  Warning: Failed to update deployment file. Current file preserved."
fi

# ... (remaining content after line 312 remains unchanged)