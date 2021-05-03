#!/bin/bash
set -e
trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT
export RUST_LOG=INFO

PUBLIC_HOST="$1"
if [ -z "$PUBLIC_HOST" ]; then
    echo "Usage: $0 [public mirror host]"
    exit 1
fi

if [ ! -f "mirror-private.pem" ]; then
    echo "mirror-private.pem does not exist. Did you run generate-keys.js?"
    exit 1
fi

mkdir -p /tmp/mobilecoin/wallet-db
./bin/full-service \
    --wallet-db /tmp/mobilecoin/wallet-db/wallet.db \
    --ledger-db /tmp/mobilecoin/ledger-db/ \
    --peer mc://node1.test.mobilecoin.com/ \
    --peer mc://node2.test.mobilecoin.com/ \
    --tx-source-url https://s3-us-west-1.amazonaws.com/mobilecoin.chain/node1.test.mobilecoin.com/ \
    --tx-source-url https://s3-us-west-1.amazonaws.com/mobilecoin.chain/node2.test.mobilecoin.com/ \
    > /tmp/mobilecoin-full-service.log 2>&1 &

echo "Daemon is starting up (5 seconds)"
sleep 5

ACCT_KEY=$(curl localhost:9090/entropy/$ENTROPY)
curl localhost:9090/monitors -d "{\"account_key\":$ACCT_KEY,\"first_subaddress\": 0,\"num_subaddresses\":1000}" -X POST -H 'Content-Type: application/json'

echo

echo "Starting the private side of the mirror."
./bin/wallet-service-mirror-private --mirror-public-uri "wallet-service-mirror://$PUBLIC_HOST/?ca-bundle=mirror.crt&tls-hostname=localhost" --wallet-service-uri http://localhost:9090/wallet --mirror-key mirror-private.pem
