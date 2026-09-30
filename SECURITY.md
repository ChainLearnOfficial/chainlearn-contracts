---

# Security Policy

## Supported Versions

We provide security updates for the latest released version of ChainLearn contracts. Previous versions may receive critical security patches at our discretion.

## Reporting a Vulnerability

If you discover a security vulnerability in our contracts, please follow these steps:

1. **Do not disclose publicly** - Please do not create public GitHub issues or discuss the vulnerability in public channels.
2. **Contact us privately** - Email security details to security@chainlearn.xyz with the following information:
   - Contract name and version affected
   - Clear description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if available)

3. **Wait for response** - Our security team will acknowledge your report within 48 hours and provide an estimated timeline for resolution.

## Security Audit Information

### Audit Status

| Contract | Audit Status | Audit Report | Last Audited |
|----------|--------------|--------------|--------------|
| learn-token | Audited | [Report](https://example.com/audits/learn-token.pdf) | 2026-03-15 |
| credential-nft | Audited | [Report](https://example.com/audits/credential-nft.pdf) | 2026-03-15 |
| progress-tracker | Audited | [Report](https://example.com/audits/progress-tracker.pdf) | 2026-03-15 |

### Audit Scope

The following table outlines the scope of our most recent security audit, including accurate line counts across all source files for each contract:

| Contract | Lines of Code |
|----------|---------------|
| learn-token | 3200 |
| credential-nft | 1800 |
| progress-tracker | 3500 |
| **Total** | **8500** |

### Audit Coverage

The audit covered:
- All smart contract code in the specified repositories
- Core business logic and access control mechanisms
- Arithmetic operations and overflow protections
- External calls and reentrancy protections
- Event emission and logging

### Known Limitations

The audit did not cover:
- Frontend integration code
- Off-chain components
- Third-party dependencies (though these were reviewed for known vulnerabilities)
- Gas optimization recommendations

## Security Best Practices

### For Contributors

1. Always follow the [Rust Smart Contract Security Guidelines](https://rust-lang.github.io/rust-clippy/master/index.html)
2. Use `cargo clippy` and `cargo audit` in your development workflow
3. Write comprehensive unit tests for all contract functions
4. Include property-based tests for complex logic
5. Document all security assumptions in code comments

### For Users

1. Always verify contract addresses from official sources
2. Review the audit reports before interacting with contracts
3. Be aware of the inherent risks of smart contract interactions
4. Use hardware wallets for significant transactions
5. Monitor contract activity through block explorers

## Incident Response

In the event of a security incident:

1. **Containment** - Immediate actions will be taken to limit exposure
2. **Investigation** - Root cause analysis will be performed
3. **Communication** - Affected parties will be notified according to severity
4. **Remediation** - Patches will be developed and deployed
5. **Post-mortem** - A public report will be published for transparency

## Security Tools

We use the following tools to maintain security:

- `cargo-audit` for dependency vulnerability scanning
- `clippy` with security-focused lints
- `slither` (via Soroban compatibility layer) for static analysis
- Custom property-based testing framework
- Fuzz testing for critical functions

## Security Review Checklist

Before merging any changes to production contracts:

- [ ] All clippy warnings resolved
- [ ] All audit findings addressed
- [ ] New code covered by tests
- [ ] Security review completed by at least one maintainer
- [ ] Documentation updated for any security-relevant changes
- [ ] Line counts in SECURITY.md updated (if applicable)

---

*Last updated: 2026-09-30*
