printf "Transfering tokens from Ibtc to Gaia...\n"

cargo run --no-default-features \
  tx ft-transfer \
  --timeout-seconds 10000 \
  --denom ibc/C1840BD16FCFA8F421DAA0DAAB08B9C323FC7685D0D7951DC37B3F9ECB08A199 \
  --dst-chain gaia \
  --src-chain ibtc \
  --src-port transfer \
  --src-channel channel-0 \
  --amount 100000

  printf "\n"