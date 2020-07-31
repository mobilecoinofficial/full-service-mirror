// Copyright (c) 2018-2020 MobileCoin Inc.

//! Cryptographic primitives.

use digest::Digest;
use generic_array::typenum::Unsigned;
use rsa::{PaddingScheme, PublicKey, PublicKeyParts, RSAPublicKey};
use sha2::Sha256;

/// Encrypt a payload of arbitrary length.
pub fn encrypt(key: &RSAPublicKey, payload: &[u8]) -> Result<Vec<u8>, String> {
    // Each encrypted chunk must be no longer than the length of the public modulus minus (2 + 2*hash.size()).
    // (Taken from `rsa::oaep::encrypt`).
    let key_size = key.size();
    let hash_size = <Sha256 as Digest>::OutputSize::to_usize();
    let max_chunk_size = key_size - (2 * hash_size + 2);

    let mut rng = rand::thread_rng();
    let chunks: Vec<Vec<u8>> = payload
        .chunks(max_chunk_size)
        .map(|chunk| {
            key.encrypt(&mut rng, PaddingScheme::new_oaep::<Sha256>(), chunk)
                .map_err(|err| format!("encrypt failed: {:?}", err))
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(chunks
        .into_iter()
        .flat_map(|chunk| chunk.into_iter())
        .collect())
}
