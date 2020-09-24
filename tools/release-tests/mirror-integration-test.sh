#!/bin/bash

# Copyright (c) 2018-2020 MobileCoin Inc.
#
# Builds mobilecoind, mobilecoind-json, and mobilecoind-mirror, then runs
# an integration test on all endpoints.

set -ex

trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT

source "$HOME/.cargo/env"

pushd "$(dirname "$0")"

pushd ../../
MIRROR_ROOT=$(pwd)
MOBILECOIN_ROOT=$(pwd)/mobilecoin

# Build Vars
SGX_MODE=${SGX_MODE:-HW}
IAS_MODE=${IAS_MODE:-PROD}
NAMESPACE=${NAMESPACE:-test}

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
  --bin mobilecoind-mirror-privagte \
  --bin mobilecoind-mirror-public

# Remove artifacts from previous test runs
rm -rf /tmp/ledger-db /tmp/mobilecoind-db $(pwd)/mobilecoin.log $(pwd)/mobilecoind-json.log

# Start mobilecoind
echo "Starting local mobilecoind using ${NAMESPACE} servers for source of ledger. Check log at $(pwd)/mobilecoind.log."
${TARGETDIR}/mobilecoind \
        --ledger-db /tmp/ledger-db \
        --poll-interval 10 \
        --peer mc://node1.${NAMESPACE}.mobilecoin.com/ \
        --peer mc://node2.${NAMESPACE}.mobilecoin.com/ \
        --tx-source-url https://s3-us-west-1.amazonaws.com/mobilecoin.chain/node1.${NAMESAPCE}.mobilecoin.com/ \
        --tx-source-url https://s3-us-west-1.amazonaws.com/mobilecoin.chain/node2.${NAMESPACE}.mobilecoin.com/ \
        --mobilecoind-db /tmp/mobilecoind-db \
        --listen-uri insecure-mobilecoind://127.0.0.1:4444/ &> $(pwd)/mobilecoind.log &

m_pid=$!

# Start mobilecoind-json - defaults are all good.
echo "Starting mobilecoind-json. Check log at $(pwd)/mobilecoind-json.log"
${TARGETDIR}/mobilecoind-json &> $(pwd)/mobilecoind-json.log &

mj_pid=$!

# Wait for mobilecoind to sync the ledger.
block_height=$(curl localhost:9090/ledger/local | jq)
echo $block_height

# Helper method to assert equalities
assert_eq() {
  local expected="$1"
  local actual="$2"
  local msg

  if [ "$#" -ge 3 ]; then
    msg="$3"
  fi

  if [ "$expected" == "$actual" ]; then
    return 0
  else
    [ "${#msg}" -gt 0 ] && log_failure "$expected == $actual :: $msg" || true
    return 1
  fi
}

# Test block endpoint
expected='{"block_id":"e498010ee6a19b4ac9313af43d8274c53d54a1bbc275c06374dbe0095872a6ee","version":0,"parent_id":"0000000000000000000000000000000000000000000000000000000000000000","index":"0","cumulative_txo_count":"10000","contents_hash":"40bffaff21f4825bc36e4598c3346b375fe77ec1c78f15c8a98623c0ba6b1d21"}'

popd


