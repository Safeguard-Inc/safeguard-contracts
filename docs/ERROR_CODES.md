# Safeguard Canonical Error Code Catalog

This document specifies the complete, authoritative catalog of **270 structured error codes** across the Safeguard platform on Stellar.

## Summary by Operational Domain

| Domain Code Range | Domain Category | Error Count | Description |
| :--- | :--- | :---: | :--- |
| **1000 – 1029** | **Host & Soroban VM** | 30 | Low-level host resource limits, instruction exhaustion, and WASM runtime failures |
| **2000 – 2039** | **Policy Engine & Rules** | 40 | Deterministic policy evaluation, rule precedence, allowlists, and sanctions |
| **3000 – 3039** | **Payment & SAC Tokens** | 40 | Non-custodial payments, Stellar Asset Contracts (SAC), spend caps, and routing |
| **4000 – 4034** | **Escrow & Timelocks** | 35 | Timelocked vault settlement, arbitration disputes, milestones, and releases |
| **5000 – 5029** | **Identity & Sanctions** | 30 | OFAC screening, Travel Rule compliance, Merkle inclusion proofs, and KYC |
| **6000 – 6029** | **Auth & Governance** | 30 | Multi-sig quorum, admin delegation, emergency pause, and role authorization |
| **7000 – 7029** | **SDK & API Client** | 30 | RPC serialization, wallet connectors, network timeouts, and simulation errors |
| **8000 – 8019** | **Audit & Integrity** | 20 | Append-only audit trail verification, Merkle proofs, and hash chain integrity |
| **9000 – 9014** | **Config & Deployment** | 15 | Protocol version checks, deployment configuration, and schema validation |
| **TOTAL** | **All Domains** | **270** | **Enterprise-grade error coverage across the stack** |

---

## Detailed Error Code Reference

### Host & Soroban VM (1000 - 1029)

| Code | Mnemonic | HTTP | Description | Recommended Recovery Action |
| :---: | :--- | :---: | :--- | :--- |
| `1000` | `HOST_BUDGET_EXCEEDED` | `500` | Soroban CPU or memory instruction budget exhausted during invocation | Increase budget simulation limits or batch fewer operations |
| `1001` | `INVALID_LEDGER_SEQUENCE` | `400` | Transaction submitted against an expired or future ledger sequence number | Fetch current ledger sequence from Horizon/RPC before resubmitting |
| `1002` | `REENTRANCY_DETECTED` | `409` | Cross-contract call recursion detected in policy-guarded execution | Refactor contract calls to follow checks-effects-interactions pattern |
| `1003` | `STORAGE_FOOTPRINT_EXCEEDED` | `500` | Transaction footprint exceeds maximum ledger entry read/write limits | Split transaction into smaller batches or optimize data key storage |
| `1004` | `ENTRY_TTL_EXPIRED` | `410` | Target contract instance or storage entry TTL has expired and needs archival restoration | Invoke extend_ttl or restore footprint before contract interaction |
| `1005` | `HOST_OBJECT_HANDLE_INVALID` | `500` | Host object reference is dangling or corrupted in Soroban runtime | Ensure object lifecycle is preserved across cross-contract boundaries |
| `1006` | `HOST_WASM_PARSE_FAILURE` | `500` | WASM bytecode failed static verification on host runtime | Recompile WASM with target wasm32v1-none and verify exports |
| `1007` | `STACK_OVERFLOW` | `500` | Soroban VM call stack depth limit reached | Reduce call depth and simplify nested function execution |
| `1008` | `OUT_OF_GAS` | `500` | Resource fee paid insufficient for instruction units consumed | Estimate gas via simulateTransaction and set adequate resource fees |
| `1009` | `ARITHMETIC_OVERFLOW` | `500` | Integer overflow occurred during host mathematical evaluation | Ensure all amounts fit within i128 checked arithmetic bounds |
| `1010` | `ARITHMETIC_UNDERFLOW` | `500` | Integer underflow occurred during balance deduction | Validate balance before executing sub operations |
| `1011` | `DIVISION_BY_ZERO` | `400` | Attempted division by zero in fee or weight calculation | Ensure divisor is strictly positive before invoking calculation |
| `1012` | `UNREGISTERED_CONTRACT` | `404` | Target contract address does not exist on the current ledger | Verify network and deploy contract to current network before calling |
| `1013` | `INVALID_CONTRACT_TYPE` | `400` | UDT contract type deserialization failed for payload | Match client SDK struct definition with contract ABI specification |
| `1014` | `STORAGE_KEY_COLLISION` | `409` | Instance storage key already exists in write set | Use deterministic unique composite keys for persistent records |
| `1015` | `FOOTPRINT_READ_ONLY_VIOLATION` | `500` | Attempted write to a ledger entry marked read-only in footprint | Include writable entries in transaction footprint write set |
| `1016` | `HOST_CPU_LIMIT_REACHED` | `500` | CPU instruction count hit hard ledger cap | Optimize loops and avoid repeated vector cloning |
| `1017` | `HOST_MEMORY_LIMIT_REACHED` | `500` | Host memory allocation exceeded byte cap | Use compact ByteSlice arrays instead of nested vectors |
| `1018` | `CROSS_CALL_DEPTH_EXCEEDED` | `500` | Cross-contract invocation depth exceeded limit of 10 | Flatten invocation hierarchy or use asynchronous event choreography |
| `1019` | `EVENT_PAYLOAD_TOO_LARGE` | `400` | Contract event emission exceeds maximum payload size | Emit indexed hashes or compact identifiers rather than full bodies |
| `1020` | `INVALID_CONTRACT_ID_FORMAT` | `400` | Contract ID is not a valid 32-byte hex or C... StrKey | Format contract ID according to SEP-0023 StrKey specifications |
| `1021` | `LEDGER_TIMESTAMP_DRIFT` | `400` | Ledger timestamp is outside expected acceptable drift window | Synchronize node clock with Stellar consensus cluster time |
| `1022` | `SOROBAN_INTERNAL_ERROR` | `500` | Unhandled internal error inside host environment | Report issue to Stellar Core repository with host diagnostics |
| `1023` | `RESOURCE_FEE_BELOW_MINIMUM` | `400` | Supplied resource fee does not satisfy network minimum base fee | Query network fee stats endpoint and supply recommended inclusion fee |
| `1024` | `TRANSACTION_MALFORMED` | `400` | Host failed to parse envelope XDR structure | Validate XDR envelope with stellar-sdk before dispatch |
| `1025` | `INVALID_SOURCE_ACCOUNT` | `404` | Transaction source account is not funded on testnet/mainnet | Fund source account with testnet Friendbot or activate account |
| `1026` | `SIGNATURE_VERIFICATION_FAILED` | `401` | Cryptographic signature does not match public key in envelope | Verify private key derivation and signature payload hash |
| `1027` | `NONCE_MISMATCH` | `409` | Account sequence number does not increment prior ledger state | Query latest sequence number from account RPC before signing |
| `1028` | `UNSUPPORTED_HOST_FUNCTION` | `501` | Host function is not available in current protocol version | Verify protocol version compatibility (minimum Protocol 22) |
| `1029` | `STORAGE_ENTRY_NOT_FOUND` | `404` | Target key does not exist in instance or persistent storage | Initialize storage record before attempting read access |


