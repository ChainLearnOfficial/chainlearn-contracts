# Integration Guide

This document covers SDK setup, contract interaction patterns, error handling, and best practices for integrating with ChainLearn contracts.

## SDK Setup

### Prerequisites

- **Soroban SDK** for Rust: `soroban-sdk = "21.0.0"`
- **Stellar SDK** for JavaScript/TypeScript: `@stellar/stellar-sdk`
- **Network Configuration**:
  - Testnet RPC: `https://soroban-testnet.stellar.org:443`
  - Network Passphrase: `Test SDF Network ; September 2015`

### Rust Integration

```toml
# Cargo.toml
[dependencies]
soroban-sdk = "21.0.0"
```

```rust
use soroban_sdk::{Address, Env, Symbol};

// Initialize environment
let env = Env::default();
let contract_id = Address::from_string(&env, "<CONTRACT_ID>");
let client = soroban_sdk::contractclient!(<ContractName>Client);
```

### JavaScript/TypeScript Integration

```typescript
import * as SorobanClient from "@stellar/stellar-sdk";

// Connect to Soroban RPC
const server = new SorobanClient.SorobanRpc.Server(
  "https://soroban-testnet.stellar.org:443",
);

// Load contract
const contract = new SorobanClient.Contract("<CONTRACT_ID>", server);
```

## Contract Interaction Patterns

### 1. Progress Tracker

#### Create a Course (Admin)

```rust
// Rust
client.create_course(
    &course_id,
    &total_modules,
    &total_quizzes,
    &module_ids,
    &quiz_ids,
);
```

```typescript
// TypeScript
await contract.createCourse({
  course_id: "rust_101",
  total_modules: 3,
  total_quizzes: 2,
  module_ids: ["mod_basics", "mod_ownership", "mod_traits"],
  quiz_ids: ["quiz_1", "quiz_2"],
});
```

#### Enroll in a Course

```rust
// Rust
client.enroll(&learner, &course_id);
```

```typescript
// TypeScript
await contract.enroll({
  learner: learnerAddress,
  course_id: "rust_101",
});
```

#### Complete a Module

```rust
// Rust
client.complete_module(&learner, &course_id, &module_id);
```

```typescript
// TypeScript
await contract.completeModule({
  learner: learnerAddress,
  course_id: "rust_101",
  module_id: "mod_basics",
});
```

#### Submit Quiz Score

```rust
// Rust
client.submit_quiz_score(&learner, &course_id, &quiz_id, &score);
```

```typescript
// TypeScript
await contract.submitQuizScore({
  learner: learnerAddress,
  course_id: "rust_101",
  quiz_id: "quiz_1",
  score: 85,
});
```

#### Get Progress

```rust
// Rust
let progress = client.get_progress(&learner, &course_id);
println!("Progress: {}%", progress.overall_progress);
println!("Eligible: {}", progress.eligible_for_credential);
```

```typescript
// TypeScript
const progress = await contract.getProgress({
  learner: learnerAddress,
  course_id: "rust_101",
});
console.log(`Progress: ${progress.overall_progress}%`);
console.log(`Eligible: ${progress.eligible_for_credential}`);
```

### 2. Learn Token

#### Claim Reward

```rust
// Rust
client.claim_reward(&learner, &course_id, &quiz_id);
```

```typescript
// TypeScript
await contract.claimReward({
  learner: learnerAddress,
  course_id: "rust_101",
  quiz_id: "quiz_1",
});
```

#### Batch Claim Rewards

```rust
// Rust
let successful = client.batch_claim_reward(&learner, &course_id, &quiz_ids);
println!("Claimed {} rewards", successful.len());
```

```typescript
// TypeScript
const successful = await contract.batchClaimReward({
  learner: learnerAddress,
  course_id: "rust_101",
  quiz_ids: ["quiz_1", "quiz_2", "quiz_3"],
});
```

#### Estimate Claim Gas

