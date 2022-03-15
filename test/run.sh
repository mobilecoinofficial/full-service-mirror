#!/bin/bash
set -eu
export RUST_LOG=DEBUG

mnemonic="MNEMONIC HERE"

openssl req -x509 -sha256 -nodes -newkey rsa:2048 -days 365 -keyout server.key -out server.crt -subj "/C=US/ST=CA/L=SF/O=MobileCoin/OU=IT/CN=localhost"

echo "Starting ledger validator node"

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


echo "Starting full service node"

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

declare -a MethodList=("assign_address_for_account" "build_and_submit_transaction" "build_gift_code" "build_split_txo_transaction" "build_transaction" "check_b58_type" "check_gift_code_status" "check_receiver_receipt_status" "claim_gift_code" "create_account"  "create_receiver_receipts" "export_account_secrets" "get_all_addresses_for_account" "get_all_gift_codes" "get_all_transaction_logs_for_account" "get_all_transaction_logs_ordered_by_block" "get_all_txos_for_account" "get_all_txos_for_address" "get_gift_code" "get_mc_protocol_transaction" "get_mc_protocol_txo" "get_txo" "get_txos_for_account" "import_account" "import_account_from_legacy_root_entropy" "remove_account" "remove_gift_code" "submit_gift_code" "submit_transaction" "update_account_name")
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
echo "$method return $response which was not 'Http error, status: 400: Unsupported request'"
exit 42
fi
done

response=$(node ./test_suite/test_script.js 127.0.0.1 9091 127.0.0.1 9090 mirror-client.pem "${mnemonic}")
echo "Test result: $response"    


exit 0


