# Monitoring Guide

This guide describes how to monitor the health and operational performance of
deployed ChainLearn contracts. Monitor testnet and mainnet separately; their
contract IDs, data, RPC endpoints, and risk profiles are different.

## Health checks

The repository provides a baseline health check:

```bash
./scripts/monitor.sh testnet
# or
./scripts/monitor.sh mainnet
```

The script requires Soroban CLI, `jq`, and the matching
`deployments-<network>.json` file. It verifies:

1. the configured RPC endpoint is reachable;
2. `progress-tracker`, `learn-token`, and `credential-nft` report initialized;
3. each contract returns a configured admin; and
4. `learn-token.total_supply` can be read.

Run this check after every deployment, initialization, upgrade, configuration
change, and incident recovery. Schedule it regularly and alert on a non-zero
exit code. A passing result is a liveness baseline, not a guarantee that every
business rule or integration is correct.

## Metrics to track

Collect metrics from RPC, transaction/indexing data, and contract read methods.
Tag every metric with network, contract ID, contract name, and deployment or
WASM version so incidents can be scoped accurately.

| Area | Metrics | Why it matters |
|---|---|---|
| Availability | RPC success rate, request latency, timeout rate, monitor success/failure | Detects loss of ability to read or submit transactions. |
| Transactions | invocation count, success rate, contract error count, failed-transaction reason, confirmation latency | Identifies user-facing failures and regressions. |
| Cost and performance | CPU instructions, transaction fees, WASM size, ledger resource use | Detects cost growth and execution-limit risk. |
| learn-token | `total_supply`, mint/burn volume, reward-claim count and rejection rate, allowance failures | Detects unexpected supply movement and reward-flow issues. |
| credential-nft | credentials minted, revoked credentials, mint rejection rate, pagination/read errors | Detects certificate issuance and verification problems. |
| progress-tracker | enrollments, module completions, quiz submissions, eligible-credential events, course creation/archival activity | Measures learner flow and surfaces content or state errors. |
| Administration | admin/role changes, pauses, upgrades, transfer-restriction changes, progress-tracker address changes | Security-critical changes that require immediate review. |

Use a fixed baseline for normal transaction failures and costs. Compare by
function and release rather than only using aggregate totals; a change in one
entrypoint can be hidden in a global average.

## Alerting rules

Tune thresholds from observed traffic, but begin with these actionable rules:

| Severity | Trigger | First response |
|---|---|---|
| Critical | Health monitor fails twice consecutively, or all RPC requests fail for 10 minutes | Declare an incident, confirm endpoint and network, and stop automated state-changing jobs. |
| Critical | Unexpected admin, role, upgrade, pause, transfer restriction, or progress-tracker-address change | Treat as a potential security event; restrict privileged operations and investigate the transaction immediately. |
| High | Contract invocation error rate exceeds 5% for 15 minutes, excluding known client validation failures | Identify the failing function/error, compare against the latest deployment, and assess user impact. |
| High | Reward claims or credential mints fail at more than twice their established baseline for 30 minutes | Check progress-tracker availability, initialization, eligibility inputs, and recent configuration changes. |
| High | Transaction fee, CPU instruction use, or WASM size rises more than 25% from the approved baseline | Pause rollout, reproduce the workload, and evaluate whether the change risks resource limits. |
| Medium | RPC p95 latency exceeds the service objective for 15 minutes or timeout rate exceeds 1% | Fail over or investigate the RPC provider while continuing read-only verification. |
| Medium | Any monitor run reports missing initialization, missing admin, or unreadable total supply | Verify contract IDs and initialization order; escalate if production configuration changed unexpectedly. |

Alerts must include network, contract ID, affected function, time window,
current value, baseline/threshold, recent deployment version, and a link to the
relevant transaction or dashboard. Avoid placing secret keys or sensitive
operator credentials in alert payloads.

## Incident response

### 1. Triage and contain

1. Acknowledge the alert and assign an incident owner.
2. Identify the network, contract IDs, affected entrypoints, start time, and
   user impact.
3. Preserve transaction hashes, RPC responses, monitor output, and sanitized
   logs.
4. Stop automated deploys, upgrades, and nonessential state-changing jobs for
   the affected network. Do not make an unreviewed production change to clear
   an alert.

### 2. Verify and investigate

Run the health check and compare contract configuration with the expected
deployment record:

```bash
./scripts/monitor.sh <network>
```

Check recent admin, role, pause, upgrade, and configuration transactions first.
Then compare error rates and costs by function before and after the suspected
change. Reproduce safely in tests or on testnet where possible.

### 3. Mitigate and recover

- For an RPC incident, use a verified endpoint or wait for provider recovery;
  confirm reads and transaction status before resuming automation.
- For a contract regression, use the documented upgrade and administrative
  controls only after review. Follow the [upgrade guide](upgrade-guide.md) and
  validate compatibility, tests, and rollback options.
- For a suspected security incident, limit privileged operations, preserve
  evidence, rotate exposed credentials, and follow the private reporting path
  in the [security guide](security.md).

After mitigation, rerun monitoring, validate critical learner flows, and keep
heightened monitoring until metrics return to baseline.

### 4. Close the incident

Document the timeline, impact, root cause, remediation, affected contract and
version, and follow-up actions. Add regression tests, operational safeguards,
and a changelog entry where applicable. Review alert thresholds after learning
from the incident.
