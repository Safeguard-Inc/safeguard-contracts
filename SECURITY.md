# Security Policy

Safeguard is institutional-grade financial infrastructure built on Stellar and Soroban. We take smart contract security, cryptographic integrity, and compliance privacy with the highest level of rigor.

## Supported Versions

Only the latest release and active testnet deployments receive security patches:

| Version | Supported          |
| ------- | ------------------ |
| 1.0.x   | :white_check_mark: |
| < 1.0   | :x:                |

## Reporting a Vulnerability

**Please do NOT disclose security vulnerabilities publicly through GitHub issues or discussions.**

If you discover a vulnerability within Safeguard contracts, backend services, or console applications:

1. **Email Findings**: Send details to **`security@safeguard.inc`**.
2. **Encrypted Submission**: If transmitting sensitive PoC exploits, please encrypt your email with our PGP public key available at `https://safeguard-docs.vercel.app/security.asc`.
3. **Include Details**:
   - Contract addresses / affected modules (`safeguard-payments`, `safeguard-policy`, `safeguard-core`)
   - Detailed step-by-step reproduction steps or Rust Soroban test cases
   - Impact assessment (funds at risk, unauthorized authority escalation, consensus desync)
   - Proposed remediations if available

## Response Timelines

* **Initial Acknowledgement**: Within 24 hours of receipt.
* **Triage & Severity Rating**: Within 48 hours according to CVSS v3.1.
* **Fix & Testnet Verification**: Within 7 business days for critical issues.
* **Public Disclosure**: Coordinated after deployment of patches and timelocked guardian updates.

## Bug Bounty & Recognition

We coordinate bug bounty rewards for critical and high-severity discoveries impacting on-chain funds or policy enforcement integrity. Responsible reporters will be credited in our release notes and audit disclosures.