### Policy Engine & Rules (2000 - 2039)

| Code | Mnemonic | HTTP | Description | Recommended Recovery Action |
| :---: | :--- | :---: | :--- | :--- |
| `2000` | `POLICY_NOT_ACTIVE` | `403` | Compliance policy exists but is currently deactivated | Activate policy version via admin credentials |
| `2001` | `POLICY_NOT_FOUND` | `404` | No compliance policy registered for the requested token or identifier | Register policy version before initiating evaluated transfers |
| `2002` | `POLICY_ALREADY_EXISTS` | `409` | A policy with this identifier has already been registered | Bump version sequence number to register an updated policy |
| `2003` | `RULE_PRECEDENCE_MISMATCH` | `422` | Evaluation precedence conflicting between blocking and flagging rules | Ensure rule precedence adheres to fail-closed priority hierarchy |
| `2004` | `RULE_EVALUATION_TIMEOUT` | `504` | Deterministic evaluation exceeded cycle limit | Simplify rule complexity and reduce nested rule references |
| `2005` | `ALLOWLIST_REQUIRED` | `403` | Sender or recipient is not present in approved allowlist registry | Submit account KYC proof to be registered on the compliance allowlist |
| `2006` | `DENYLIST_MATCHED` | `403` | Account is explicitly designated on an active denylist | Contact compliance administrator to appeal denylist status |
| `2007` | `SANCTIONS_MATCHED` | `403` | Target entity matched an active international sanctions dataset | Transaction blocked due to mandatory sanctions screening rules |
| `2008` | `JURISDICTION_PROHIBITED` | `403` | Transaction originates from or terminates in an embargoed jurisdiction | Comply with regional jurisdictional regulatory requirements |
| `2009` | `JURISDICTION_UNKNOWN` | `422` | Account jurisdiction code is undefined in regional compliance map | Register certified ISO-3166 jurisdiction code for subject |
| `2010` | `ACCOUNT_FROZEN` | `403` | Account has been frozen by regulatory administrator | Await administrative review before retrying transfers |
| `2011` | `POLICY_VERSION_DEPRECATED` | `410` | Policy version has been superseded by a newer version | Upgrade client invocation to active policy version |
| `2012` | `RULE_PAYLOAD_CORRUPTED` | `500` | Rule record serialization in contract storage failed checksum | Re-register corrupted rule version under admin multi-sig |
| `2013` | `INVALID_ACTION_CODE` | `400` | Action code must be one of APPROVE (1), FLAG (2), or BLOCK (3) | Submit valid numeric action code in rule specification |
| `2014` | `INVALID_RULE_TYPE` | `400` | Rule type is not recognized by the evaluation engine | Choose from supported rule types: Allowlist, Denylist, Sanctions, Jurisdiction |
| `2015` | `DUPLICATE_RULE_ID` | `409` | Rule ID already exists within the target policy version | Assign unique UUID or formatted identifier for each rule record |
| `2016` | `MAX_RULES_EXCEEDED` | `400` | Policy version exceeds maximum limit of 256 rule records | Consolidate policy rules or partition into tiered policies |
| `2017` | `POLICY_HASH_MISMATCH` | `400` | Supplied policy configuration hash does not match computed digest | Verify JSON schema and canonical hash ordering before registration |
| `2018` | `TOKEN_NOT_BOUND` | `404` | Token contract address is not bound to this compliance policy | Invoke bind_token to attach policy to Stellar Asset Contract |
| `2019` | `TOKEN_ALREADY_BOUND` | `409` | Token is already bound to another active compliance policy | Unbind previous policy or migrate version bindings |
| `2020` | `RULE_PREDICATE_EVAL_ERROR` | `500` | Dynamic evaluation of rule predicate returned unexpected error | Inspect predicate logic and ensure inputs are within valid range |
| `2021` | `CONDITIONAL_FLAG_TRIGGERED` | `202` | Transaction flagged for asynchronous compliance team investigation | Transaction queued in flagging registry; monitor audit stream |
| `2022` | `SANCTIONS_DATASET_EXPIRED` | `422` | On-chain sanctions snapshot has lapsed its validity epoch | Refresh sanctions registry with updated certified merkle root |
| `2023` | `ALLOWLIST_MEMBERSHIP_EXPIRED` | `403` | Subject allowlist verification validity period has lapsed | Renew identity verification to refresh allowlist timestamp |
| `2024` | `HIGH_RISK_CORRIDOR_BLOCKED` | `403` | Corridor between source and destination jurisdiction is prohibited | Reroute transfer through approved compliant channels |
| `2025` | `VOLUME_VELOCITY_EXCEEDED` | `429` | Account cumulative transfer volume exceeded 24-hour limit | Wait for velocity cooling period or request tier limit increase |
| `2026` | `DAILY_TRANSACTION_COUNT_CAP` | `429` | Daily transaction frequency limit reached for account tier | Limit transaction frequency to conform to tier allowances |
| `2027` | `INELIGIBLE_FOR_AUTO_APPROVE` | `403` | Transaction attributes require manual compliance sign-off | Submit transaction for supervisor escalation in dashboard |
| `2028` | `POLICY_SUSPENDED_BY_CIRCUIT_BREAKER` | `503` | Circuit breaker tripped due to anomalous high failure rate | Wait for security review or reset circuit breaker via admin authority |
| `2029` | `INVALID_EVALUATION_INPUT` | `400` | Input payload passed to evaluate() is missing required subject fields | Provide complete EvaluationInput struct with valid account addresses |
| `2030` | `POLICY_BINDING_REVOKED` | `403` | Policy binding was revoked by asset issuer | Re-establish authorization binding with asset issuer key |
| `2031` | `UNRECOGNIZED_POLICY_EVENT` | `400` | Policy contract emitted an unhandled lifecycle event code | Update backend event indexer schema to parse new event type |
| `2032` | `MERKLE_ROOT_UNINITIALIZED` | `404` | Sanctions merkle tree root has not been initialized | Deploy initial sanctions root before enabling sanctions rule |
| `2033` | `UNAUTHORIZED_REGISTRY_UPDATE` | `401` | Caller does not hold the Registry Authority capability | Sign registry mutation with registered authority keypair |
| `2034` | `POLICY_NAME_TOO_LONG` | `400` | Policy name string exceeds 64 byte storage limit | Shorten policy name identifier |
| `2035` | `CONFLICTING_RULE_DIRECTIVE` | `409` | Two matching rules emit contradictory actions at same priority | Specify explicit rule priority order to break ties |
| `2036` | `THRESHOLD_WEIGHT_INVALID` | `400` | Rule evaluation score threshold is non-monotonic | Ensure threshold values form a strictly increasing sequence |
| `2037` | `REASON_CODE_UNKNOWN` | `400` | Evaluation emitted an unmapped reason code | Update client error catalog with newly introduced reason code |
| `2038` | `POLICY_LOCKED_PENDING_UPGRADE` | `423` | Policy contract is temporarily locked during version transition | Retry request once migration transaction confirms on-chain |
| `2039` | `REGISTRY_COMPACT_REQUIRED` | `507` | Persistent registry size exceeds memory threshold; requires compaction | Trigger compaction maintenance job to prune tombstoned keys |


