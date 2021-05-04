#!/bin/sh

set -e

RELEASE_NAME="$1"
if [ -z "$RELEASE_NAME" ]; then
    echo "Usage: $0 [release name, e.g. wallet-service-mirror-0.6.0]"
    exit 1
fi
: CONSENSUS_ENCLAVE_CSS=${CONSENSUS_ENCLAVE_CSS:?"Must provide CONSENSUS_ENCLAVE_CSS"}

SCRIPT_DIR="$( cd "$( dirname "$0" )" >/dev/null 2>&1 && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/.."

export SGX_MODE=HW
export IAS_MODE=PROD

# Build requires dependencies
cargo build -p mc-full-service --release --manifest-path $PROJECT_ROOT/full-service/Cargo.toml
cargo build --manifest-path $PROJECT_ROOT/Cargo.toml --release

# Create release dir
mkdir $RELEASE_NAME
cp -R $SCRIPT_DIR/package/* $RELEASE_NAME
cp $PROJECT_ROOT/full-service/target/release/full-service $RELEASE_NAME/bin/
cp $PROJECT_ROOT/target/release/wallet-service-mirror-private $RELEASE_NAME/bin/
cp $PROJECT_ROOT/target/release/wallet-service-mirror-public $RELEASE_NAME/bin/
tar -czvf $RELEASE_NAME.tar.gz $RELEASE_NAME

echo Created $RELEASE_NAME.tar.gz
