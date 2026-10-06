# Gas & Fee Benchmarks — Measured on Stellar Testnet

Every number below is the **`fee_charged`** that Horizon reports for a real
transaction against the live deployment in
[`deployments/testnet.json`](../deployments/testnet.json). Nothing here is
estimated or simulated — click any hash to inspect it on StellarExpert.

- **Payments contract:** [`CDC6KVX7…SRCN`](https://stellar.expert/explorer/testnet/contract/CDC6KVX7QT7CD3GOVGX44NQUNS7FMSZKIAXTV3TDGSXQKJRQMDZRSRCN)
- **Token:** native XLM SAC (`CDLZFC3S…CYSC`)
- **Measured:** 2026-10-05 with `stellar-cli 28.1.0`
- **Unit:** stroops (1 XLM = 10,000,000 stroops)

| Call | Scenario | Fee charged (stroops) | Fee (XLM) | Transaction |
| :--- | :--- | ---: | ---: | :--- |
| `pay` | Approved — 50 XLM, under the cap, settles directly | **19,237** | 0.0019237 | [`c762b42f…`](https://stellar.expert/explorer/testnet/tx/c762b42f818387aa584ea33d3da006f22671071ed6e182068994ea6597395e6c) |
| `pay` | Escrowed — 150 XLM, over the 100 XLM cap, reason code `6` | **739,309** | 0.0739309 | [`2d832316…`](https://stellar.expert/explorer/testnet/tx/2d83231685f03b17e1a001e6c82c38453459b4f67b416ef60f9be73133026f0e) |
| `release_escrow` | Admin releases escrow #1 to the recipient | **16,575** | 0.0016575 | [`f1257dd8…`](https://stellar.expert/explorer/testnet/tx/f1257dd8e902c4dad2c00b8f71cc98999d9885e402fbd7959c4242202cef2331) |
| `add_to_denylist` | First denylist entry (creates a persistent entry) | **79,083** | 0.0079083 | [`2dcc5827…`](https://stellar.expert/explorer/testnet/tx/2dcc5827355c125605aaf7fd180b547f0235ee9a642d5475c50cedc6dcffd2ba) |
| `set_spend_cap` | Instance-storage update | **6,968** | 0.0006968 | [`e4f5a316…`](https://stellar.expert/explorer/testnet/tx/e4f5a316105cf59ab467c55c4b6744fb51a84d2ace4f2feffd99a410abe96fc8) |
| `pay` | Blocked — recipient denylisted | **0** | 0 | Rejected at simulation with `RecipientDenylisted` (#11); never submitted |

## Reading the numbers

- **The common path is cheap.** An approved payment costs about 0.002 XLM, which
  includes the token transfer.
- **Escrow is the expensive path, and that is rent, not compute.** Diverting to
  escrow writes a new persistent `EscrowRecord`, so most of the 739k stroops is
  the one-off rent deposit for that ledger entry. Releasing it afterwards costs
  only 16.5k.
- **Blocked payments cost nothing.** Denylist checks run before any token call,
  so a blocked payment fails in simulation and the sender pays no fee.

## Binary size

| WASM | Bytes | SHA-256 |
| :--- | ---: | :--- |
| `safeguard_payments.wasm` | 25,881 | `66c0e184…0458156` |
| `safeguard_policy.wasm` | 56,262 | `4721e383…38c8385` |

Built with `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`
and stripped symbols (see the workspace [`Cargo.toml`](../Cargo.toml)).

## Reproducing

```bash
./scripts/build.sh
STELLAR_IDENTITY=my-testnet-key ./scripts/deploy-testnet.sh
# then invoke pay / release_escrow and read fee_charged from Horizon:
curl https://horizon-testnet.stellar.org/transactions/<hash> | jq .fee_charged
```

Fees move with network load and the resource fee schedule, so expect your
numbers to differ slightly from these.

## Known optimisation opportunities

These are open for contributors:

1. **Escrow rent:** store a smaller `EscrowRecord`, for example by dropping the
   duplicated `token` and `sender` fields in favour of a packed key, to cut the
   rent deposit.
2. **Denylist storage:** a single shared map would avoid one persistent entry
   per address for small lists.
