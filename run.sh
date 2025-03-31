# Check documentation here: https://docs.tendermint.com/master/rpc/#/ABCI/abci_query

cp ibtc-osmosis-config.toml $HOME/.hermes/config.toml

printf "Creating clients... for chain ibc-0\n"
# Creates LC on chain-1, referencing chain-0
cargo run --no-default-features \
    create client \
    --host-chain ibc-1 \
    --reference-chain ibc-0
printf "\n"

printf "Creating clients... for chain ibc-1\n"
# Creates LC on chain-0, referencing chain-1
cargo run --no-default-features \
    create client \
    --host-chain ibc-0 \
    --reference-chain ibc-1
printf "\n"

# For this to work, start with block N in validator and create clients;
# Then, increase block number on validator;
# Call create-connection
printf "Creating connection...\n"
cargo run --no-default-features \
    create connection \
    --a-chain ibc-0 \
    --a-client 07-tendermint-0 \
    --b-client 07-tendermint-0
printf "\n"

printf "Creating channel...\n"
cargo run --no-default-features \
    create channel \
    --a-chain ibc-0 \
    --a-connection connection-0 \
    --a-port transfer \
    --b-port transfer
printf "\n"

# Query balances
gaiad --node tcp://localhost:27030 query bank balances $(gaiad --home ~/.gm/ibc-0 keys --keyring-backend="test" show wallet -a)
gaiad --node tcp://localhost:27040 query bank balances $(gaiad --home ~/.gm/ibc-1 keys --keyring-backend="test" show wallet -a)

printf "Relaying...\n"
cargo run --no-default-features \
    start \
    --config my-config
printf "\n"

# Deploy listener to catch Hermes message
#nc -l 27040