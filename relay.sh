cp ibtc-gaia-config.toml $HOME/.hermes/config.toml

printf "Relaying...\n"
cargo run --no-default-features \
    start
printf "\n"