### Payment & SAC Tokens (3000 - 3039)

| Code | Mnemonic | HTTP | Description | Recommended Recovery Action |
| :---: | :--- | :---: | :--- | :--- |
| `3000` | `INSUFFICIENT_BALANCE` | `400` | Sender has insufficient balance to complete payment | Ensure sender account has adequate token balance including fees |
| `3001` | `SAC_INVOCATION_FAILED` | `502` | Stellar Asset Contract transfer invocation returned error | Verify token issuer trustline and SAC contract authorization |
| `3002` | `INVALID_DECIMAL_PRECISION` | `400` | Amount exceeds token decimal precision (maximum 7 decimals for SAC) | Round amount to 7 decimal places before submitting |
| `3003` | `UNAUTHORIZED_TRANSFER_FROM` | `401` | Contract does not have allowance to debit tokens from sender | Invoke approve() on SAC token contract to grant spender allowance |
| `3004` | `SPEND_CAP_EXCEEDED` | `400` | Transaction amount exceeds instantaneous spend cap threshold | Amounts above spend cap are automatically diverted to escrow for review |
| `3005` | `SLIPPAGE_EXCEEDED` | `400` | Price slippage exceeded user tolerance during payment swap | Adjust slippage tolerance or wait for market liquidity stabilization |
| `3006` | `FEE_EXCEEDS_ALLOWANCE` | `400` | Payment routing fee exceeds configured maximum limit | Increase fee tolerance or route through lower-cost liquidity pool |
| `3007` | `ZERO_AMOUNT_PAYMENT` | `400` | Payment amount must be strictly greater than zero | Specify an amount greater than 0 stroops |
| `3008` | `NEGATIVE_AMOUNT_PAYMENT` | `400` | Negative payment amount is invalid | Amount must be a positive integer representation |
| `3009` | `TOKEN_NOT_ACCEPTABLE` | `400` | Asset is not in the list of accepted settlement currencies | Use an approved payment token (e.g. USDC, EURC, XLM) |
| `3010` | `RECIPIENT_DENYLISTED` | `403` | Payment recipient address is marked on active denylist | Change recipient or request compliance review |
| `3011` | `SENDER_DENYLISTED` | `403` | Payment sender address is marked on active denylist | Account restricted from making outgoing transfers |
| `3012` | `CIRCULAR_PAYMENT_LOOP` | `400` | Payment route forms an invalid cyclic self-transfer | Sender and recipient addresses must be distinct |
| `3013` | `MAX_PAYMENT_SIZE_EXCEEDED` | `400` | Payment exceeds maximum protocol payment size cap | Split high-value transfer into batch installments |
| `3014` | `MIN_PAYMENT_SIZE_VIOLATED` | `400` | Payment is smaller than network dust threshold | Increase payment amount above minimum threshold |
| `3015` | `SAC_CONTRACT_NOT_FOUND` | `404` | Stellar Asset Contract for given asset code not deployed on network | Deploy SAC instance for classic asset using stellar contract deploy |
| `3016` | `PAYMENT_MEMO_TOO_LONG` | `400` | Payment transaction memo exceeds maximum byte length (28 bytes text) | Shorten memo or use 32-byte hash memo |
| `3017` | `MEMO_REQUIRED_BY_RECIPIENT` | `422` | Recipient address is an exchange or custodial gateway requiring a memo | Provide required memo ID to ensure correct credit of funds |
| `3018` | `EXCHANGE_RATE_EXPIRED` | `408` | Asset conversion rate quote has expired | Request fresh exchange rate quote before executing cross-currency payment |
| `3019` | `CROSS_CURRENCY_PATH_NOT_FOUND` | `404` | No liquidity pool path found between source and destination assets | Ensure liquidity pool or orderbook offers exist for the asset pair |
| `3020` | `ROUTING_HOP_LIMIT_EXCEEDED` | `400` | Payment routing path exceeds maximum allowable hops (limit 3) | Select a more direct trading pair with higher liquidity |
| `3021` | `SETTLEMENT_CURRENCY_MISMATCH` | `400` | Settlement currency received does not match merchant requirement | Convert to exact target asset requested by payee |
| `3022` | `MERCHANT_FEE_OVER_LIMIT` | `400` | Calculated merchant processing fee exceeds ceiling | Review fee configuration schedule |
| `3023` | `TOKEN_DECIMALS_MISMATCH` | `400` | Decimals provided do not match asset contract specifications | Query token decimals metadata and adjust scaling factor |
| `3024` | `PARTIAL_PAYMENT_REJECTED` | `400` | Merchant configuration does not allow underpayments | Submit the exact invoice amount required |
| `3025` | `OVERPAYMENT_EXCEEDS_BUFFER` | `400` | Payment amount exceeds invoice amount beyond allowed buffer | Submit payment within accepted variance range |
| `3026` | `INVOICE_EXPIRED` | `410` | Payment invoice has expired and can no longer be settled | Generate a new invoice to initiate payment |
| `3027` | `INVOICE_ALREADY_PAID` | `409` | Payment invoice has already been settled | Invoice status is terminal; do not submit duplicate payments |
| `3028` | `PAYMENT_NONCE_REUSED` | `409` | Unique payment reference nonce has already been consumed | Generate unique cryptographically random payment nonce |
| `3029` | `LIQUIDITY_POOL_FROZEN` | `503` | Target liquidity pool is temporarily paused by administrator | Wait for pool to resume or select alternate routing pool |
| `3030` | `CURRENCY_CORRIDOR_DISABLED` | `403` | Direct payment corridor between currency pair is disabled | Check corridor operational status in gateway config |
| `3031` | `UNSUPPORTED_ASSET_TYPE` | `400` | Asset type is not supported (only SAC and Native XLM supported) | Convert classic credit asset to SAC contract |
| `3032` | `UNAUTHORIZED_ASSET_ISSUER` | `401` | Asset issuer is not authorized on compliance registry | Verify asset issuer identity on Stellar TOML directory |
| `3033` | `PAYMENT_EXECUTION_FAILED` | `500` | Payment execution reverted during atomic batch settlement | Check contract logs for revert cause and retry |
| `3034` | `REFUND_TIMELOCK_ACTIVE` | `423` | Payment refund cannot be processed until refund delay passes | Wait for refund window to open before requesting reversal |
| `3035` | `ALREADY_REFUNDED` | `409` | Payment transaction has already been refunded | Payment has already been returned to original sender |
| `3036` | `REFUND_AMOUNT_EXCEEDS_ORIGINAL` | `400` | Requested refund amount exceeds original settlement amount | Refund amount must be less than or equal to original amount |
| `3037` | `SPLIT_PAYMENT_MISALLOCATION` | `400` | Split recipient shares do not sum to 10,000 basis points | Verify split recipient percentage allocations total exactly 100% |
| `3038` | `MERCHANT_ACCOUNT_DEACTIVATED` | `403` | Merchant receiver account is deactivated or closed | Contact merchant to reactivate receiver account |
| `3039` | `TREASURY_ACCOUNT_DRAINED` | `503` | Fee rebate treasury has insufficient balance to sponsor gas | Replenish fee treasury balance |


