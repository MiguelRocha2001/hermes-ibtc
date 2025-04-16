# Check documentation here: https://docs.tendermint.com/master/rpc/#/ABCI/abci_query

cp ibtc-gaia-config.toml $HOME/.hermes/config.toml

printf "Creating clients... for chain ibc-0\n"
# Creates LC on chain-1, referencing chain-0
cargo run --no-default-features \
    create client \
    --host-chain ibtc \
    --reference-chain gaia
printf "\n"

printf "Creating clients... for chain ibc-1\n"
# Creates LC on chain-0, referencing chain-1
cargo run --no-default-features \
    create client \
    --host-chain gaia \
    --reference-chain ibtc
printf "\n"

# For this to work, start with block N in validator and create clients;
# Then, increase block number on validator;
# Call create-connection
printf "Creating connection...\n"
"""
cargo run --no-default-features \
    create connection \
    --a-chain gaia \
    --a-client 09-ibtc-0 \
    --b-client 07-tendermint-0
printf "\n"
"""
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

printf "Querying channels...\n"
cargo run --no-default-features \
    query channels --show-counterparty --chain ibtc
printf "\n"

# Query balances
printf "Querying balance...\n"
gaiad --node tcp://localhost:27030 query bank balances $(gaiad --home ~/.gm/gaia keys --keyring-backend="test" show wallet -a)
#gaiad --node tcp://localhost:27040 query bank balances $(gaiad --home ~/.gm/ibc-1 keys --keyring-backend="test" show wallet -a)
printf "\n"

printf "Relaying...\n"
cargo run --no-default-features \
    start
printf "\n"

printf "Transfering...\n"
cargo run --no-default-features \
  tx ft-transfer \
  --timeout-seconds 1000 \
  --dst-chain ibtc \
  --src-chain gaia \
  --src-port transfer \
  --src-channel channel-0 \
  --amount 100000
printf "\n"


# Deploy listener to catch Hermes message
#nc -l 27040