# ChainLearn Contracts Upgrade Guide

## Overview

This guide covers the upgrade procedures for ChainLearn smart contracts.

---

## Option C: Full Redeployment

Use this option if the upgrade failed and you need to redeploy all contracts from scratch.

### Steps
1. Stop all services interacting with the contracts.
2. Remove or back up the existing deployment file to avoid the guard in `deploy.sh`:
   ```bash
   mv deployments-<network>.json deployments-<network>.json.backup
   ```
3. Run the deployment script:
   ```bash
   ./scripts/deploy.sh <network>
   ```
4. Verify the new deployment and update any dependent configurations.
5. Restart services.

---

## Notes
- Replace `<network>` with the target network name (e.g., `mainnet`, `testnet`).
- The deployment file guard in `deploy.sh` (lines 62-66) prevents accidental overwrites. Step 2 is required to bypass this.
