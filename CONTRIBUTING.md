# Contributing to Safeguard Contracts

Thank you for your interest in contributing to Safeguard Contracts! We welcome community contributions, bug reports, and pull requests.

## Development Workflow

1. **Fork and Clone:**
   ```bash
   git clone https://github.com/Safeguard-Inc/safeguard-contracts.git
   cd safeguard-contracts
   ```

2. **Verify Toolchain:**
   ```bash
   rustup target add wasm32v1-none
   cargo test --workspace
   ```

3. **Branching & Commits:**
   * Create a branch: `git checkout -b feat/your-feature-name` or `fix/issue-description`
   * We follow conventional commits: `feat: ...`, `fix: ...`, `docs: ...`, `test: ...`

4. **Quality Gates before Submitting PR:**
   * Formatting: `cargo fmt --all -- --check`
   * Linting: `cargo clippy --workspace --all-targets -- -D warnings`
   * Tests: `cargo test --workspace`

## Community Backlog

* Open issues are categorized with complexity ratings (`trivial`, `small`, `medium`, `large`) and area labels.
* Please link the relevant issue in your PR body (`Closes #123`).
* PRs are reviewed and merged following standard automated CI and testing workflows.
