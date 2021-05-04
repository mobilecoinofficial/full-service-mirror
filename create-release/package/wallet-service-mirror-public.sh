#!/bin/bash
set -e
trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT
export RUST_LOG=INFO

./bin/wallet-service-mirror-public --client-listen-uri http://0.0.0.0:9091/ --mirror-listen-uri "insecure-wallet-service-mirror://0.0.0.0/"
