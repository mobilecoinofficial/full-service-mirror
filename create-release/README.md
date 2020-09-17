This directory contains a shell script + associated files for generating a mobilecoind-mirror release.
To create a release, run `./create-release.sh [release name]`, e.g. `./create-release.sh mobilecoind-mirror-0.6.0`. This will create a directory named `mobilecoind-mirror-0.6.0` as well as an archive `mobilecoind-mirror-0.6.0.tar.gz`.

Note that you need to have the `SGX_MODE`/`IAS_MODE`/`CONSENSUS_ENCLAVE_CSS` environment variables set based on the network you are building for.
