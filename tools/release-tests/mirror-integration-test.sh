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

TARGETDIR=./target/release

echo "Building mobilecoind and mobilecoind-json"
SGX_MODE=${SGX_MODE} IAS_MODE=${IAS_MODE} CONSENSUS_ENCLAVE_CSS=$(pwd)/consensus-enclave.css \
        cargo build --release -p mc-mobilecoind -p mc-testnet-client




