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
import * as SorobanClient from '@stellar/stellar-sdk';

// Connect to Soroban RPC
const server = new SorobanClient.SorobanRpc.Server(
  'https://soroban-testnet.stellar.org:443'
);

// Load contract
const contract = new SorobanClient.Contract(
  '<CONTRACT_ID>',
  server
);
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
  course_id: 'rust_101',
  total_modules: 3,
  total_quizzes: 2,
  module_ids: ['mod_basics', 'mod_ownership', 'mod_traits'],
  quiz_ids: ['quiz_1', 'quiz_2'],
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
  course_id: 'rust_101',
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
  course_id: 'rust_101',
  module_id: 'mod_basics',
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
  course_id: 'rust_101',
  quiz_id: 'quiz_1',
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
  course_id: 'rust_101',
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
  course_id: 'rust_101',
  quiz_id: 'quiz_1',
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
  course_id: 'rust_101',
  quiz_ids: ['quiz_1', 'quiz_2', 'quiz_3'],
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
  course_id: 'rust_101',
  quiz_id: 'quiz_1',
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

#### Mint Credential (Admin)

```rust
// Rust
let credential_id = client.mint_credential(
    &learner,
    &course_id,
    &score,
    &metadata_uri,
);
```

```typescript
// TypeScript
const credentialId = await contract.mintCredential({
  to: learnerAddress,
  course_id: 'rust_101',
  score: 85,
  metadata_uri: 'ipfs://Qm...',
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
const isValid = await contract.isCredentialValid({ credential_id: credentialId });
```

## Error Handling

### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `"already enrolled"` | Learner already enrolled in course | Check enrollment status first |
| `"module already completed"` | Module already marked complete | Use `get_progress` to check |
| `"quiz already submitted"` | Quiz score already submitted | Use `get_quiz_score` to check |
| `"reward already claimed"` | Reward already claimed for quiz | Use `is_reward_claimed` to check |
| `"not authorized"` | Caller lacks required permissions | Ensure correct admin/user calling |
| `"score exceeds maximum"` | Score > 100 | Keep score <= 100 |
| `"course not found"` | Invalid course_id | Verify course exists |
| `"insufficient balance"` | Not enough tokens | Check balance first |
| `"credential not found"` | Invalid credential_id | Verify credential exists |

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
    course_id: 'rust_101',
    quiz_id: 'quiz_1',
  });
  console.log('Reward claimed successfully');
} catch (error) {
  console.error('Failed to claim reward:', error.message);
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

### 4. Event Listening

Subscribe to events for real-time updates:

```rust
// Rust - Listen for events
env.events().subscribe(|event| {
    match event.symbol.as_str() {
        "reward_claimed" => { /* Handle reward claimed */ }
        "credential_minted" => { /* Handle credential minted */ }
        "credential_eligible" => { /* Handle eligibility */ }
        _ => {}
    }
});
```

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

    // 3. Mint credential (requires admin)
    if tracker_client.is_eligible_for_credential(learner, course_id) {
        let score = tracker_client.get_course_score(learner, course_id);
        credential_client.mint_credential(
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