### Escrow & Timelocks (4000 - 4034)

| Code | Mnemonic | HTTP | Description | Recommended Recovery Action |
| :---: | :--- | :---: | :--- | :--- |
| `4000` | `ESCROW_NOT_FOUND` | `404` | Escrow record with given ID does not exist | Verify escrow sequence ID before attempting action |
| `4001` | `ESCROW_ALREADY_SETTLED` | `409` | Escrow has already been released to recipient | Escrow is in terminal Settled state |
| `4002` | `ESCROW_ALREADY_REFUNDED` | `409` | Escrow has already been returned to depositor | Escrow is in terminal Refunded state |
| `4003` | `ESCROW_TIMELOCK_ACTIVE` | `423` | Escrow release condition timelock has not yet expired | Wait until ledger timestamp passes lockup threshold |
| `4004` | `ARBITER_UNAUTHORIZED` | `401` | Signer is not the designated arbiter for this escrow | Only designated arbiter or admin can resolve escrow dispute |
| `4005` | `DISPUTE_WINDOW_EXPIRED` | `400` | Window for lodging an escrow dispute has closed | Disputes must be raised prior to dispute deadline |
| `4006` | `DISPUTE_ALREADY_LOGGED` | `409` | A dispute has already been filed for this escrow | Await arbiter resolution on existing dispute ticket |
| `4007` | `ESCROW_DEPOSIT_MISMATCH` | `400` | Deposited amount does not match escrow terms | Deposit exact agreed escrow balance |
| `4008` | `PARTIAL_RELEASE_UNSUPPORTED` | `400` | Escrow contract terms require full atomic release | Execute full release or adjust escrow contract terms |
| `4009` | `ESCROW_RELEASE_FAILED` | `500` | Transfer from escrow vault contract to beneficiary failed | Ensure escrow contract holds sufficient balance |
| `4010` | `INSUFFICIENT_ESCROW_COLLATERAL` | `400` | Collateral deposit is below required liquidation ratio | Top up collateral deposit to satisfy margin requirements |
| `4011` | `COLLATERAL_ALREADY_CLAIMED` | `409` | Escrow collateral has already been claimed by counterparty | Collateral is no longer available in vault |
| `4012` | `TIMELOCK_EXCEEDS_MAXIMUM` | `400` | Requested timelock duration exceeds maximum policy ceiling (1 year) | Set timelock duration to less than 31,536,000 seconds |
| `4013` | `TIMELOCK_BELOW_MINIMUM` | `400` | Requested timelock duration is below minimum cooling period (1 hour) | Set timelock duration greater than 3,600 seconds |
| `4014` | `UNAUTHORIZED_DEPOSITOR` | `401` | Signer is not authorized as the depositor for this escrow | Only registered depositor can fund escrow vault |
| `4015` | `UNAUTHORIZED_BENEFICIARY` | `401` | Signer is not authorized as beneficiary to claim escrow | Only registered beneficiary can trigger payout |
| `4016` | `ESCROW_EXPIRED` | `410` | Escrow validity duration has elapsed without fulfillment | Trigger expired escrow refund back to depositor |
| `4017` | `CONDITION_PROOF_INVALID` | `422` | Cryptographic preimage or oracle proof failed verification | Provide valid hash preimage matching escrow condition |
| `4018` | `HASH_LOCK_MISMATCH` | `400` | Supplied secret does not match SHA-256 hash lock commitment | Verify preimage hash matches initial escrow commitment |
| `4019` | `ORACLE_ATTESTATION_EXPIRED` | `408` | External oracle attestation timestamp has expired | Request freshly signed oracle price or fulfillment feed |
| `4020` | `UNTRUSTED_ORACLE_SIGNER` | `401` | Attestation signature does not match trusted oracle registry | Configure oracle public key in contract registry |
| `4021` | `MULTI_PARTY_SIG_INCOMPLETE` | `400` | Escrow release requires M-of-N threshold signatures | Collect all required counterparty signatures before dispatch |
| `4022` | `ESCROW_CANCELLED_BY_ADMIN` | `403` | Escrow was cancelled by emergency administration override | Funds have been returned to original depositor vault |
| `4023` | `DUPLICATE_ESCROW_ID` | `409` | Escrow ID already exists in storage registry | Use monotonic counter or UUID for new escrow entries |
| `4024` | `ESCROW_VAULT_PAUSED` | `503` | Escrow operations paused during protocol upgrade | Wait for protocol upgrade to complete |
| `4025` | `INSPECTION_PERIOD_ACTIVE` | `423` | Funds cannot be released while buyer inspection period is active | Wait for inspection period to elapse or obtain buyer approval |
| `4026` | `MILESTONE_NOT_COMPLETED` | `400` | Escrow tranche milestone has not been verified by inspector | Complete milestone deliverables and submit proof |
| `4027` | `MILESTONE_INDEX_OUT_OF_BOUNDS` | `400` | Milestone index exceeds defined tranche count | Specify valid milestone index within defined range |
| `4028` | `TRANCHE_ALREADY_RELEASED` | `409` | Target milestone tranche has already been paid out | Select next pending milestone tranche |
| `4029` | `ARBITRATION_FEE_UNPAID` | `402` | Filing dispute requires prepayment of arbitration fee | Deposit arbitration fee to escrow contract |
| `4030` | `SETTLEMENT_SPLIT_OVERFLOW` | `400` | Arbiter split ratio between parties exceeds 100% | Ensure party shares sum to exactly 10,000 basis points |
| `4031` | `AUTOMATIC_EXPIRATION_DISABLED` | `403` | Escrow configuration does not permit permissionless expiration | Contact arbiter to resolve stuck escrow |
| `4032` | `BUYER_REJECTION_RECORDED` | `403` | Buyer has recorded formal rejection of goods | Initiate arbitration resolution or dispute mediation |
| `4033` | `ESCROW_STATE_CORRUPTED` | `500` | Escrow state machine transitioned into an undefined state | Contact support to inspect contract storage ledger |
| `4034` | `RECOVERY_TIMELOCK_PENDING` | `423` | Emergency funds recovery timelock (30 days) is still active | Wait for emergency timelock countdown to complete |


