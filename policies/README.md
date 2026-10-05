# Policies

Reference policy documents for Safeguard. Everything here is machine-readable
and validated; treat these files as data, not prose.

| Directory | Contents |
| --------- | -------- |
| [`default/`](default/) | The recommended default configuration and its rationale. |
| [`examples/`](examples/) | Annotated reference policies, one per use case plus a combined policy. |
| [`fixtures/`](fixtures/) | Sample accounts, jurisdiction classification and normalized sanctions data used by tests and tooling. |

## The policy document format

```json
{
  "policy_id": "my-policy",
  "version": 1,
  "rules": [
    { "id": "ALLOWLIST-001", "type": "allowlist", "action": "block" },
    { "id": "SANCTIONS-001", "type": "sanctions", "action": "block" },
    {
      "id": "JURISDICTION-001",
      "type": "jurisdiction",
      "action": "flag",
      "regions": {
        "permitted": ["US", "GB"],
        "restricted": [],
        "prohibited": ["IR", "KP"]
      }
    }
  ]
}
```

- **`policy_id`** — ASCII, 1–32 bytes; becomes the fixed-width id registered
  on-chain.
- **`version`** — positive integer. Policies are append-only: a change is a
  new version, never an edit to a published one.
- **`rules`** — the enabled rule set, **at most one rule per type**, each
  with a unique id. Rule `type` ∈ `allowlist | denylist | sanctions |
  jurisdiction`; `action` ∈ `block | flag`.
- **`regions`** — required on `jurisdiction` rules only. Classifies ISO
  alpha-2 codes as `permitted`, `restricted` or `prohibited`. Restricted,
  prohibited and **unknown** regions trigger the rule's action — never
  silently approve.
- **`metadata`** — free-form deployment context (authority, references).

## What a policy decides

The engine evaluates in fixed precedence order, after a structural account
status check (frozen/suspended blocks, restricted/unknown flags):

```text
account status → allowlist → denylist → sanctions → jurisdiction → APPROVE
```

Absent rule types are skipped; the first decisive check wins. The complete
semantics live in `safeguard-core`'s evaluator and are pinned by its
property tests; see `../policy-schema/README.md` for the machine-readable
contract.

## Working with policies

```bash
# Validate any policy document (schema + invariants)
python3 scripts/validate_policy.py path/to/policy.json

# Full battery: schemas, parity, negative cases, fixture consistency
python3 scripts/test-schema.py
python3 scripts/check-fixtures.py
```

A new policy document should:

1. Follow the format above (copy an example as a starting point).
2. Pass `validate_policy.py` — including the unique-id and one-per-type
   invariants the JSON Schema cannot express.
3. Reference only region codes that exist in `fixtures/jurisdictions.json`
   (enforced by `check-fixtures.py`).
4. Be reviewed for severity before activation: `block` denies, `flag` sends
   to review, and missing compliance information always fails closed.