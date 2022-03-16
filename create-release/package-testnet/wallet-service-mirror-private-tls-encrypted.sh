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
    echo "mirror-private.pem does not exist. Did you run generate-rsa-keypair?"
    exit 1
fi

mkdir -p ./validator-dbs
./bin/mc-validator-service \
    --ledger-db ./validator-vbs/ledger-db/ \
    --peer mc://node1.test.mobilecoin.com/ \
    --peer mc://node2.test.mobilecoin.com/ \
    --tx-source-url https://s3-us-west-1.amazonaws.com/mobilecoin.chain/node1.test.mobilecoin.com/ \
    --tx-source-url https://s3-us-west-1.amazonaws.com/mobilecoin.chain/node2.test.mobilecoin.com/ \
    --listen-uri insecure-validator://localhost:5554/tmp \
    > /tmp/mobilecoin-full-service.log 2>&1 &

mkdir -p ./full-service-dbs
./bin/full-service \
    --wallet-db ./full-service-dbs/wallet.db \
    --ledger-db ./full-service-dbs/ledger-db/ \
    --validator insecure-validator://localhost:5554/ \
    --fog-ingest-enclave-css ./ingest-enclave.css \
    > /tmp/mobilecoin-full-service.log 2>&1 &

echo "Daemon is starting up (5 seconds)"
sleep 5

echo "Starting the private side of the mirror."
./bin/wallet-service-mirror-private --mirror-public-uri "wallet-service-mirror://$PUBLIC_HOST/?ca-bundle=server.crt&tls-hostname=localhost" --wallet-service-uri http://localhost:9090/wallet --mirror-key mirror-private.pem
