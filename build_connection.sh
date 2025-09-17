# Check documentation here: https://docs.tendermint.com/master/rpc/#/ABCI/abci_query

cp ibtc-gaia-config.toml $HOME/.hermes/config.toml

cargo run --no-default-features \
  create connection \
  --a-chain gaia \
  --b-chain ibtc

printf "Creating channel...\n"
cargo run --no-default-features \
    create channel \
    --a-chain gaia \
    --a-connection connection-0 \
    --a-port transfer \
    --b-port transfer
printf "\n"

printf "Relaying...\n"
cargo run --no-default-features \
    start
printf "\n"