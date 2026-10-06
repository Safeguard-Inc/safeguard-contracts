#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# Safeguard Inc. - Soroban Testnet Contract Deployment & Initialization Script
# ==============================================================================

NETWORK="testnet"
RPC_URL="https://soroban-testnet.stellar.org"
NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
IDENTITY="${STELLAR_IDENTITY:-safeguard-admin}"

echo "=========================================================="
echo " Deploying Safeguard Stack to Stellar Testnet             "
echo " Network: $NETWORK                                        "
echo " Identity: $IDENTITY                                      "
echo "=========================================================="

# 1. Ensure stellar CLI is available
if ! command -v stellar &> /dev/null; then
    echo "Error: stellar CLI is not installed. Install via: cargo install --locked stellar-cli"
    exit 1
fi

# 2. Build release contracts
echo "==> [1/4] Building release WASMs..."
cargo build --target wasm32v1-none --release -p safeguard-payments -p safeguard-policy

PAYMENTS_WASM="target/wasm32v1-none/release/safeguard_payments.wasm"
POLICY_WASM="target/wasm32v1-none/release/safeguard_policy.wasm"

# 3. Deploy Safeguard Policy Contract
echo "==> [2/4] Deploying safeguard-policy contract..."
POLICY_CONTRACT_ID=$(stellar contract deploy \
  --wasm "$POLICY_WASM" \
  --source "$IDENTITY" \
  --network "$NETWORK")

echo "--> Policy Contract Deployed ID: $POLICY_CONTRACT_ID"

# 4. Deploy Safeguard Payments Contract
echo "==> [3/4] Deploying safeguard-payments contract..."
PAYMENTS_CONTRACT_ID=$(stellar contract deploy \
  --wasm "$PAYMENTS_WASM" \
  --source "$IDENTITY" \
  --network "$NETWORK")

echo "--> Payments Contract Deployed ID: $PAYMENTS_CONTRACT_ID"

# 5. Initialize both contracts
ADMIN_ADDR=$(stellar keys address "$IDENTITY")
SPEND_CAP="1000000000"      # 100 tokens at 7 decimals
ESCROW_PERIOD="86400"       # 24h refund timelock (seconds)

echo "==> [4/4] Initializing contracts with Admin ($ADMIN_ADDR)..."
stellar contract invoke \
  --id "$POLICY_CONTRACT_ID" \
  --source "$IDENTITY" \
  --network "$NETWORK" \
  -- initialize \
  --admin "$ADMIN_ADDR"

stellar contract invoke \
  --id "$PAYMENTS_CONTRACT_ID" \
  --source "$IDENTITY" \
  --network "$NETWORK" \
  -- initialize \
  --admin "$ADMIN_ADDR" \
  --escrow_period "$ESCROW_PERIOD" \
  --spend_cap "$SPEND_CAP"

# Save deployment artifact
cat <<EOF > deployments/testnet.json
{
  "network": "testnet",
  "rpcUrl": "$RPC_URL",
  "networkPassphrase": "$NETWORK_PASSPHRASE",
  "contracts": {
    "safeguardPolicy": "$POLICY_CONTRACT_ID",
    "safeguardPayments": "$PAYMENTS_CONTRACT_ID"
  },
  "admin": "$ADMIN_ADDR",
  "spendCap": "$SPEND_CAP",
  "deployedAt": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
}
EOF

echo "==> Deployment complete! Details written to deployments/testnet.json"
