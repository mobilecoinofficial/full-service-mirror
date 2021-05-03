#!/bin/bash
set -e
trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT
export RUST_LOG=INFO

PUBLIC_HOST="$1"
if [ -z "$PUBLIC_HOST" ]; then
    echo "Usage: $0 [public mirror host]"
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

echo "Starting the private side of the mirror."
./bin/mobilecoind-mirror-private --mirror-public-uri "mobilecoind-mirror://$PUBLIC_HOST/?ca-bundle=mirror.crt&tls-hostname=localhost" --wallet-service-uri http://localhost:9090/wallet 