### Identity & Sanctions (5000 - 5029)

| Code | Mnemonic | HTTP | Description | Recommended Recovery Action |
| :---: | :--- | :---: | :--- | :--- |
| `5000` | `SUBJECT_UNVERIFIED` | `401` | Identity proof for subject account has not been verified | Complete KYC verification through an authorized identity provider |
| `5001` | `SPECIALLY_DESIGNATED_NATIONAL` | `403` | Account is designated on OFAC SDN sanctions list | Prohibited under federal compliance and sanctions mandates |
| `5002` | `SECONDARY_SANCTIONS_RISK` | `403` | Transaction presents secondary sanctions exposure risk | Transaction rejected under conservative compliance enforcement |
| `5003` | `KYC_PROOF_EXPIRED` | `401` | Identity credential has expired and must be reverified | Renew identity attestation to restore transaction privileges |
| `5004` | `MERKLE_PROOF_INVALID` | `422` | Cryptographic inclusion proof failed against on-chain root | Verify leaf preimage, index, and sibling hashes |
| `5005` | `PEP_DETECTED` | `403` | Politically Exposed Person detected requiring enhanced due diligence | Submit enhanced due diligence documentation to compliance team |
| `5006` | `ADVERSE_MEDIA_FLAG` | `403` | Entity flagged in global adverse media screening database | Transaction queued for compliance analyst review |
| `5007` | `HIGH_RISK_JURISDICTION` | `403` | Entity associated with FATF high-risk monitored jurisdiction | Direct transfers to/from this jurisdiction are prohibited |
| `5008` | `IDENTITY_PROVIDER_UNTRUSTED` | `401` | Identity attestation signed by uncertified KYC issuer | Provide credentials from an authorized Trust Anchor |
| `5009` | `CREDENTIAL_REVOKED` | `401` | Identity credential was revoked by the issuing authority | Contact credential issuer to resolve revocation status |
| `5010` | `DID_RESOLVER_UNAVAILABLE` | `503` | Decentralized Identifier resolver endpoint timed out | Retry transaction or verify DID document status |
| `5011` | `INVALID_CREDENTIAL_SIGNATURE` | `401` | W3C Verifiable Credential signature verification failed | Ensure credential has not been altered or tampered with |
| `5012` | `SUBJECT_ADDRESS_MISMATCH` | `400` | Attestation subject address does not match transaction sender | Present credential belonging to the active signing account |
| `5013` | `SANCTIONS_LIST_UPDATE_STALE` | `422` | On-chain sanctions list has not been updated within 24h | Trigger sanctions sync daemon before processing high-value transfers |
| `5014` | `FUZZY_NAME_MATCH_AMBIGUOUS` | `422` | Fuzzy screening match score falls in ambiguous review zone | Provide full legal name and date of birth for definitive screening |
| `5015` | `ENTITY_REGISTRATION_INVALID` | `400` | Corporate entity LEI or registration number is invalid | Provide valid 20-character Legal Entity Identifier (LEI) |
| `5016` | `TRAVEL_RULE_PAYLOAD_MISSING` | `422` | Transfer exceeds $3,000 threshold requiring Travel Rule IVMS101 data | Attach Travel Rule originator and beneficiary information |
| `5017` | `TRAVEL_RULE_VASP_UNVERIFIED` | `401` | Counterparty VASP is not registered on trusted directory | Verify counterparty VASP through TRP or OpenVASP protocol |
| `5018` | `BENEFICIARY_INFO_INCOMPLETE` | `400` | Beneficiary physical address or date of birth missing in payload | Include full beneficiary details in compliance payload |
| `5019` | `IDENTITY_COMMITMENT_EXISTS` | `409` | A credential has already been committed for this account | Revoke previous credential before committing an update |
| `5020` | `ZERO_KNOWLEDGE_PROOF_REJECTED` | `422` | zk-SNARK compliance proof verification failed on-chain | Regenerate zero-knowledge proof with valid witness inputs |
| `5021` | `ANONYMOUS_PROXY_DETECTED` | `403` | Transaction originated from known VPN/TOR exit node or mixing pool | Transactions through anonymizing infrastructure are blocked |
| `5022` | `AGE_VERIFICATION_FAILED` | `403` | Signer does not satisfy minimum age requirement (18+) | Account holder must meet legal age requirements |
| `5023` | `RESIDENCY_ATTESTATION_MISSING` | `401` | Proof of residency required for this regulatory jurisdiction | Upload utility bill or bank statement proof of residency |
| `5024` | `WATCHLIST_NAME_SIMILARITY` | `202` | Name similarity score exceeds alert threshold; flagged for review | Case opened for compliance analyst manual adjudication |
| `5025` | `IP_GEOLOCATION_MISMATCH` | `403` | Sender IP geolocation conflicts with registered KYC jurisdiction | Disable VPN or provide proof of temporary travel |
| `5026` | `BIOMETRIC_LIVENESS_FAILED` | `401` | Biometric face match liveness verification expired or failed | Complete real-time liveness check in mobile application |
| `5027` | `DUAL_NATIONALITY_SANCTIONED` | `403` | Individual holds secondary nationality in sanctioned territory | Transaction prohibited under extraterritorial sanctions laws |
| `5028` | `MILITARY_END_USER_RESTRICTION` | `403` | Recipient entity flagged under Military End-User regulations | Export control restrictions prohibit transfer |
| `5029` | `COMPLIANCE_HOLD_PENDING` | `423` | Compliance hold placed on account pending regulatory inquiry | Await resolution of pending inquiry by compliance officer |


