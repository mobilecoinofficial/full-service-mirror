#!/bin/bash

# Copyright (c) 2018-2020 MobileCoin Inc.
#
# Builds linux client tarball for a release.
#
# Usage:
#
# `./build-mobilecoind-release.sh`

set -ex

trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT

pushd "$(dirname "$0")"

pushd ../../
MIRROR_ROOT=$(pwd)
MOBILECOIN_ROOT=$(pwd)/mobilecoin

# Input Vars
: TAG_DATE=${TAG_DATE:?"Must set TAG_DATE to determine release-artifacts location"}
: NAMESPACE=${NAMESPACE:?"Must provide namespace for deployment logging substitution"}
: VERSION=${VERSION:?"Must provide VERSION for docker image tags"}
: AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID:?"Must provide AWS_ACCESS_KEY_ID"}
: AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY:?"Must provide AWS_SECRET_ACCESS_KEY"}

# Build Vars
SGX_MODE=${SGX_MODE:-HW}
IAS_MODE=${IAS_MODE:-PROD}

pushd ${MOBILECOIN_ROOT}
  
# Release Vars
RELEASE_REVISION=${RELEASE_REVISION:-$( git rev-parse HEAD )}
ARTIFACTS=${INTERNAL}/release-artifacts/enclave-${TAG_DATE}
CONSENSUS_ENCLAVE_CSS=${ARTIFACTS}/consensus-enclave.css

# Sanity check that css matches built
curl -O https://enclave-distribution.${NAMESPACE}.mobilecoin.com/pool/${RELEASE_REVISION}/${SIGNER_HASH}/consensus-enclave.css
if [[ $(md5sum $CONSENSUS_ENCLAVE_CSS) != $(md5sum consensus-enclave.css) ]]; then
  echo "Files differ. Note, verify that the S3 cache has expired. Proceeding with artifact enclave."
fi

# Build the binaries
RUSTFLAGS=' ' SGX_MODE=${SGX_MODE} IAS_MODE=${IAS_MODE} CONSENSUS_ENCLAVE_CSS=${CONSENSUS_ENCLAVE_CSS} \
  cargo build --release -p mc-mobilecoind -p mc-testnet-client

# Client directory is non-versioned because it is used for mobilecoind distribution in other scripts
mkdir -p mobilecoin-testnet-linux/bin
cp target/release/{mobilecoind,mc-testnet-client} mobilecoin-testnet-linux/bin

mkdir -p ${MOBILECOIN_ROOT}/mobilecoind/release-packages/${VERSION}
cp ${MOBILECOIN_ROOT}/mobilecoind/release-packages/v0.4.0/mobilecoin-testnet.sh ${MOBILECOIN_ROOT}/mobilecoind/release-packages/${VERSION}
sed -i 's/v0.4.0/${VERSION}/g' ${PUBLIC}/mobilecoind/release-packages/${VERSION}/mobilecoin-testnet.sh
cp ${MOBILECOIN_ROOT}/mobilecoind/release-packages/${VERSION}/mobilecoin-testnet.sh mobilecoin-testnet-linux/

# Modify the startup script
tar -czvf mobilecoin-testnet-linux.tar.gz mobilecoin-testnet-linux/
