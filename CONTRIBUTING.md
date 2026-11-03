# Contributing to Safeguard Contracts

Thank you for your interest in contributing to Safeguard Contracts! We welcome community contributions, bug reports, and pull requests to help advance institutional payments and compliance infrastructure on Stellar.

## Code of Conduct

All contributors and maintainers are expected to adhere to our [Code of Conduct](CODE_OF_CONDUCT.md).

## Development Workflow

### Prerequisites
* Rust toolchain (stable, 1.80+)
* Target `wasm32v1-none` or `wasm32-unknown-unknown`
* Soroban CLI (Protocol 22+)

### Setup
```bash
git clone https://github.com/Safeguard-Inc/safeguard-contracts.git
cd safeguard-contracts
rustup target add wasm32v1-none
cargo check
cargo test --workspace
```

### Commit Guidelines
We follow standard Conventional Commits:
* `feat(...)`: New smart contract features or entrypoints
* `fix(...)`: Bug fixes or security patches
* `test(...)`: Unit tests, property tests, or benchmark additions
* `docs(...)`: Documentation or specification improvements
* `chore(...)`: Toolchain, dependency, or formatting updates

### Quality Assurance & Verification
Before submitting any pull request, ensure:
1. **Formatting**: `cargo fmt --all --check`
2. **Linting**: `cargo clippy --workspace --all-targets -- -A deprecated -A clippy::all`
3. **Test Suite**: `cargo test --workspace` (All 163+ tests must pass)
4. **WASM Compilation**: `cargo build --target wasm32v1-none --release -p safeguard-payments -p safeguard-policy`
5. **WASM Size Cap**: Ensure each binary is strictly below 256KB for Soroban deployment.

## Security Vulnerabilities
If you discover a security issue or potential exploit, please refer to [SECURITY.md](SECURITY.md) and report directly to `security@safeguard.inc`. Do not open public GitHub issues for security vulnerabilities.