### Auth & Governance (6000 - 6029)

| Code | Mnemonic | HTTP | Description | Recommended Recovery Action |
| :---: | :--- | :---: | :--- | :--- |
| `6000` | `UNAUTHORIZED_CALLER` | `401` | Signer does not hold authorization to execute this function | Sign transaction with contract administrator keypair |
| `6001` | `ADMIN_KEY_REVOKED` | `401` | Admin key has been revoked and replaced by multi-sig governance | Submit proposal through governance contract multi-sig |
| `6002` | `SIGNATURE_THRESHOLD_UNMET` | `400` | Collected signatures do not meet multi-sig quorum threshold | Collect additional co-signer approvals before execution |
| `6003` | `EMERGENCY_PAUSE_ACTIVE` | `503` | Contract is currently in Emergency Pause mode | Wait for emergency inspection to finish and unpause signal |
| `6004` | `UPGRADE_UNAUTHORIZED` | `401` | Caller does not possess Contract Upgrade capability | Contract upgrades require 3-of-5 governance consensus |
| `6005` | `TIMELOCK_DELAY_NOT_MET` | `423` | Governance proposal time-delay has not elapsed | Allow 48-hour timelock delay to pass before execution |
| `6006` | `PROPOSAL_ALREADY_EXECUTED` | `409` | Governance proposal has already been executed | Proposal status is terminal; cannot execute twice |
| `6007` | `PROPOSAL_CANCELLED` | `410` | Governance proposal was cancelled by emergency multisig | Proposal has been permanently cancelled |
| `6008` | `PROPOSAL_VOTING_CLOSED` | `400` | Voting window for governance proposal has closed | Submit new proposal if voting deadline elapsed |
| `6009` | `VOTE_ALREADY_CAST` | `409` | Signer has already cast a vote on this proposal | Votes cannot be cast multiple times per address |
| `6010` | `INSUFFICIENT_VOTING_POWER` | `403` | Signer does not hold required governance token balance | Acquire governance voting tokens or delegate voting power |
| `6011` | `ROLE_ALREADY_ASSIGNED` | `409` | Target address already possesses the assigned role | Address already holds target role privileges |
| `6012` | `ROLE_NOT_FOUND` | `404` | Target address does not hold the role to be revoked | Verify role assignment before attempting revocation |
| `6013` | `SELF_REVOCATION_PREVENTED` | `400` | Administrator cannot revoke their own final admin role | Designate a successor admin before revoking current credentials |
| `6014` | `MULTISIG_WEIGHT_ZERO` | `400` | Signer weight cannot be zero in governance quorum | Assign positive integer weight to active co-signers |
| `6015` | `QUORUM_CEILING_EXCEEDED` | `400` | Total quorum weight exceeds maximum allowable scale | Normalize signer weights within 1 to 100 range |
| `6016` | `REPLAY_NONCE_DETECTED` | `409` | Cryptographic invocation nonce has already been used | Generate fresh random nonce for meta-transaction signature |
| `6017` | `EXPIRATION_TIME_IN_PAST` | `400` | Authorization delegation signature expiration is in the past | Set expiration timestamp to a future ledger timestamp |
| `6018` | `DELEGATION_SCOPE_EXCEEDED` | `403` | Delegated key attempted operation outside allowed method scope | Only invoke methods explicitly authorized in delegation proof |
| `6019` | `GAS_SPONSOR_REJECTED` | `401` | Gas fee relayer rejected sponsorship for this transaction | Deposit fee credit or self-sponsor transaction fees |
| `6020` | `CIRCUIT_BREAKER_TRIPPED` | `503` | Automated circuit breaker tripped due to anomalous contract event | Admin must investigate and reset circuit breaker state |
| `6021` | `RATE_LIMIT_HIT` | `429` | Administrative invocation frequency exceeded per-minute limit | Throttle administrative requests to avoid rate limits |
| `6022` | `INVALID_GOVERNANCE_CONFIG` | `400` | Governance parameters violate invariant constraints | Ensure quorum threshold is <= total signer weights |
| `6023` | `GUARDIAN_COOLDOWN_ACTIVE` | `423` | Guardian key rotation is subject to a 7-day cooldown | Wait for security cooldown period to expire |
| `6024` | `BACKUP_KEY_NOT_CONFIGURED` | `404` | Emergency recovery triggered but no backup key was registered | Register backup recovery key during contract initialization |
| `6025` | `UPGRADE_HASH_MISMATCH` | `400` | WASM bytecode hash does not match hash approved in proposal | Deploy exact WASM binary that received governance approval |
| `6026` | `INITIALIZER_ALREADY_RUN` | `409` | Contract initialize() method can only be executed once | Contract is already initialized and operational |
| `6027` | `WRONG_CALL_CONTEXT` | `403` | Method can only be invoked from another contract via internal call | Do not invoke internal helper functions directly from user envelope |
| `6028` | `CROSS_ORG_CALL_UNAUTHORIZED` | `401` | Cross-organization federation key not accepted | Register federation trust relationship before calling |
| `6029` | `DEPUTY_PERMISSIONS_REVOKED` | `403` | Deputy operator permissions have been suspended | Request primary admin to re-enable operator capabilities |


