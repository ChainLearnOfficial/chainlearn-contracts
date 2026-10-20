# Pull Request

<!--
Thanks for contributing to ChainLearn! Fill in each section below. See
CONTRIBUTING.md > "Pull request process" for the full requirements this
template mirrors, and SECURITY.md for the security checklist items.
-->

## Summary

<!--
What changed and why. Keep it concise. Link the issue(s) this closes, e.g.
"Closes #123".
-->

-

## Changes

<!--
The concrete change set. Call out affected contracts, public APIs, storage,
events, and deployment behavior. Note anything compatibility-sensitive.
-->

-

## Testing

<!--
List the tests and checks you ran and their results. Every PR must pass the
checks run by CI (see CONTRIBUTING.md > "Testing requirements"):

    cargo build --release --target wasm32-unknown-unknown
    cargo test --workspace
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all -- --check

Add or update tests for every behavior change. Contract changes must cover the
successful path, authorization failures, validation and boundary conditions,
and relevant error paths. Cross-contract changes must include an integration
test. Use ./scripts/estimate-gas.sh for execution-cost impact and
./scripts/measure-size.sh for WASM-size impact.
-->

- [ ] `cargo build --release --target wasm32-unknown-unknown`
- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all -- --check`

## Security Considerations

<!--
Review the relevant items in SECURITY.md and confirm the ones that apply. Note
any gas (./scripts/estimate-gas.sh) or WASM-size (./scripts/measure-size.sh)
impact.
-->

- [ ] Authorization is preserved and `require_auth()` is called before any state read/write on every state-changing entrypoint
- [ ] Role-based access control (Admin/Minter/Pauser) is enforced and revocation takes effect immediately
- [ ] Initialization/re-initialization guards are intact (rejects double-initialization)
- [ ] Arithmetic uses checked or saturating operations; supply and reward bounds hold
- [ ] Cross-contract calls remain read-only and resolve the progress-tracker address once per batch
- [ ] Storage layout changes are documented with a migration/upgrade plan (see docs/upgrade-guide.md)
- [ ] Transfer restrictions and soulbound credential rules are unchanged
- [ ] All state-changing operations emit appropriate events
- [ ] No hardcoded addresses or private keys; no `unsafe`; `#[no_std]` preserved
- [ ] New/changed public functions have doc comments and test coverage for error conditions

## Breaking Changes

<!--
Describe any changes users must make to integrations, configuration, or
deployed state, including migration steps. If there are none, write "None".
-->

None

## Checklist

- [ ] One focused change; no unrelated formatting, generated files, secrets, or deployment keys
- [ ] Branch is created from the current `main`
- [ ] Documentation and `CHANGELOG.md` updated (including a **Breaking Changes** entry when applicable)
- [ ] Required local checks above pass with no failures
- [ ] Linked issue(s) referenced in the Summary
- [ ] All testable acceptance criteria in the linked issue(s) are met
