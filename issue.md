#226 . Add token governance voting
Repo Avatar
ChainLearnOfficial/chainlearn-contracts
What: Add on-chain governance voting using token balances as voting power.

Why: Decentralized governance requires token-weighted voting. Important for protocol upgrades.

Scope: Add Proposal struct with description, choices, start/end time. Add create_proposal, vote, execute_proposal functions. Use token snapshots for voting power.

Acceptance criteria:

Proposals can be created
Token holders can vote
Voting power is token-weighted
Proposals can be executed
Technical context: contracts/learn-token/src/lib.rs — token functions.

#225 . Add token vesting schedules
Repo Avatar
ChainLearnOfficial/chainlearn-contracts
What: Add token vesting schedules that lock tokens until a specified time.

Why: Token vesting prevents immediate selling and aligns incentives. Important for contributor rewards.

Scope: Add VestingSchedule struct with cliff, duration, beneficiary. Add create_vesting admin function. Add claim_vested beneficiary function.

Acceptance criteria:

Vesting schedules are created
Tokens are locked until cliff
Tokens vest linearly after cliff
Beneficiary can claim vested tokens
Technical context: contracts/learn-token/src/storage.rs — storage patterns.

#224 . Add token permit function
Repo Avatar
ChainLearnOfficial/chainlearn-contracts
What: Add a permit function that allows gasless token approvals via off-chain signatures.

Why: Users shouldn't need XLM for gas to approve token spending. Permits enable meta-transactions.

Scope: Add permit(owner, spender, amount, expiration, v, r, s) function. Verify signature off-chain. Set allowance without owner authorization.

Acceptance criteria:

Permit works without owner auth
Signature is verified
Allowance is set
Expiration is respected
Technical context: contracts/learn-token/src/lib.rs:218-237 — approve function.

#223 . Add course completion certificate generation
Repo Avatar
ChainLearnOfficial/chainlearn-contracts
What: Add an on-chain function that generates a certificate URI for completed courses.

Why: Users want verifiable certificates. On-chain generation ensures consistency.

Scope: Add generate_certificate(learner, course_id) function that creates a certificate URI based on learner and course data. Store URI in credential metadata.

Acceptance criteria:

Certificate URI is generated
URI is unique per learner/course
URI is stored on-chain
URI is queryable
Technical context: contracts/credential-nft/src/metadata.rs:4-19 — CredentialInfo.

