## wallet service mirror

To launch, you will need to start both the public side of the mirror and the private side.
The public side accepts client requests on port 8001 and mirror requests from the private side on port 10080.

First, launch the public side: `./wallet-service-mirror-public.sh`.
Now, launch the private side: `./wallet-service-mirror-private.sh localhost` (assuming you are running both public and private on the same machine for test purposes). It will ask you for an entropy (account key) to use.

Once launched, you can test it using curl:

Get block information (for block 0):
```
curl -X POST -H 'Content-Type: application/json' -d '{"method": "get_block", "params": {"block_index": "0"}, "jsonrpc": "2.0", "id": 1}' http://localhost:9091/unsigned-request
```
This returns:
```
{"method":"get_block","result":{"block":{"id":"dba9b5bb61dc3941c6730a4c5e9b81f30f9def32abd4251d0715100072a7425e","version":"0","parent_id":"0000000000000000000000000000000000000000000000000000000000000000","index":"0","cumulative_txo_count":"16","root_element":{"range":{"from":"0","to":"0"},"hash":"0000000000000000000000000000000000000000000000000000000000000000"},"contents_hash":"882cea8bf5e082294ae1707ad2841c6f4846ece978d077f15bc090ac97885e81"},"block_contents":{"key_images":[],"outputs":[{"amount":{"commitment":"3a72e2231c1462354dfe6d4c289d05c67a528dfcdba52d8d87c07914c507dc5f","masked_value":"28067792405079518"},"target_key":"8c43d0e80adcf7c8a59f6350d010f7b257f2d6454efa7ca693eb92180a06ee6c","public_key":"50c5916be94c0dcba5054fe2852422ec7c5e208cb31355b8e74e8c4ed007a60b","e_fog_hint":"05e32fee11b4612c9fd54f97e9662c8e576ab91d062c62295974cdd940d0a257eb8ce687e9bbbf8e6dccb0ec16bf15ad6902f9c249d2fe1ed198918ec1c614a48b299c657aa32b9e5c3580f24c07e354b31e0100"},{"amou...
```

For other requests please see https://mobilecoin.gitbook.io/full-service-api/

If you want to have a TLS connection between the mirror sides, use `wallet-service-mirror-public-tls.sh` and `wallet-service-mirror-private-tls.sh`. Note that they use a self-signed test certificate stored inside `mirror.crt` / `mirror.key` that was generated with `openssl req -x509 -sha256 -nodes -newkey rsa:2048 -days 365 -keyout mirror.key -out mirror.crt`.


### End-to-end encryption and request verification

It is possible to run the mirror in a mode that causes it to authenticate requests from clients, and encrypt responses. In this mode, anyone having access to the public side of the mirror will be unable to tamper with requests or view response data. When running in this mode, which is enabled by passing the `--mirror-key` argument to the private side of the mirror, only signed requests will be processed and only encrypted responses will be returned.

In order to use this mode, follow the following steps.
1) Ensure that you have NodeJS installed. **The minimum supported version is v12.9.0** (`node -v`)
1) Generate a keypair: `node generate-keys.js`. This will generate two files: `mirror-client.pem` and `mirror-private.pem`.
1) Run the public side of the mirror: `./wallet-service-mirror-public-tls.sh` (the public side does not care if the private side and mirror are using end to end encryption as it just forwards the requests and responses).
1) Run the private side of the mirror: `./wallet-service-mirror-private-tls-encrypted.sh localhost`
1) 1) Issue a response using the sample client:
   - To get block data: `node example-client.js 127.0.0.1 9091 mirror-client.pem '{"method": "get_block", "params": {"block_index": "0"}, "jsonrpc": "2.0", "id": 1}'`
