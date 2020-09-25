#!/bin/bash

# Copyright (c) 2018-2020 MobileCoin Inc.
#
# Builds mobilecoind, mobilecoind-json, and mobilecoind-mirror, then runs
# an integration test on all endpoints.

set -ex

trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT

source "$HOME/.cargo/env"

RELEASE_DIR="$(dirname "$0")"
pushd ${RELEASE_DIR}

pushd ../../
MIRROR_ROOT=$(pwd)
MOBILECOIN_ROOT=$(pwd)/mobilecoin

# Build Vars
SGX_MODE=${SGX_MODE:-HW}
IAS_MODE=${IAS_MODE:-PROD}
NAMESPACE=${NAMESPACE:-test}
: ENTROPY=${ENTROPY:?"Must provide existing ENTROPY with balance for these tests."}


pushd ${MOBILECOIN_ROOT}

echo "Pulling down consensus validator signature material from enclave-distribution.${NAMESPACE}"
SIGSTRUCT_URI=$(curl -s https://enclave-distribution.${NAMESPACE}.mobilecoin.com/production.json | grep sigstruct | awk '{print $2}' | tr -d \")
curl -O https://enclave-distribution.${NAMESPACE}.mobilecoin.com/${SIGSTRUCT_URI}

TARGETDIR=${MOBILECOIN_ROOT}/target/release

echo "Building mobilecoind and mobilecoind-json"
SGX_MODE=${SGX_MODE} IAS_MODE=${IAS_MODE} \
  CONSENSUS_ENCLAVE_CSS=$(pwd)/consensus-enclave.css \
  cargo build --release -p mc-mobilecoind -p mc-mobilecoind-json

popd

echo "Building mobilecoind-mirror"
cargo build --release -p mc-mobilecoind-mirror \
  --bin mobilecoind-mirror-private \
  --bin mobilecoind-mirror-public

MIRROR_TARGETDIR=${MIRROR_ROOT}/target/release

# Remove artifacts from previous test runs
rm -rf /tmp/ledger-db /tmp/mobilecoind-db $(pwd)/mobilecoin.log $(pwd)/mobilecoind-json.log

# Start mobilecoind
echo "Starting local mobilecoind using ${NAMESPACE} servers for source of ledger. Check log at $(pwd)/mobilecoind.log."
${TARGETDIR}/mobilecoind \
        --ledger-db /tmp/ledger-db \
        --poll-interval 10 \
        --peer mc://node1.${NAMESPACE}.mobilecoin.com/ \
        --peer mc://node2.${NAMESPACE}.mobilecoin.com/ \
        --tx-source-url https://s3-us-west-1.amazonaws.com/mobilecoin.chain/node1.${NAMESPACE}.mobilecoin.com/ \
        --tx-source-url https://s3-us-west-1.amazonaws.com/mobilecoin.chain/node2.${NAMESPACE}.mobilecoin.com/ \
        --mobilecoind-db /tmp/mobilecoind-db \
        --listen-uri insecure-mobilecoind://127.0.0.1:4444/ &> $(pwd)/mobilecoind.log &

m_pid=$!

# Start mobilecoind-json - defaults are all good.
echo "Starting mobilecoind-json. Check log at $(pwd)/mobilecoind-json.log"
${TARGETDIR}/mobilecoind-json &> $(pwd)/mobilecoind-json.log &

mj_pid=$!

# Start mobilecoind-mirror
${MIRROR_TARGETDIR}/mobilecoind-mirror-public \
  --client-listen-uri http://0.0.0.0:8001/ \
  --mirror-listen-uri "insecure-mobilecoind-mirror://0.0.0.0/" &> mirror-public.log &

mpub_pid=$!

${MIRROR_TARGETDIR}/mobilecoind-mirror-private \
  --mirror-public-uri insecure-mobilecoind-mirror://127.0.0.1/ \
  --mobilecoind-uri insecure-mobilecoind://127.0.0.1:4444/ &> mirror-private.log &

mpriv_pid=$!

# Wait for mobilecoind-json and mirror to be live
sleep 5

source ${RELEASE_DIR}/assert.sh

failure_count=0

# FIXME: Wait for mobilecoind to sync the ledger

# Generate a new entropy
entropy=$(curl -s localhost:9090/entropy -X POST)
if [[ $(echo $entropy | jq '.entropy?') != 'null' ]]; then
  log_success "entropy"
else
  log_failure "entropy"
  failure_count=$(( $failure_count + 1 ))
fi

# add and remove dummy entropy
# remove=$(curl -s -X DELETE localhost:9090/monitors/$monitor_id)
# FIXME: Try to remove twice and see that it fails

# Generate account key from entropy - Use an the provided ENTROPY with a balance on the NAMESPACE network
account_key=$(curl -s localhost:9090/entropy/$ENTROPY)
if [[ $(echo $account_key | jq '.view_private_key?') != 'null' ]] && [[ $(echo $account_key | jq '.spend_private_key?') != 'null' ]]; then
  log_success "account_key"
else
  log_failure "account_key"
  failure_count=$(( $failure_count + 1 ))
fi

# Add a monitor for a key over a range of subaddress indices
monitor=$(curl -s localhost:9090/monitors \
  -d '{"account_key": '$account_key',
       "first_subaddress": 0, "num_subaddresses": 10}' \
  -X POST -H 'Content-Type: application/json')
if [[ $(echo $monitor | jq '.monitor_id?') != 'null' ]]; then
  log_success "monitor_id"
else
  log_failure "monitor_id"
  failure_count=$(( $failure_count + 1 ))
fi

monitor_id=$(echo $monitor | jq -r '.monitor_id')

# Get the status of an existing monitor
status=$(curl -s localhost:9090/monitors/$monitor_id)
if [[ $(echo $status | jq '.first_subaddress?') != 'null' ]] \
  && [[ $(echo $status | jq '.num_subaddresses?') != 'null' ]] \
  && [[ $(echo $status | jq '.first_block?') != 'null' ]] \
  && [[ $(echo $status | jq '.next_block?') != 'null' ]]; then
  log_success "monitor status"
else
  log_failure "monitor status"
  failure_count=$(( $failure_count + 1 ))
fi

# Check balance
balance=$(curl -s localhost:9090/monitors/$monitor_id/subaddresses/0/balance)
if [[ $(echo $balance | jq '.balance?') != 'null' ]] \
  && [[ $(( $(echo $balance | jq -r '.balance') > 0)) == 1 ]]; then
  log_success "balance"
else
  log_failure "balance"
  failure_count=$(( $failure_count + 1 ))
fi

public_address=$(curl -s localhost:9090/monitors/$monitor_id/subaddresses/0/public-address)
if [[ $(echo $public_address | jq '.view_public_key?') != 'null' ]] \
  && [[ $(echo $public_address | jq '.spend_public_key?') != 'null' ]] \
  && [[ $(echo $public_address | jq '.fog_report_url?') != 'null' ]] \
  && [[ $(echo $public_address | jq '.fog_authority_fingerprint_sig?') != 'null' ]] \
  && [[ $(echo $public_address | jq '.fog_report_id?') != 'null' ]] \
  && [[ $(echo $public_address | jq '.b58_address_code?') != 'null' ]]; then
  log_success "pubic address"
else
  log_failure "public address"
  failure_count=$(( $failure_count + 1 ))
fi

address_code=$(echo $public_address | jq -r '.b58_address_code')
pay=$(curl -s http://localhost:9090/monitors/$monitor_id/subaddresses/0/pay-address-code \
  -d '{"receiver_b58_address_code": "'$address_code'", "value": "1"}' \
  -X POST -H 'Content-Type: application/json')
if [[ $(echo $pay | jq '.sender_tx_receipt?') != 'null' ]] \
  && [[ $(echo $pay | jq '.receiver_tx_receipt_list?') != 'null' ]]; then
  log_success "pay"
else
  log_failure "pay"
  failure_count=$(( $failure_count + 1 ))
fi

status=$(curl -s localhost:9090/tx/status-as-sender -d $pay -X POST -H 'Content-Type: application/json')
if [[ $(echo $status | jq '.status?') != 'null' ]] \
  && [[ $(echo $status | jq '.status? == "verified"') == true ]]; then
  log_success "status"
else
  log_failure "status"
  failure_count=$(( $failure_count + 1 ))
fi

receipt=$(echo $pay | jq -c '.receiver_tx_receipt_list[0]')
status=$(curl -s localhost:9090/tx/status-as-receiver \
  -d $receipt -X POST -H 'Content-Type: application/json')
if [[ $(echo $status | jq '.status?') != 'null' ]] \
  && [[ $(echo $status | jq '.status? == "verified"') == true ]]; then
  log_success "status"
else
  log_failure "status"
  failure_count=$(( $failure_count + 1 ))
fi

code=$(curl -s localhost:9090/codes/request \
  -d '{"receiver": '$public_address', "value": "10", "memo": "Test"}'\
  -X POST -H 'Content-Type: application/json')
if [[ $(echo $code | jq '.b58_code?') != 'null' ]]; then
  log_success "code"
else
  log_failure "code"
  failure_count=$(( $failure_count + 1 ))
fi

request_code=$(echo $code | jq -r '.b58_code')
request_data=$(curl -s localhost:9090/codes/request/$request_code)
if [[ $(echo $request_data | jq '.receiver?') != 'null' ]] \
  && [[ $(echo $request_data | jq '.value?') != 'null' ]] \
  && [[ $(echo $request_data | jq '.memo?') != 'null' ]]; then
  log_success "status"
else
  log_failure "status"
  failure_count=$(( $failure_count + 1 ))
fi

submit=$(curl -s localhost:9090/monitors/$monitor_id/subaddresses/0/build-and-submit \
  -d '{"request_data": '$request_data'}' \
  -X POST -H 'Content-Type: application/json')
if [[ $(echo $submit | jq '.sender_tx_receipt?') != 'null' ]] \
  && [[ $(echo $submit | jq '.receiver_tx_receipt_list?') != 'null' ]]; then
  log_success "submit"
else
  log_failure "submit"
  failure_count=$(( $failure_count + 1 ))
fi

# Ledger info
block_height=$(curl -s localhost:9090/ledger/local)
if [[ $(echo $block_height | jq '.block_count?') != 'null' ]] \
  && [[ $(echo $block_height | jq '.txo_count?') != 'null' ]]; then
  log_success "block_height"
else
  log_failure "block_height"
  failure_count=$(( $failure_count + 1 ))
fi

# Counts for a specific block
counts=$(curl -s localhost:9090/ledger/blocks/1/header)
if [[ $(echo $counts | jq '.key_image_count?') != 'null' ]] \
  && [[ $(echo $counts | jq '.txo_count?') != 'null' ]]; then
  log_success "counts"
else
  log_failure "counts"
  failure_count=$(( $failure_count + 1 ))
fi

# Details for a specific block
details=$(curl -s localhost:9090/ledger/blocks/1)
if [[ $(echo $details | jq '.block_id?') != 'null' ]] \
  && [[ $(echo $details | jq '.version?') != 'null' ]] \
  && [[ $(echo $details | jq '.parent_id?') != 'null' ]] \
  && [[ $(echo $details | jq '.index?') != 'null' ]] \
  && [[ $(echo $details | jq '.cumulative_txo_count?') != 'null' ]] \
  && [[ $(echo $details | jq '.contents_hash?') != 'null' ]]; then
  log_success "details"
else
  log_failure "details"
  failure_count=$(( $failure_count + 1 ))
fi

# Offline UTXOS
#utxos=$(curl -s localhost:9090/monitors/$monitor_id/subaddresses/0/utxos)
## FIXME: check contents
#
#utxo=$(echo $utxos | jq '.output_list[0]')
#proposal=$(curl -s localhost:9090/monitors/$monitor-id/subaddresses/0/generate-tx \
#  -d '{"input_list": ['$utxo'], "transfer": '$request_data'}' \
#  -X POST -H 'Content-Type: application/json')
## FIXME: Getting empty return

# Hit the mirror endpoints
# Ledger info
block_height=$(curl -s localhost:8001/ledger/local)
if [[ $(echo $block_height | jq '.block_count?') != 'null' ]] \
  && [[ $(echo $block_height | jq '.txo_count?') != 'null' ]]; then
  log_success "block_height"
else
  log_failure "block_height"
  failure_count=$(( $failure_count + 1 ))
fi

# Counts for a specific block
counts=$(curl -s localhost:8001/ledger/blocks/1/header)
if [[ $(echo $counts | jq '.key_image_count?') != 'null' ]] \
  && [[ $(echo $counts | jq '.txo_count?') != 'null' ]]; then
  log_success "counts"
else
  log_failure "counts"
  failure_count=$(( $failure_count + 1 ))
fi

# Details for a specific block
details=$(curl -s localhost:8001/ledger/blocks/1)
if [[ $(echo $details | jq '.block_id?') != 'null' ]] \
  && [[ $(echo $details | jq '.version?') != 'null' ]] \
  && [[ $(echo $details | jq '.parent_id?') != 'null' ]] \
  && [[ $(echo $details | jq '.index?') != 'null' ]] \
  && [[ $(echo $details | jq '.cumulative_txo_count?') != 'null' ]] \
  && [[ $(echo $details | jq '.contents_hash?') != 'null' ]]; then
  log_success "details"
else
  log_failure "details"
  failure_count=$(( $failure_count + 1 ))
fi

for pid in $m_pid $mj_pid $mpub_pid $mpriv_pid; do
  wait $pid
done

popd


