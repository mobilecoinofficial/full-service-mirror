#!/bin/bash
set -e
trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT
export RUST_LOG=INFO

PUBLIC_HOST="$1"
if [ -z "$PUBLIC_HOST" ]; then
    echo "Usage: $0 [public mirror host]"
    exit 1
fi

mkdir -p ./full-service-dbs
./bin/full-service \
    --wallet-db ./full-service-dbs/wallet.db \
    --ledger-db ./full-service-dbs/ledger-db/ \
    --peer mc://node1.prod.mobilecoinww.com/ \
    --peer mc://node2.prod.mobilecoinww.com/ \
    --tx-source-url https://ledger.mobilecoinww.com/node1.prod.mobilecoinww.com/ \
    --tx-source-url https://ledger.mobilecoinww.com/node2.prod.mobilecoinww.com/ \
    --fog-ingest-enclave-css ./ingest-enclave.css \
    > /tmp/mobilecoin-full-service.log 2>&1 &

echo "Daemon is starting up (5 seconds)"
sleep 5

echo "Starting the private side of the mirror."
./bin/wallet-service-mirror-private --mirror-public-uri "wallet-service-mirror://$PUBLIC_HOST/?ca-bundle=mirror.crt&tls-hostname=localhost" --wallet-service-uri http://localhost:9090/wallet 
