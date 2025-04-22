printf "Transfering tokens from gaia to Ibtc...\n"
cargo run --no-default-features \
  tx ft-transfer \
  --timeout-seconds 1000 \
  --dst-chain ibtc \
  --src-chain gaia \
  --src-port transfer \
  --src-channel channel-0 \
  --amount 100000
printf "\n"

printf "Querying gaia balance...\n"
gaiad --node tcp://localhost:27030 query bank balances $(gaiad --home ~/.gm/gaia keys --keyring-backend="test" show wallet -a)
printf "\n"