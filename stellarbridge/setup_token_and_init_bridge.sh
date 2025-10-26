#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "Usage: $0 <CONTRACT_ID_C...> <VERIFIER_ADDRESS_G...>"
  exit 1
fi

CONTRACT_ID="$1"
VERIFIER_ADDRESS="$2"

echo "==> CONTRACT_ID:      $CONTRACT_ID"
echo "==> VERIFIER_ADDRESS: $VERIFIER_ADDRESS"

# --- sanity checks ---
command -v soroban >/dev/null || { echo "soroban CLI not found"; exit 1; }
command -v git >/dev/null || { echo "git not found"; exit 1; }
command -v rustup >/dev/null || { echo "rustup not found"; exit 1; }

echo "==> Ensuring rust target wasm32v1-none (and fallback)..."
rustup target add wasm32v1-none || true
rustup target add wasm32-unknown-unknown || true

WORKDIR="$(pwd)"
TMPDIR="$(mktemp -d)"
echo "==> Using temp dir: $TMPDIR"
cd "$TMPDIR"

echo "==> Cloning soroban-examples (token)..."
git clone --depth 1 https://github.com/stellar/soroban-examples.git
cd soroban-examples/token

echo "==> Building standard token (try wasm32v1-none first)..."
if soroban contract build; then
  :
else
  echo "Build failed once, retrying after clean…"
  cargo clean || true
  soroban contract build
fi

echo "==> Looking for token wasm..."
TOKEN_WASM="$(find target -name 'soroban_token_contract.wasm' | head -n 1)"
if [[ -z "$TOKEN_WASM" ]]; then
  TOKEN_WASM="$(find target -name '*.wasm' | head -n 1)"
fi
[[ -n "$TOKEN_WASM" ]] || { echo "Token wasm not found"; exit 1; }
echo "==> TOKEN_WASM: $TOKEN_WASM"

echo "==> Deploying token to Testnet (constructor: admin,decimals,name,symbol)..."
DEPLOYER_ADDR="$(soroban keys address deployer)"
USDC_TOKEN="$(
  soroban contract deploy \
    --wasm "$TOKEN_WASM" \
    --source deployer \
    --network testnet \
    -- \
    --arg address:$DEPLOYER_ADDR \
    --arg u32:2 \
    --arg symbol:"dUSD" \
    --arg symbol:"dUSD"
)"
echo "==> USDC_TOKEN: $USDC_TOKEN"

echo "==> Ensure accounts funded on friendbot (investor, verifier, deployer)"
soroban keys generate investor >/dev/null 2>&1 || true
INVESTOR_ADDR="$(soroban keys address investor)"
curl -s "https://friendbot.stellar.org/?addr=$INVESTOR_ADDR" >/dev/null || true
curl -s "https://friendbot.stellar.org/?addr=$VERIFIER_ADDRESS" >/dev/null || true
curl -s "https://friendbot.stellar.org/?addr=$DEPLOYER_ADDR" >/dev/null || true

echo "==> Minting 1,000 dUSD to investor ($INVESTOR_ADDR) (i128=100000 con 2 decimales)"
soroban contract invoke \
  --id "$USDC_TOKEN" \
  --source deployer \
  --network testnet \
  --fn mint \
  --arg address:$INVESTOR_ADDR \
  --arg i128:100000

echo "==> Initializing your StellarBridge contract (must be signed by verifier)…"
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source verifier \
  --network testnet \
  --fn initialize \
  --arg address:$VERIFIER_ADDRESS \
  --arg address:$USDC_TOKEN

echo
echo "========================================="
echo "✅ DONE!"
echo "USDC_TOKEN = $USDC_TOKEN"
echo "INVESTOR   = $INVESTOR_ADDR"
echo "DEPLOYER   = $DEPLOYER_ADDR"
echo "VERIFIER   = $VERIFIER_ADDRESS"
echo "========================================="
echo
echo "Next steps (examples):"
echo "  # Create project (owner signs)"
echo "  soroban keys generate owner >/dev/null 2>&1 || true"
echo "  OWNER=\$(soroban keys address owner); curl -s \"https://friendbot.stellar.org/?addr=\$OWNER\" >/dev/null || true"
echo "  AMOUNTS='[25,15,10]'; DEADLINES='[1730851200,1731456000,1732060800]'"
echo "  soroban contract invoke --id $CONTRACT_ID --source owner --network testnet --fn create_project \\"
echo "    --arg address:\$OWNER --arg i128:50 --arg vec:i128:\$AMOUNTS --arg vec:u64:\$DEADLINES"
echo
echo "  # Invest 5 dUSD (investor signs)"
echo "  soroban contract invoke --id $CONTRACT_ID --source investor --network testnet --fn invest \\"
echo "    --arg u32:1 --arg address:$INVESTOR_ADDR --arg i128:5"
echo
echo "  # Submit evidence (owner signs)"
echo "  soroban contract invoke --id $CONTRACT_ID --source owner --network testnet --fn submit_evidence \\"
echo "    --arg u32:1 --arg u32:0 --arg bytes32:0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
echo
echo "  # Verify milestone (verifier signs)"
echo "  soroban contract invoke --id $CONTRACT_ID --source verifier --network testnet --fn verify_milestone \\"
echo "    --arg u32:1 --arg u32:0 --arg bool:true"
echo
echo "Tip: tail events"
echo "  soroban events tail --network testnet --id $CONTRACT_ID"