```rust
// Rust
let estimate = client.estimate_claim_gas(&learner, &course_id, &quiz_id);
if estimate.would_succeed {
    println!("Estimated reward: {}", estimate.estimated_reward);
} else {
    println!("Would fail: {}", estimate.failure_reason);
}
```

```typescript
// TypeScript
const estimate = await contract.estimateClaimGas({
  learner: learnerAddress,
  course_id: "rust_101",
  quiz_id: "quiz_1",
});
if (estimate.would_succeed) {
  console.log(`Estimated reward: ${estimate.estimated_reward}`);
}
```

#### Get Balance

```rust
// Rust
let balance = client.balance(&address);
println!("Balance: {} tokens", balance);
```

```typescript
// TypeScript
const balance = await contract.balance({ address: walletAddress });
console.log(`Balance: ${balance} tokens`);
```

### 3. Credential NFT

#### Mint Credential (Minter)

```rust
// Rust
let credential_id = client.mint_credential(
    &admin,       // caller (Minter or Admin)
    &learner,
    &course_id,
    &score,
    &metadata_uri,
);
```

```typescript
// TypeScript
const credentialId = await contract.mintCredential({
  caller: adminAddress,   // Minter or Admin
  to: learnerAddress,
  course_id: "rust_101",
  score: 85,
  metadata_uri: "ipfs://Qm...",
});
```

#### Verify Credential

```rust
// Rust
let info = client.verify_credential(&credential_id);
println!("Learner: {}", info.learner);
println!("Course: {}", info.course_id);
println!("Score: {}", info.score);
println!("Revoked: {}", info.revoked);
```

```typescript
// TypeScript
const info = await contract.verifyCredential({
  credential_id: credentialId,
});
console.log(`Learner: ${info.learner}`);
console.log(`Score: ${info.score}`);
```

#### Get Learner's Credentials

```rust
// Rust
let count = client.get_credential_count(&learner);
let credentials = client.get_credentials_for(&learner, &0, &50);
```

```typescript
// TypeScript
const count = await contract.getCredentialCount({ learner: learnerAddress });
const credentials = await contract.getCredentialsFor({
  learner: learnerAddress,
  start: 0,
  limit: 50,
});
```

#### Check Credential Validity

```rust
// Rust
let is_valid = client.is_credential_valid(&credential_id);
```

```typescript
// TypeScript
const isValid = await contract.isCredentialValid({
  credential_id: credentialId,
});
```

### 4. Admin Transfer (Two-Step Handover)

All three contracts (`progress-tracker`, `learn-token`, `credential-nft`) use the
same delayed handover. `transfer_admin` does **not** change the admin
immediately — it records a pending candidate, and control only moves once that
candidate accepts. The default delay is 48 hours (172,800s) on every contract.

```rust
// 1. Current admin proposes a new admin. Admin is unchanged by this call.
contract.transfer_admin(&new_admin);

// 2. Optionally, the current admin can abort at any point before acceptance.
contract.cancel_admin_transfer();

// 3. The candidate accepts, but only after the delay has elapsed.
contract.accept_admin();
```

| Function                   | Auth    | Returns                        | Description                                                 |
| -------------------------- | ------- | ------------------------------ | ----------------------------------------------------------- |
| `transfer_admin`           | Admin   | —                              | Record `new_admin` as pending; does not transfer.           |
| `accept_admin`             | Pending | —                              | Complete the transfer once the delay has elapsed.           |
| `cancel_admin_transfer`    | Admin   | —                              | Clear the pending transfer.                                 |
| `pending_admin`            | —       | `Option<PendingAdminTransfer>` | Inspect the in-flight transfer.                             |
| `admin_transfer_delay`     | —       | `u64`                          | Current delay in seconds.                                   |
| `set_admin_transfer_delay` | Admin   | —                              | Set the delay for future transfers (minimum 3,600 seconds). |

Notes for integrators:

