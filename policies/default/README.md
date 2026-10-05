# Default Policy

[`policy.json`](policy.json) is the reference default for institutional
Confidential Token deployments. It is a starting point, not a prescription —
every deployment must review the rule set, severities and region lists for
its own jurisdiction before activating it on-chain.

## What it enables and why

| Rule | Action | Rationale |
| ---- | ------ | --------- |
| Allowlist (`ALLOWLIST-001`) | `block` | Membership is required. Institutions typically know exactly who may hold and transact; a required allowlist is the strongest identity gate and the least ambiguous to reason about. |
| Denylist (`DENYLIST-001`) | `block` | Listed subjects are excluded outright. |
| Sanctions (`SANCTIONS-001`) | `block` | A sanctions match under a blocking policy must never approve — this is the property the engine's tests enforce. Screening happens against datasets that enter through reviewed adapters. |
| Jurisdiction (`JURISDICTION-001`) | `flag` | Region classification flags for review rather than blocking. The default deliberately chooses `flag` so a misclassified or unknown region (which also flags, fail-closed) cannot freeze a legitimate user, while `prohibited` regions are still surfaced for review every time. Deployments with stricter regional requirements should switch this rule to `block`. |

## Evaluation order

Rules are evaluated in the fixed engine precedence: allowlist → denylist →
sanctions → jurisdiction, after the structural account-status check (a
frozen/suspended account blocks regardless of rules). See the core evaluator
and the rule-engine documentation for the authoritative order.

## Region lists

- `permitted` — regions that may hold and transact.
- `restricted` — regions that need review; under this policy they flag.
- `prohibited` — regions that may not transact; under this policy they flag
  for review, or block if the action is changed to `block`.
- **Unknown regions fail closed**: they trigger the rule action (here: `flag`)
  and can never silently approve.

Region codes are ISO 3166-1 alpha-2, uppercase.

## Deploying

1. Copy this policy, adjust rule severities and region lists for your
   jurisdiction.
2. Validate: `python3 scripts/validate_policy.py path/to/your/policy.json`.
3. Register and activate it on the policy contract (admin), then bind your
   token addresses (admin or registry authority).