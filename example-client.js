// Minimum supported NodeJS version: v12.9.0
const NODE_MAJOR_VERSION = process.versions.node.split('.')[0];
if (NODE_MAJOR_VERSION < 12) {
  throw new Error('Requires Node 12 (or higher)');
}

// Imports
const http = require('http');
const fs = require('fs');
const crypto = require('crypto');

// TODO: Command line parsing
let key_file = 'beam-private.pem';
let public_mirror_host  = '127.0.0.1';
let public_mirror_port = 8001;

// Load key
// TODO: Make this a command line argument
let key_bytes = fs.readFileSync('beam-private.pem')
if (!key_bytes) {
    throw 'Failed loading key';
}
let key = crypto.createPrivateKey({key: key_bytes, passphrase: ''});
if (!key) {
    throw 'Failed creating key';
}

// Ensure the key is 4096 bits (outputs 512-byte chunks).
const KEY_SIZE = 512;

let test_data = encrypt([1, 2, 3]);
if (test_data.length != KEY_SIZE) {
    throw `Key is not 4096-bit, encrypted output chunk size returned was ${test_data.length}`;
}

// Example request
let request = JSON.stringify({GetBlock: {block: 0}});

let signature = sign(request);
let post_data = JSON.stringify({
    request: request,
    signature: [...signature],
});

// Send request to server
let req = http.request({
    host: public_mirror_host,
    port: public_mirror_port,
    path: '/signed-request',
	method: 'POST',
	headers: {
	    'Content-Type': 'application/json',
	    'Content-Length': Buffer.byteLength(post_data)
	}
}, (response) => {
    	let buf = []
        response.on('data', (chunk) => {
            buf.push(chunk)
        });

        response.on('end', () => {
            if(response.statusCode == 200) {
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
req.write(post_data)
req.end()

// Crypto utilities
function encrypt(buf) {
    let res = [];
    let pub_key = crypto.createPublicKey(key);

    // Each encrypted chunk must be no longer than the length of the public modulus minus (2 + 2*hash.size()).
    // Since hash size is 32 (SHA256), this equals to 66.
    const MAX_CHUNK_SIZE = KEY_SIZE - 66;

    while (buf.length > 0)
    {
        let data = buf.slice(0, MAX_CHUNK_SIZE);
        buf = buf.slice(data.length);

        res.push(crypto.publicEncrypt({
            key: pub_key,
            oaepHash: 'sha256',
            padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
            passphrase: '',
        }, Buffer.from(data)));
    }

    return Buffer.concat(res)
}

function decrypt(buf) {
    let res = [];

    while (buf.length > 0)
    {
        let data = buf.slice(0, KEY_SIZE);
        buf = buf.slice(data.length);

        res.push(crypto.privateDecrypt({
            key,
            oaepHash: 'sha256',
            padding: crypto.constants.RSA_PKCS1_OAEP_PADDING,
            passphrase: '',
        }, Buffer.from(data)));
    }

    return Buffer.concat(res)
}

function sign(buf) {
    return crypto.sign(null, Buffer.from(buf), {key, passphrase: ''})
}