- `accept_admin` reverts with `admin transfer delay has not elapsed` if called
  too early, and with `no pending admin transfer` if nothing is in flight.
- Calling `transfer_admin` again before acceptance **overwrites** the candidate
  and restarts the delay; it does not queue a second transfer.
- `set_admin_transfer_delay` only affects transfers started after the call — it
  never shortens or extends a transfer that is already pending. It panics with
  `admin transfer delay is below the minimum` unless `delay_seconds` is at
  least 3,600s (one hour): a zero or near-zero delay would collapse the
  two-step handoff into a single-step one, defeating the point of the guard.
- Every progress-tracker event indexes the acting addresses/ids as topic
  slots, not just in its data payload: learner/course events use
  `[event_name, learner, course_id]`, course-admin events use
  `[event_name, course_id]`, and learner-only events use
  `[event_name, learner]`. Filters match server-side, so e.g. "every module
  completed by learner X" or "every event for course Y" never needs a
  full scan-and-decode of the payload.
- A client can watch `admin_transfer_initiated`, `admin_transfer_accepted` and
  `admin_transfer_cancelled` events to follow the lifecycle of one candidate.
  All three share the pending address in the second topic slot.

### 5. Governance

#### Create Proposal (Admin)

```rust
// Rust
client.create_proposal(
    &proposal_id,
    &title,
    &description,
    &proposal_type,
    &execution_data,
);
```

```typescript
// TypeScript
await contract.createProposal({
  proposal_id: "prop_001",
  title: "Increase reward pool",
  description: "Increase reward pool by 10%",
  proposal_type: "parameter_change",
  execution_data: Buffer.from("10"),
});
```

#### Vote on Proposal

```rust
// Rust
client.vote(
    &voter,
    &proposal_id,
    &vote_type, // 'for' or 'against'
);
```

```typescript
// TypeScript
await contract.vote({
  voter: voterAddress,
  proposal_id: "prop_001",
  vote_type: "for",
});
```

#### Execute Proposal (Admin)

```rust
// Rust
client.execute_proposal(&proposal_id);
```

```typescript
// TypeScript
await contract.executeProposal({
  proposal_id: "prop_001",
});
```

#### Get Proposal Details

```rust
// Rust
let proposal = client.get_proposal(&proposal_id);
println!("Proposal: {}", proposal.title);
println!("Votes For: {}", proposal.votes_for);
println!("Votes Against: {}", proposal.votes_against);
println!("Status: {}", proposal.status);
```

```typescript
// TypeScript
const proposal = await contract.getProposal({
  proposal_id: "prop_001",
});
console.log(`Proposal: ${proposal.title}`);
console.log(`Votes For: ${proposal.votes_for}`);
console.log(`Status: ${proposal.status}`);
```

#### Check Voting Power (Snapshot-Based)

```rust
// Rust
let voting_power = client.get_voting_power(&voter, &snapshot_block);
println!("Voting power at snapshot: {}", voting_power);
```

```typescript
// TypeScript
const votingPower = await contract.getVotingPower({
  voter: voterAddress,
  snapshot_block: 123456,
});
console.log(`Voting power: ${votingPower}`);
```

#### Error Handling for Governance

```rust
// Rust
match panic::catch_unwind(AssertUnwindSafe(|| {
    client.vote(&voter, &proposal_id, &"for".to_string());
})) {
    Ok(_) => println!("Vote registered"),
    Err(_) => println!("Vote failed - check proposal status or voting eligibility"),
}
```

```typescript
// TypeScript
try {
  await contract.vote({
    voter: voterAddress,
    proposal_id: "prop_001",
    vote_type: "for",
  });
  console.log("Vote registered");
} catch (error) {
  if (error.message.includes("voting period expired")) {
    console.error("Voting period has ended");
  } else if (error.message.includes("insufficient voting power")) {
    console.error("Insufficient voting power at snapshot");
  } else {
    console.error("Vote failed:", error.message);
  }
}
```

