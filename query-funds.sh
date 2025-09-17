printf "Querying gaia balance...\n"
gaiad --node tcp://localhost:27030 query bank balances $(gaiad --home ~/.gm/gaia keys --keyring-backend="test" show wallet -a)
printf "\n"