#!/bin/bash
set -e
trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT
export RUST_LOG=INFO

./bin/mobilecoind-mirror-public --client-listen-uri http://0.0.0.0:9091/ --mirror-listen-uri "mobilecoind-mirror://0.0.0.0/?tls-chain=mirror.crt&tls-key=mirror.key" --allow-self-signed-tls
