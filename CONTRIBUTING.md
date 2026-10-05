# Contributing to Safeguard Contracts

Thank you for your interest in contributing to Safeguard Contracts! We actively participate in the **Stellar Drips Wave** ecosystem sprints.

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

## Drips Wave Issues

* All tasks for Drips Wave sprints are labeled with `Stellar Wave` and complexity indicators.
* Please link the issue in your PR body (`Closes #123`).
* PRs are reviewed and merged promptly during Wave sprints.
