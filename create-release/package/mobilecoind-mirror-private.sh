#!/bin/bash
set -e
trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT
export RUST_LOG=INFO

PUBLIC_HOST="$1"
if [ -z "$PUBLIC_HOST" ]; then
    echo "Usage: $0 [public mirror host]"
    exit 1
fi


./bin/mobilecoind --ledger-db /tmp/mobilecoin/0.6.0/ledger \
      --poll-interval 1 \
      --peer mc://node1.test.mobilecoin.com/ \
      --peer mc://node2.test.mobilecoin.com/ \
      --tx-source-url https://s3-us-west-1.amazonaws.com/mobilecoin.chain/node1.test.mobilecoin.com/ \
      --mobilecoind-db /tmp/mobilecoin/0.6.0/wallet \
      --listen-uri insecure-mobilecoind://127.0.0.1:4444/ > /tmp/mobilecoind.log 2>&1 &
echo "Daemon is starting up (5 seconds)"
sleep 5

echo "Rest server is starting up (5 seconds)"
./bin/mobilecoind-json > /tmp/mobilecoind-json.log 2>&1 &
sleep 5


echo "Please provide account entropy:"
read ENTROPY

ACCT_KEY=$(curl localhost:9090/entropy/$ENTROPY)
curl localhost:9090/monitors -d "{\"account_key\":$ACCT_KEY,\"first_subaddress\": 0,\"num_subaddresses\":1000}" -X POST -H 'Content-Type: application/json'

echo

echo "Starting the private side of the mirror."
./bin/mobilecoind-mirror-private --mirror-public-uri "insecure-mobilecoind-mirror://$PUBLIC_HOST/" --mobilecoind-uri insecure-mobilecoind://127.0.0.1:4444/
