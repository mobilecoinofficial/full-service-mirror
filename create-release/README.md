This directory contains a shell script + associated files for generating a wallet-service-mirror release.
To create a release, run `./create-release.sh [release name]`, e.g. `./create-release.sh wallet-service-mirror-0.6.0`. This will create a directory named `wallet-service-mirror-0.6.0` as well as an archive `wallet-service-mirror-0.6.0.tar.gz`.

Note that you need to have the `SGX_MODE`/`IAS_MODE`/`CONSENSUS_ENCLAVE_CSS` environment variables set based on the network you are building for.
