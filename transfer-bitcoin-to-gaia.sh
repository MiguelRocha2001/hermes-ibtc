cargo run --no-default-features \
  tx ft-transfer \
  --timeout-seconds 1000 \
  --denom bitcoin \
  --dst-chain gaia \
  --src-chain ibtc \
  --src-port transfer \
  --src-channel channel-0 \
  --amount 5000

printf "Querying gaia balance...\n"
gaiad --node tcp://localhost:27030 query bank balances $(gaiad --home ~/.gm/gaia keys --keyring-backend="test" show wallet -a)
printf "\n"

cargo run --no-default-features \
  tx ft-transfer \
  --timeout-seconds 10000 \
  --denom ibc/38E6DC2812FA05F7659518243E5E2E57858239DD38884F8809873437B4EA1B7C \
  --dst-chain ibtc \
  --src-chain gaia \
  --src-port transfer \
  --src-channel channel-0 \
  --amount 990