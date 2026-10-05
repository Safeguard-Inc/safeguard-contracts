# Fixtures

Sample compliance data used by tests, examples and tooling. The fixtures map
directly onto the snapshot values the policy engine consumes, so an
`accounts` entry plus a policy document fully determines an expected
decision.

| File | Maps to | Notes |
| ---- | ------- | ----- |
| [`accounts.json`](accounts.json) | `EvaluationInput` facts | Account, status (core `AccountStatus` labels), jurisdiction code, allowlist membership, denylist presence. Addresses are well-formed Stellar-style `G...` fixtures, not live accounts. |
| [`jurisdictions.json`](jurisdictions.json) | Jurisdiction classification | The region universe (permitted/restricted/prohibited) that example policies and account fixtures reference. Every code used by a policy's jurisdiction rule must appear here. |
| [`sanctions.json`](sanctions.json) | Normalized sanctions entries | Validates against `policy-schema/sanctions.schema.json`; models adapter output (subject hash, list id, status, dataset version, effective time, source). |
| [`identity.json`](identity.json) | Identity/verification records | Account → `IdentityStatus` label plus attestation reference and expiry. No PII — attestation references only, mirroring `set_identity` on-chain. |
| [`tokens.json`](tokens.json) | Token registry bindings | Maps each shipped policy to bound confidential-token addresses, mirroring the contract's `bind_token`/`bound_tokens` surface. |
| [`snapshots/ofac-sample.txt`](snapshots/ofac-sample.txt) | Adapter source input | A fictional OFAC-style pipe-delimited snapshot exercising the adapter pipeline (`safeguard dataset build`); includes removed and deliberately unmapped records to demonstrate review items. |

## Consistency rules

- Account `status` and account/jurisdiction codes are validated against the
  fixture schema and `jurisdictions.json`. `XX` is the reserved sentinel for
  an unknown jurisdiction (maps to `RegionStatus::Unknown` and fails closed).
- Every region code in a policy's jurisdiction rule must exist in
  `jurisdictions.json`.
- Sanctions entries must validate against the sanctions schema.
- Identity records must use a known `IdentityStatus` label and carry an attestation reference.
- Token bindings must reference a shipped policy id and a well-formed Stellar address.
- The sample snapshot's normalized output is pinned by the dataset-build CLI test: 5 entries + 1 review item.

`scripts/check-fixtures.py` enforces all of the above; it fails loudly when a
policy or fixture drifts out of sync.