#!/bin/bash
set -eu
export RUST_LOG=DEBUG

echo "v3"

openssl req -x509 -sha256 -nodes -newkey rsa:2048 -days 365 -keyout server.key -out server.crt -subj "/C=US/ST=CA/L=SF/O=MobileCoin/OU=IT/CN=localhost"

echo "Staring ledger validator node"

mkdir -p ./lvn-dbs
./bin/mc-validator-service \
   --ledger-db ./lvn-dbs/ledger-db/ \
   --peer mc://node1.test.mobilecoin.com/ \
   --peer mc://node2.test.mobilecoin.com/ \
   --tx-source-url https://s3-us-west-1.amazonaws.com/mobilecoin.chain/node1.test.mobilecoin.com/ \
   --tx-source-url https://s3-us-west-1.amazonaws.com/mobilecoin.chain/node2.test.mobilecoin.com/ \
   --listen-uri "validator://localhost:5554/?tls-chain=server.crt&tls-key=server.key" \
    > /tmp/mobilecoin-validator.log 2>&1 &

sleep 5


echo "Staring full service node"

mkdir -p ./fs-dbs/wallet-db/
./bin/full-service \
   --wallet-db ./fs-dbs/wallet-db/wallet.db \
   --ledger-db ./fs-dbs/ledger-db/ \
   --validator "validator://localhost:5554/?ca-bundle=server.crt&tls-hostname=localhost" \
   --fog-ingest-enclave-css ./ingest-enclave.css \
    > /tmp/mobilecoin-full-service.log 2>&1 &

sleep 5

echo "generate keypair for mirror"

./bin/generate-rsa-keypair

echo "Starting the public side of the mirror."
./bin/wallet-service-mirror-public --client-listen-uri http://0.0.0.0:9091/ --mirror-listen-uri "wallet-service-mirror://0.0.0.0/?tls-chain=server.crt&tls-key=server.key" --allow-self-signed-tls \
    > /tmp/mobilecoin-public-mirror.log 2>&1 &

echo "Starting the private side of the mirror."
./bin/wallet-service-mirror-private --mirror-public-uri "wallet-service-mirror://localhost/?ca-bundle=server.crt&tls-hostname=localhost" --wallet-service-uri http://localhost:9090/wallet --mirror-key mirror-private.pem \
    > /tmp/mobilecoin-private-mirror.log 2>&1 &

declare -a MethodList=("assign_address_for_account" )
for method in ${MethodList[@]}; do
response=$(node example-client.js 127.0.0.1 9091 mirror-client.pem "{
    \"method\": \"${method}\",
    \"params\": {},
    \"jsonrpc\": \"2.0\", 
    \"api_version\": \"2\", 
    \"id\": 1
    }")
echo "node returned: $response"    
if [ "$response" != 'Http error, status: 400: Unsupported request' ] 
then
exit 42
fi

response=$(node example-client.js 127.0.0.1 9091 mirror-client.pem '{
    "method": "get_block",
    "params": {
        "block_index": "0"
    },
    "jsonrpc": "2.0",
    "id": 1
    }')
method="get_block"
mnemonic=""
params="
    \"mnemonic\": \"$mnemonic\",
    \"key_derivation_version\": \"2\",
    \"name\": \"AccountName\"
"
reponse=$(curl -s localhost:9090/wallet \
 -d "{
    \"method\": \"${method}\",
    \"params\": {$params},
    \"jsonrpc\": \"2.0\", 
    \"api_version\": \"2\", 
    \"id\": 1
    }
" \
 -X POST -H 'Content-type: application/json' | jq
 )
echo "node returned: $response"    

done
exit 0


