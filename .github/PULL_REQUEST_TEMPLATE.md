<!--
Thanks for contributing! Please fill in each section as fully as you can.
See CONTRIBUTING.md for the full pull request requirements.
-->

## Summary

<!-- What changed and why. Link the issue this PR addresses: Fixes #NNN -->

## Changes

<!-- List the main changes. Call out affected contracts, public APIs, storage
layouts, events, and deployment behavior. -->

- Affected contracts:
- Public API changes:
- Storage / event changes:
- Deployment impact:

## Testing

<!-- Tests and checks run. PRs must pass CI:
`cargo build --release --target wasm32-unknown-unknown`,
`cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --all -- --check` -->

- [ ] `cargo build --release --target wasm32-unknown-unknown`
- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all -- --check`
- New/updated tests: <!-- describe coverage: happy path, auth failures, validation, boundaries, error paths -->

## Security considerations

<!-- Any security implications, per SECURITY.md. -->

- [ ] Authorization checks preserved on every state-changing entrypoint (`require_auth` on the correct address, called before state reads/writes)
- [ ] No unchecked arithmetic or unbounded storage growth
- [ ] Gas impact assessed (`./scripts/estimate-gas.sh` if execution cost may change):
- [ ] WASM size impact assessed (`./scripts/measure-size.sh` if size may change):

## Breaking Changes

<!-- Clearly labeled. Write `None` when there are no breaking changes. -->

None

## Checklist

- [ ] I read [CONTRIBUTING.md](../CONTRIBUTING.md) and followed the pull request process
- [ ] Documentation and `CHANGELOG.md` updated in this PR (including a **Breaking Changes** entry when applicable)
- [ ] No unrelated formatting, generated files, secrets, or deployment keys included
- [ ] This is not a security vulnerability disclosure (those are reported privately per [SECURITY.md](../SECURITY.md))
