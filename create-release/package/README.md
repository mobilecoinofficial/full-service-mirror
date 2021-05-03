## wallet service mirror

To launch, you will need to start both the public side of the mirror and the private side.
The public side accepts client requests on port 8001 and mirror requests from the private side on port 10080.

First, launch the public side: `./wallet service-mirror-public.sh`.
Now, launch the private side: `./wallet service-mirror-private.sh localhost` (assuming you are running both public and private on the same machine for test purposes). It will ask you for an entropy (account key) to use.

Once launched, you can test it using curl:

Get block information (for block 0):
```
$ curl http://localhost:8001/block/0/
{"block_id":"e498010ee6a19b4ac9313af43d8274c53d54a1bbc275c06374dbe0095872a6ee","version":0,"parent_id":"0000000000000000000000000000000000000000000000000000000000000000","index":"0","cumulative_txo_count":"10000","contents_hash":"40bffaff21f4825bc36e4598c3346b375fe77ec1c78f15c8a98623c0ba6b1d21"}
```

Get processed block information:
```
$ curl http://localhost:8001/processed-block/33826/
{"tx_outs":[{"monitor_id":"08b4e048afc793213fae60d6ad69a5cb73e43a0d1ebba1cdaaf008a912acf1c3","subaddress_index":0,"public_key":"0ce630939a15c9314b36323547fe671d3865622f04190c377571f8c94a066700","key_image":"d20b42ad18a31048e69ea50a5136363f84cca3558a06d1d2c7b6e069fbcf5a53","value":"999999999840","direction":"received"},{"monitor_id":"08b4e048afc793213fae60d6ad69a5cb73e43a0d1ebba1cdaaf008a912acf1c3","subaddress_index":0,"public_key":"58292cdd7f2d7c3caf885d9bbeca69f17d2e15fe781fc31eafbdb9506433560d","key_image":"d6716d7c4f038a847b2f106eed62c0ce59c2e0eecfcf1d1da473bd26e9864d58","value":"999999999890","direction":"spent"}]}
```


If you want to have a TLS connection between the mirror sides, use `wallet service-mirror-public-tls.sh` and `wallet service-mirror-private-tls.sh`. Note that they use a self-signed test certificate stored inside `mirror.crt` / `mirror.key` that was generated with `openssl req -x509 -sha256 -nodes -newkey rsa:2048 -days 365 -keyout mirror.key -out mirror.crt`.


### End-to-end encryption and request verification

It is possible to run the mirror in a mode that causes it to authenticate requests from clients, and encrypt responses. In this mode, anyone having access to the public side of the mirror will be unable to tamper with requests or view response data. When running in this mode, which is enabled by passing the `--mirror-key` argument to the private side of the mirror, only signed requests will be processed and only encrypted responses will be returned.

In order to use this mode, follow the following steps.
1) Ensure that you have NodeJS installed. **The minimum supported version is v12.9.0** (`node -v`)
1) Generate a keypair: `node generate-keys.js`. This will generate two files: `mirror-client.pem` and `mirror-private.pem`.
1) Run the public side of the mirror: `./wallet service-mirror-public-tls.sh` (the public side does not care if the private side and mirror are using end to end encryption as it just forwards the requests and responses).
1) Run the private side of the mirror: `./wallet service-mirror-private-tls-encrypted.sh`
1) 1) Issue a response using the sample client:
   - To get block data: `node example-client.js 127.0.0.1 9091 mirror-client.pem '{"method": "get_block", "params": {"block_index": "0"}, "jsonrpc": "2.0", "id": 1}'`
