Closes #226, Closes #225, Closes #224, Closes #223

### Summary of Changes
- **Token Governance Voting (#226)**: Added `Proposal` struct, `create_proposal`, `vote`, and `execute_proposal` functions using token snapshots for voting power.
- **Token Vesting Schedules (#225)**: Added `VestingSchedule` struct, `create_vesting` admin function, and `claim_vested` beneficiary function with linear vesting after cliff.
- **Token Permit Function (#224)**: Added `permit` gasless allowance function with authorization and replay-prevention nonce tracking (`permit_nonce`).
- **Course Completion Certificate Generation (#223)**: Added `generate_certificate` and `get_certificate_uri` functions to generate and store unique certificate URIs on-chain in credential metadata.
- **Repository Maintenance**: Updated `.gitignore` with `mimo` related ignore patterns.
