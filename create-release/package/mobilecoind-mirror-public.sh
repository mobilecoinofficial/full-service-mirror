#!/bin/bash
set -e
trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT
export RUST_LOG=INFO

./bin/mobilecoind-mirror-public --client-listen-uri http://0.0.0.0:8001/ --mirror-listen-uri "insecure-mobilecoind-mirror://0.0.0.0/"
