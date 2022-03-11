// Minimum supported NodeJS version: v12.9.0
const NODE_MAJOR_VERSION = process.versions.node.split('.')[0];
if (NODE_MAJOR_VERSION < 12) {
    throw new Error('Requires Node 12 (or higher)');
}

// Imports
const http = require('http');
const fs = require('fs');
const crypto = require('crypto');

// Command line parsing
if (process.argv.length != 6) {
    console.log(`Usage: node example-client.js <public mirror host> <public mirror port> <key file> <request>`);
    console.log(`For example: node example-client.js 127.0.0.1 9091 mirror-client.pem '{"method": "get_block", "params": {"block_index": "0"}, "jsonrpc": "2.0", "id": 1}'`);
    console.log('To generate keys please run the generate-rsa-keypair binary. See README.md for more details')
    return;
}

let public_mirror_host = process.argv[2];
let public_mirror_port = process.argv[3];
let key_file = process.argv[4];
let request = process.argv[5];

// Load key
let key_bytes = fs.readFileSync(key_file)
if (!key_bytes) {
    throw 'Failed loading key';
}
let key = crypto.createPublicKey(key_bytes);
if (!key) {
    throw 'Failed creating key';
}

// Ensure the key is 4096 bits (outputs 512-byte chunks).
const KEY_SIZE = 512;

let test_data = encrypt([1, 2, 3]);
if (test_data.length != KEY_SIZE) {
    throw `Key is not 4096-bit, encrypted output chunk size returned was ${test_data.length}`;
}

// Prepare request
let encrypted_request = encrypt(request);
// Send request to server
let req = http.request({
    host: public_mirror_host,
    port: public_mirror_port,
    timeout: 120000,
    path: '/encrypted-request',
    method: 'POST',
    headers: {
        'Content-Type': 'application/octet-stream',
        'Content-Length': Buffer.byteLength(encrypted_request)
    }
}, (response) => {
    let buf = []
    response.on('data', (chunk) => {
        buf.push(chunk)
    });

    response.on('end', () => {
        if (response.statusCode == 200) {
            let result = decrypt(Buffer.concat(buf)).toString();
            console.log(result);
        } else {
            console.log(`Http error, status: ${response.statusCode}: ${buf}`)
        }
    });

    response.on('error', (error) => {
        console.log('error occured while reading response:', error);
    })
}
)
req.write(encrypted_request)
req.end()

// Crypto utilities
function encrypt(buf) {
    let res = [];

    // Each encrypted chunk must be no longer than the length of the public modulus minus padding size.
    // PKCS1 is 11 bytes of padding (which is also defined as PKCS1_PADDING_LEN in the rust code).
    const MAX_CHUNK_SIZE = KEY_SIZE - 11;

    while (buf.length > 0) {
        let data = buf.slice(0, MAX_CHUNK_SIZE);
        buf = buf.slice(data.length);

        res.push(crypto.publicEncrypt({
            key: key,
            padding: crypto.constants.RSA_PKCS1_PADDING,
        }, Buffer.from(data)));
    }

    return Buffer.concat(res)
}

function decrypt(buf) {
    let res = [];

    while (buf.length > 0) {
        let data = buf.slice(0, KEY_SIZE);
        buf = buf.slice(data.length);

        res.push(crypto.publicDecrypt({
            key,
            padding: crypto.constants.RSA_PKCS1_PADDING,
        }, Buffer.from(data)));
    }

    return Buffer.concat(res)
}

function sign(buf) {
    return crypto.sign(null, Buffer.from(buf), { key, passphrase: '' })
}