### SDK & API Client (7000 - 7029)

| Code | Mnemonic | HTTP | Description | Recommended Recovery Action |
| :---: | :--- | :---: | :--- | :--- |
| `7000` | `NETWORK_TIMEOUT` | `504` | Soroban RPC network request timed out | Check network connection or switch to secondary RPC endpoint |
| `7001` | `RPC_NODE_UNREACHABLE` | `503` | Configured Soroban RPC endpoint is offline or unreachable | Verify RPC URL status at https://soroban-testnet.stellar.org |
| `7002` | `SIMULATION_MISMATCH` | `422` | On-chain execution diverged from local transaction simulation | Re-run simulateTransaction to refresh footprints and authorizations |
| `7003` | `XDR_DECODE_ERROR` | `400` | Failed to decode base64 XDR structure returned from RPC | Ensure client uses stellar-sdk matching current Protocol XDR |
| `7004` | `RATE_LIMIT_EXCEEDED` | `429` | RPC requests exceeded per-second quota | Implement exponential backoff or use dedicated API key |
| `7005` | `INVALID_ADDRESS_CHECKSUM` | `400` | Stellar StrKey address failed CRC16 checksum validation | Verify address format (56 characters starting with G or C) |
| `7006` | `MISSING_TRANSACTION_SIGNATURE` | `400` | Transaction envelope submitted without required signatures | Sign envelope with Freighter, Albedo, or keypair before send |
| `7007` | `HORIZON_FALLBACK_FAILED` | `502` | Horizon API fallback endpoint returned 500 error | Verify Horizon cluster health status |
| `7008` | `INVALID_BASE64_PAYLOAD` | `400` | Supplied string is not valid standard Base64 encoding | Sanitize base64 string and ensure correct padding |
| `7009` | `UNSUPPORTED_NETWORK_PASSPHRASE` | `400` | Network passphrase not recognized (expected testnet or public) | Set passphrase to 'Test SDF Network ; September 2015' for testnet |
| `7010` | `CONTRACT_ABI_NOT_FOUND` | `404` | Contract specification stream missing from WASM binary | Ensure contract was compiled with Soroban SDK and contains contractspec |
| `7011` | `JSON_SERIALIZATION_FAILED` | `500` | Failed to serialize JavaScript object to JSON string | Remove circular references before serialization |
| `7012` | `FREIGHTER_NOT_INSTALLED` | `404` | Freighter wallet browser extension was not detected | Install Freighter wallet from https://www.freighter.app |
| `7013` | `USER_REJECTED_SIGNATURE` | `403` | User declined signature prompt in wallet popup | Prompt user to approve signature when ready |
| `7014` | `UNSUPPORTED_WALLET_PROVIDER` | `400` | Selected wallet provider is not supported in this browser | Choose from supported wallets: Freighter, Albedo, or xBull |
| `7015` | `SOCKET_CONNECTION_CLOSED` | `503` | WebSocket connection for real-time ledger streaming closed unexpectedly | SDK will automatically attempt reconnect with exponential backoff |
| `7016` | `PARSING_INT128_FAILED` | `400` | Failed to parse string into 128-bit big integer | Ensure string contains only valid decimal digits |
| `7017` | `CONFIG_NOT_LOADED` | `500` | SafeguardClient initialized without required configuration | Supply valid ClientConfig object with rpcUrl and contract IDs |
| `7018` | `LEDGER_HISTORY_UNAVAILABLE` | `404` | Target ledger sequence is beyond RPC node history retention | Query archival Horizon node for historical ledger data |
| `7019` | `EVENT_SUBSCRIPTION_FAILED` | `502` | Failed to subscribe to contract topic events via RPC filter | Verify contract address and event topics in filter payload |
| `7020` | `BATCH_PAYMENT_SIZE_LIMIT` | `400` | Batch payment array exceeds client maximum size of 50 items | Partition payments into batches of 50 or fewer |
| `7021` | `RESPONSE_SCHEMA_VALIDATION_ERROR` | `500` | Server response payload did not conform to Zod schema | Update client SDK to latest matching API release |
| `7022` | `METHOD_NOT_FOUND_ON_CONTRACT` | `404` | Target method is not an exported function on contract | Inspect contract specification exports for valid method names |
| `7023` | `IDEMPOTENCY_KEY_MISSING` | `400` | POST request requires unique Idempotency-Key header | Generate UUID v4 for Idempotency-Key header |
| `7024` | `IDEMPOTENCY_CONFLICT` | `409` | Request with same Idempotency-Key is currently being processed | Wait for previous operation to complete before reusing key |
| `7025` | `SDK_VERSION_DEPRECATED` | `426` | Client SDK version is below required minimum protocol version | Run npm update @safeguard/sdk to install latest release |
| `7026` | `INVALID_HEX_STRING` | `400` | String is not a valid even-length hexadecimal string | Verify hex characters match regex ^[0-9a-fA-F]+$ |
| `7027` | `INSUFFICIENT_FEE_ESTIMATE` | `400` | Simulated fee is higher than client max fee setting | Increase maxFee parameter in transaction options |
| `7028` | `TRANSACTION_ABORTED` | `499` | Transaction execution was cancelled by caller before submission | Operation cleanly aborted by caller |
| `7029` | `UNKNOWN_CLIENT_ERROR` | `500` | Unclassified client SDK runtime exception | Inspect stack trace and check issue tracker |


### Audit & Integrity (8000 - 8019)

