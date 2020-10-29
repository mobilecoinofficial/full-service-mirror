#!/bin/sh

set -e

RELEASE_NAME="$1"
if [ -z "$RELEASE_NAME" ]; then
    echo "Usage: $0 [release name, e.g. mobilecoind-mirror-0.6.0]"
    exit 1
fi
: VERSION=${VERSION:?"Must provide VERSION for to substitute for db paths"}
: CONSENSUS_ENCLAVE_CSS=${CONSENSUS_ENCLAVE_CSS:?"Must provide CONSENSUS_ENCLAVE_CSS"}

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/.."

export SGX_MODE=HW
export IAS_MODE=PROD

# Build requires dependencies
cargo build -p mc-mobilecoind -p mc-mobilecoind-json --release --manifest-path $PROJECT_ROOT/mobilecoin/Cargo.toml
cargo build --manifest-path $PROJECT_ROOT/Cargo.toml --release

# Create release dir
mkdir $RELEASE_NAME
cp -R $SCRIPT_DIR/package/* $RELEASE_NAME
sed -i 's/VERSION/'${VERSION}'/g' $RELEASE_NAME/mobilecoind-mirror-private.sh
sed -i 's/VERSION/'${VERSION}'/g' $RELEASE_NAME/mobilecoind-mirror-private-tls.sh
sed -i 's/VERSION/'${VERSION}'/g' $RELEASE_NAME/mobilecoind-mirror-private-tls-encrypted.sh
cp $PROJECT_ROOT/mobilecoin/target/release/mobilecoind $RELEASE_NAME/bin/
cp $PROJECT_ROOT/mobilecoin/target/release/mobilecoind-json $RELEASE_NAME/bin/
cp $PROJECT_ROOT/target/release/mobilecoind-mirror-private $RELEASE_NAME/bin/
cp $PROJECT_ROOT/target/release/mobilecoind-mirror-public $RELEASE_NAME/bin/
tar -czvf $RELEASE_NAME.tar.gz $RELEASE_NAME

echo Created $RELEASE_NAME.tar.gz