### 6. Vesting

#### Create Vesting Schedule (Admin)

```rust
// Rust
client.create_vesting_schedule(
    &beneficiary,
    &amount,
    &start_time,
    &cliff_time,
    &end_time,
);
```

```typescript
// TypeScript
await contract.createVestingSchedule({
  beneficiary: beneficiaryAddress,
  amount: "1000000",
  start_time: Math.floor(Date.now() / 1000),
  cliff_time: Math.floor(Date.now() / 1000) + 30 * 24 * 60 * 60, // 30 days
  end_time: Math.floor(Date.now() / 1000) + 365 * 24 * 60 * 60, // 1 year
});
```

#### Claim Vested Tokens

```rust
// Rust
client.claim_vested_tokens(&beneficiary, &vesting_id);
```

```typescript
// TypeScript
await contract.claimVestedTokens({
  beneficiary: beneficiaryAddress,
  vesting_id: "vest_001",
});
```

#### Get Vesting Schedule Details

```rust
// Rust
let schedule = client.get_vesting_schedule(&beneficiary, &vesting_id);
println!("Total amount: {}", schedule.amount);
println!("Cliff time: {}", schedule.cliff_time);
println!("End time: {}", schedule.end_time);
println!("Claimed so far: {}", schedule.claimed_amount);
```

```typescript
// TypeScript
const schedule = await contract.getVestingSchedule({
  beneficiary: beneficiaryAddress,
  vesting_id: "vest_001",
});
console.log(`Total amount: ${schedule.amount}`);
console.log(`Claimed so far: ${schedule.claimed_amount}`);
```

#### Calculate Claimable Amount

```rust
// Rust
let claimable = client.get_claimable_amount(&beneficiary, &vesting_id);
println!("Claimable now: {}", claimable);
```

```typescript
// TypeScript
const claimable = await contract.getClaimableAmount({
  beneficiary: beneficiaryAddress,
  vesting_id: "vest_001",
});
console.log(`Claimable now: ${claimable}`);
```

#### Error Handling for Vesting

```rust
// Rust
match panic::catch_unwind(AssertUnwindSafe(|| {
    client.claim_vested_tokens(&beneficiary, &vesting_id);
})) {
    Ok(_) => println!("Tokens claimed"),
    Err(_) => println!("Claim failed - check cliff time or insufficient vesting"),
}
```

```typescript
// TypeScript
try {
  await contract.claimVestedTokens({
    beneficiary: beneficiaryAddress,
    vesting_id: "vest_001",
  });
  console.log("Tokens claimed successfully");
} catch (error) {
  if (error.message.includes("cliff not reached")) {
    console.error("Cliff period has not elapsed");
  } else if (error.message.includes("no tokens available")) {
    console.error("No vested tokens available to claim");
  } else {
    console.error("Claim failed:", error.message);
  }
}
```

### 7. Transfer Restrictions and Whitelist

#### Transfer with Whitelist Check

```rust
// Rust
client.transfer(&from, &to, &amount);
// Transfer fails if recipient not on whitelist
```

```typescript
// TypeScript
try {
  await contract.transfer({
    from: senderAddress,
    to: recipientAddress,
    amount: "1000",
  });
  console.log("Transfer successful");
} catch (error) {
  if (error.message.includes("not on whitelist")) {
    console.error("Recipient not on whitelist");
  } else {
    console.error("Transfer failed:", error.message);
  }
}
```

#### Add Address to Whitelist (Admin)

```rust
// Rust
client.add_to_whitelist(&address);
```

```typescript
// TypeScript
await contract.addToWhitelist({
  address: addressToWhitelist,
});
```

#### Check Whitelist Status

```rust
// Rust
let is_whitelisted = client.is_whitelisted(&address);
println!("Whitelisted: {}", is_whitelisted);
```

```typescript
// TypeScript
const isWhitelisted = await contract.isWhitelisted({
  address: addressToCheck,
});
console.log(`Whitelisted: ${isWhitelisted}`);
```