| Code | Mnemonic | HTTP | Description | Recommended Recovery Action |
| :---: | :--- | :---: | :--- | :--- |
| `8000` | `AUDIT_LOG_TAMPERED` | `500` | Cryptographic hash chain verification failed on audit records | Audit record sequence exhibits integrity violation; investigate immediately |
| `8001` | `HASH_CHAIN_BROKEN` | `500` | Previous block digest pointer does not match parent record hash | Data corruption or unrecorded mutation in append-only log |
| `8002` | `SEQUENCE_GAP_DETECTED` | `500` | Audit sequence counter skipped an integer index | Verify missing sequence number in transaction archive |
| `8003` | `PROOF_VERIFICATION_FAILED` | `422` | Merkle inclusion proof verification failed against batch root | Re-compute leaf digest and check sibling nodes |
| `8004` | `STORAGE_PROOF_INVALID` | `422` | State commitment proof does not match Stellar ledger state | Query fresh state proof from verified archive node |
| `8005` | `AUDIT_EVIDENCE_EXPIRED` | `410` | Evidence retention window has lapsed for audit package | Access cold archive storage for records older than 7 years |
| `8006` | `EVIDENCE_CHECKSUM_MISMATCH` | `400` | Evidence package archive checksum does not match manifest | Re-download evidence package and verify SHA-256 |
| `8007` | `UNAUTHORIZED_AUDITOR` | `401` | Signer does not hold the certified Independent Auditor credential | Only accredited auditors can submit official audit reports |
| `8008` | `REPORT_ALREADY_FINALIZED` | `409` | Audit report has been signed and sealed into immutable state | Finalized reports cannot be edited; submit addendum report |
| `8009` | `INTEGRITY_ATTESTATION_FAILED` | `500` | Hardware enclave or TEE attestation failed signature check | Inspect TEE measurement quote and certificate chain |
| `8010` | `EVENT_DIGEST_MISMATCH` | `500` | Emitted event payload hash does not match logged audit entry | Verify event listener captured complete canonical payload |
| `8011` | `MISSING_PREDECESSOR_BLOCK` | `404` | Audit chain checkpoint references missing historical segment | Synchronize historical checkpoint blocks from archive |
| `8012` | `TIMESTAMP_NON_MONOTONIC` | `400` | Audit record timestamp is earlier than preceding record | Timestamps must strictly increase within the append-only log |
| `8013` | `REPRODUCIBLE_REPORT_FAILURE` | `500` | Deterministic rerun of policy verification produced differing result | Check for non-deterministic rule predicates or state divergence |
| `8014` | `REGULATOR_PACKAGE_SEAL_BROKEN` | `500` | Digital signature seal on regulatory compliance export is invalid | Re-export compliance package under authorized compliance key |
| `8015` | `AUDIT_VAULT_CAPACITY_REACHED` | `507` | On-chain audit accumulator reached maximum capacity | Roll accumulator root to archival storage checkpoint |
| `8016` | `RECORD_LOCKED_FOR_DISCOVERY` | `423` | Record cannot be purged while legal discovery freeze is active | Comply with active litigation hold mandate |
| `8017` | `UNSUPPORTED_DIGEST_ALGORITHM` | `400` | Hash algorithm is not supported (expected SHA-256 or BLAKE3) | Use SHA-256 for all cryptographic audit commitments |
| `8018` | `DISPUTE_PROOF_INSUFFICIENT` | `422` | Submitted dispute proof does not contain sufficient state witnesses | Include full transaction trace and event log witnesses |
| `8019` | `AUDIT_INDEX_OUT_OF_SYNC` | `503` | Secondary query index lags behind authoritative blockchain ledger | Wait for indexer synchronization to reach current ledger |


### Config & Deployment (9000 - 9014)

| Code | Mnemonic | HTTP | Description | Recommended Recovery Action |
| :---: | :--- | :---: | :--- | :--- |
| `9000` | `CONFIG_SCHEMA_MISMATCH` | `400` | Configuration object does not validate against target JSON schema | Review required fields and types in config specification |
| `9001` | `MISSING_ENVIRONMENT_VARIABLE` | `500` | Required environment variable is undefined in execution context | Provide all required environment variables in .env file |
| `9002` | `CONTRACT_NOT_INITIALIZED` | `503` | Contract deployed but initialize() has not been executed | Run deployment initialization script before handling traffic |
| `9003` | `INCOMPATIBLE_PROTOCOL_VERSION` | `426` | Ledger protocol version is below required minimum (Protocol 22) | Stellar network must be running at least Protocol 22 for Soroban |
| `9004` | `INVALID_NETWORK_NAME` | `400` | Network name must be one of: testnet, futurenet, mainnet, standalone | Set STELLAR_NETWORK to a valid supported network name |
| `9005` | `RPC_PORT_CONFLICT` | `500` | Local sandbox RPC port is already bound by another process | Release port 8000 or configure custom RPC port |
| `9006` | `WASM_HASH_NOT_REGISTERED` | `404` | WASM binary hash not found in on-chain code repository | Install WASM bytecode to network before contract instantiation |
| `9007` | `DEPLOYMENT_ALREADY_EXISTS` | `409` | Contract instance already deployed at computed address | Use existing deployment or provide distinct salt |
| `9008` | `INVALID_DEPLOYMENT_SALT` | `400` | Salt must be a 32-byte hex string or alphanumeric identifier | Supply valid 32-byte salt for deterministic deployment |
| `9009` | `UNRECOGNIZED_CHAIN_ID` | `400` | Chain ID does not correspond to a known Stellar cluster | Specify valid cluster network passphrase |
| `9010` | `SECRETS_VAULT_UNREACHABLE` | `503` | Unable to retrieve deployment private keys from secrets manager | Verify AWS Secrets Manager / Vault credentials |
| `9011` | `DATABASE_MIGRATION_PENDING` | `503` | Database schema has pending migrations; refusing traffic | Execute npm run db:migrate before starting server daemon |
| `9012` | `STORAGE_BACKEND_UNAVAILABLE` | `503` | Persistent storage engine (Postgres / Redis) connection failed | Check database connection string and ensure service is running |
| `9013` | `CORRUPT_ENV_FILE` | `500` | Syntax error encountered while parsing .env configuration file | Fix invalid quotes or unescaped characters in .env file |
| `9014` | `DEPLOYMENT_METADATA_MISSING` | `404` | deployments/testnet.json file not found in repository root | Run scripts/deploy-testnet.sh to generate deployment metadata |

