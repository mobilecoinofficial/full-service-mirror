#!/bin/bash
#!/bin/bash
set -exu
export RUST_LOG=DEBUG

openssl req -x509 -sha256 -nodes -newkey rsa:2048 -days 365 -keyout server.key -out server.crt

echo "Staring full service node"
./bin/full-service \
    --wallet-db ./data/wallet.db \
    --ledger-db ./data/ledger-db/ \
    --peer mc://node1.test.mobilecoin.com/ \
    --peer mc://node2.test.mobilecoin.com/ \
    --tx-source-url https://s3-us-west-1.amazonaws.com/mobilecoin.chain/node1.test.mobilecoin.com/ \
    --tx-source-url https://s3-us-west-1.amazonaws.com/mobilecoin.chain/node2.test.mobilecoin.com/ \
    --fog-ingest-enclave-css ./ingest-enclave.css \
    --listen-host 127.0.0.1 \
    > /tmp/mobilecoin-full-service.log 2>&1 &

sleep 5

echo "Starting the private side of the mirror."
./bin/wallet-service-mirror-private --wallet-service-uri http://127.0.0.1:9090/wallet --mirror-public-uri "wallet-service-mirror://localhost/?ca-bundle=server.crt"