## Error Handling

### Common Errors

| Error                        | Cause                              | Solution                          |
| ---------------------------- | ---------------------------------- | --------------------------------- |
| `"already enrolled"`         | Learner already enrolled in course | Check enrollment status first     |
| `"module already completed"` | Module already marked complete     | Use `get_progress` to check       |
| `"quiz already submitted"`   | Quiz score already submitted       | Use `get_quiz_score` to check     |
| `"reward already claimed"`   | Reward already claimed for quiz    | Use `is_reward_claimed` to check  |
| `"not authorized"`           | Caller lacks required permissions  | Ensure correct admin/user calling |
| `"score exceeds maximum"`    | Score > 100                        | Keep score <= 100                 |
| `"course not found"`         | Invalid course_id                  | Verify course exists              |
| `"insufficient balance"`     | Not enough tokens                  | Check balance first               |
| `"credential not found"`     | Invalid credential_id              | Verify credential exists          |

### Error Handling Pattern

```rust
// Rust - Handle panics with catch_unwind
use std::panic::{self, AssertUnwindSafe};

let result = panic::catch_unwind(AssertUnwindSafe(|| {
    client.claim_reward(&learner, &course_id, &quiz_id);
}));

match result {
    Ok(_) => println!("Reward claimed successfully"),
    Err(_) => println!("Failed to claim reward"),
}
```

```typescript
// TypeScript - Use try/catch
try {
  await contract.claimReward({
    learner: learnerAddress,
    course_id: "rust_101",
    quiz_id: "quiz_1",
  });
  console.log("Reward claimed successfully");
} catch (error) {
  console.error("Failed to claim reward:", error.message);
}
```

### Idempotent Operations

Some operations are safe to retry:

- `enroll` - Check `is_enrolled` first
- `complete_module` - Check `get_progress` first
- `submit_quiz_score` - Check `get_quiz_score` first

## Best Practices

### 1. Pre-flight Checks

Always check state before making state-changing calls:

```rust
// Check enrollment
if !is_enrolled(&learner, &course_id) {
    client.enroll(&learner, &course_id);
}

// Check if module completed
let progress = client.get_progress(&learner, &course_id);
if !is_module_completed(&progress, module_index) {
    client.complete_module(&learner, &course_id, &module_id);
}
```

### 2. Use Estimate Functions

Preview operations before executing:

```rust
let estimate = client.estimate_claim_gas(&learner, &course_id, &quiz_id);
if estimate.would_succeed {
    // Safe to proceed
    client.claim_reward(&learner, &course_id, &quiz_id);
} else {
    // Handle failure reason
    println!("Cannot claim: {}", estimate.failure_reason);
}
```

### 3. Batch Operations

Use batch functions for efficiency:

```rust
// Instead of individual calls
for quiz_id in quiz_ids.iter() {
    client.claim_reward(&learner, &course_id, &quiz_id);
}

// Use batch
client.batch_claim_reward(&learner, &course_id, &quiz_ids);
```

### 4. Event Monitoring

#### Testing Environments (Rust)

In testing environments, inspect emitted events with `env.events().all()`:

```rust
// Test environment: Inspect emitted events
let all_events = env.events().all();
for event in all_events.iter() {
    // Process or assert on emitted contract events
}
```

#### Client Applications - Polling Events

For off-chain indexers and client applications, use the Soroban RPC `getEvents` method to poll for events:

```rust
// Rust: Poll for events using Soroban RPC
use soroban_sdk::Address;

let contract_id = Address::from_string(&env, "<CONTRACT_ID>");
let rpc_url = "https://soroban-testnet.stellar.org:443";

// Build a GetEventsRequest to query reward_claimed events
// Note: Event filtering requires polling via RPC (no real-time subscription)
```

