---
name: Bug report
about: Report incorrect contract behavior, a failing build/test, or a script problem
title: "[Bug] "
labels: bug
assignees: ""
---

<!--
Thanks for reporting a bug! Please fill in as much as you can.

SECURITY: if this is a vulnerability (loss or theft of funds, bypassing auth,
minting without completing a course, etc.) do NOT open a public issue.
Contact the maintainers privately first. See CODE_OF_CONDUCT.md > Reporting.
-->

## Summary

<!-- A clear and concise description of what the bug is. -->

## Affected component

<!-- Check all that apply. -->

- [ ] `learn-token`
- [ ] `credential-nft`
- [ ] `progress-tracker`
- [ ] `packages/shared`
- [ ] Scripts (`scripts/*.sh`)
- [ ] CI / GitHub Actions
- [ ] Documentation
- [ ] Other:

## Steps to reproduce

<!-- Minimal steps, commands, or a failing test that reproduce the issue. -->

1.
2.
3.

```rust
// Optional: a minimal test case using soroban_sdk::Env that reproduces the bug
```

## Expected behavior

<!-- What you expected to happen. -->

## Actual behavior

<!-- What actually happened. Include the panic message, error code, or wrong value. -->

```text
Paste logs, panic output, or the failing `cargo test` output here
```

## Environment

- Network: <!-- local tests / testnet / mainnet -->
- Contract ID(s), if deployed: <!-- e.g. C... -->
- Transaction hash, if applicable:
- Commit / branch: <!-- output of `git rev-parse --short HEAD` -->
- Rust version: <!-- output of `rustc --version` -->
- Stellar / Soroban CLI version: <!-- output of `stellar --version` -->
- OS:

## Impact

<!-- Who is affected and how badly? e.g. blocks learners from claiming rewards. -->

## Possible fix

<!-- Optional: if you know the cause, point to the file/line or suggest a fix. -->

## Checklist

- [ ] I searched [existing issues](https://github.com/ChainLearnOfficial/chainlearn-contracts/issues) and this is not a duplicate
- [ ] I can reproduce this on the latest `main`
- [ ] This is not a security vulnerability (those are reported privately)
