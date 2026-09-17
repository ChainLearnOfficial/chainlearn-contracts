# Contributing to ChainLearn Contracts

Thank you for your interest in contributing to ChainLearn smart contracts on Stellar/Soroban! This guide provides instructions for setting up your development environment, adhering to code style guidelines, running tests, and submitting pull requests.

---

## Table of Contents
1. [Code of Conduct](#code-of-conduct)
2. [Getting Started](#getting-started)
   - [Prerequisites](#prerequisites)
   - [Clone and Setup](#clone-and-setup)
3. [Development Workflow](#development-workflow)
   - [Branch Naming](#branch-naming)
   - [Building Contracts](#building-contracts)
   - [Running Tests](#running-tests)
   - [Code Quality & Linting](#code-quality--linting)
4. [Pull Request Process](#pull-request-process)
5. [Security & Vulnerability Reporting](#security--vulnerability-reporting)

---

## Code of Conduct

We are committed to providing a friendly, welcoming, and harassment-free environment for all contributors. Please maintain respectful, constructive, and collaborative discussions across all issues and pull requests.

---

## Getting Started

### Prerequisites
- **Rust**: Version `1.79.0` or later (stable recommended)
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **WASM Target**:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```
- **Soroban CLI** (optional, for local invocation and deployment):
  ```bash
  cargo install --locked soroban-cli
  ```

### Clone and Setup
1. Fork the repository on GitHub.
2. Clone your fork locally:
   ```bash
   git clone https://github.com/<your-username>/chainlearn-contracts.git
   cd chainlearn-contracts
   ```
3. Set the upstream remote:
   ```bash
   git remote add upstream https://github.com/ChainLearnOfficial/chainlearn-contracts.git
   ```

---

## Development Workflow

### Branch Naming
Create a feature or fix branch from `origin/main`:
- `feat/feature-name` for new contract functionality
- `fix/issue-description` for bug fixes
- `docs/documentation-update` for documentation changes
- `test/test-description` for new unit or integration tests

```bash
git checkout -b fix/issue-name origin/main
```

### Building Contracts
Build all workspace packages:
```bash
cargo build --workspace
```

Build contracts in release mode for WebAssembly:
```bash
cargo build --target wasm32-unknown-unknown --release
```

### Running Tests
All changes must be covered by automated tests:
```bash
# Run the full test suite across the workspace
cargo test --workspace

# Run tests for a specific package
cargo test -p learn-token
cargo test -p credential-nft
cargo test -p progress-tracker
```

### Code Quality & Linting
Before opening a pull request, ensure all linters and formatting checks pass cleanly:
```bash
# Check code formatting
cargo fmt --check

# Format code automatically
cargo fmt

# Run Clippy lints
cargo clippy --workspace --all-targets -- -D warnings
```

---

## Pull Request Process

1. **Keep Changes Focused**: Each pull request should address a single issue or feature.
2. **Commit Messages**: Use conventional commit formatting:
   - `feat(contracts): add badge revocation logic`
   - `fix(token): prevent integer overflow in reward calculation`
   - `docs(readme): update deployment instructions`
   - `test(progress): add edge-case quiz retake tests`
3. **Link the Issue**: Include the issue reference in the commit message and PR description:
   - `Closes #123` or `Resolves #123`
4. **Local Verification**: Verify that `cargo test --workspace` passes 100% and no compilation warnings exist before pushing.
5. **Review**: Maintainers will review your PR. Address feedback constructively.

---

## Security & Vulnerability Reporting

If you discover a security vulnerability in ChainLearn smart contracts, please **DO NOT** open a public issue. Instead, report it privately to the maintainers or security team so it can be safely evaluated and patched.