```typescript
// TypeScript: Poll for events using SorobanRpc
import * as StellarSdk from "@stellar/stellar-sdk";

const server = new StellarSdk.SorobanRpc.Server(
  "https://soroban-testnet.stellar.org:443",
);

// Query for reward_claimed events from the contract
const events = await server.getEvents({
  contractIds: ["CABC...XYZ"], // Contract address
  filters: [
    {
      type: "contract",
      contractIds: ["CABC...XYZ"],
    },
  ],
  startLedger: 1000,
  limit: 100,
});

// Process the events
for (const event of events.events) {
  console.log("Event:", event.event);
  console.log("Ledger:", event.ledger);
  console.log("Topics:", event.topics);
  console.log("Value:", event.value);
}
```

#### Polling Loop Example

```typescript
// TypeScript: Poll for events periodically
import * as StellarSdk from "@stellar/stellar-sdk";

const server = new StellarSdk.SorobanRpc.Server(
  "https://soroban-testnet.stellar.org:443",
);

let lastLedger = 0;
const contractId = "CABC...XYZ";

async function pollForEvents() {
  try {
    const response = await server.getEvents({
      contractIds: [contractId],
      startLedger: lastLedger + 1,
      limit: 100,
    });

    for (const event of response.events) {
      console.log("New event:", {
        type: event.event.type,
        ledger: event.ledger,
        topics: event.topics,
        value: event.value,
      });
      lastLedger = event.ledger;
    }
  } catch (error) {
    console.error("Failed to fetch events:", error);
  }
}

// Poll every 5 seconds
setInterval(pollForEvents, 5000);
```

#### Event Data Retention

Note: Soroban RPC retains events for **7 days maximum**. For long-term event archival, store events in an off-chain database or indexer as you receive them.

### 5. Pagination

Use pagination for large datasets:

```rust
let page_size = 50;
let mut start = 0;
let mut all_credentials = Vec::new();

loop {
    let page = client.get_credentials_for(&learner, &start, &page_size);
    let page_len = page.len();
    all_credentials.extend(page);

    if (page_len as u32) < page_size {
        break;
    }
    start += page_size;
}
```

### 6. Gas Optimization

- Resolve contract addresses once, reuse across calls
- Use batch operations when possible
- Avoid unnecessary storage reads
- Cache frequently accessed data

### 7. Security Considerations

- Always verify authorization before state changes
- Never trust caller-provided scores
- Use cross-contract verification for rewards/credentials
- Implement proper key management for admin operations

## Example Integration Flow

```rust
// Complete learner journey
fn learner_journey(
    env: &Env,
    progress_tracker: &Address,
    learn_token: &Address,
    credential_nft: &Address,
    learner: &Address,
    course_id: &Symbol,
) {
    let tracker_client = ProgressTrackerClient::new(env, progress_tracker);
    let token_client = LearnTokenClient::new(env, learn_token);
    let credential_client = CredentialNftClient::new(env, credential_nft);

    // 1. Enroll
    tracker_client.enroll(learner, course_id);

    // 2. Complete modules and quizzes
    for module_id in course_modules.iter() {
        tracker_client.complete_module(learner, course_id, module_id);
    }

    for quiz_id in course_quizzes.iter() {
        tracker_client.submit_quiz_score(learner, course_id, quiz_id, &85);
        token_client.claim_reward(learner, course_id, quiz_id);
    }

    // 3. Mint credential (requires Minter or Admin)
    if tracker_client.is_eligible_for_credential(learner, course_id) {
        let score = tracker_client.get_course_score(learner, course_id);
        credential_client.mint_credential(
            &admin,         // caller (Minter or Admin)
            learner,
            course_id,
            score,
            &metadata_uri,
        );
    }
}
```

## Related Documentation

- [Architecture](./architecture.md)
- [Security Model](./security.md)
- [Upgrade Guide](./upgrade-guide.md)

## Support

For further integration assistance, please refer to the ChainLearn developer forums or open an issue on GitHub.
