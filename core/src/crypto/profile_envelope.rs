use serde::{Deserialize, Serialize};

pub const ENVELOPE_VERSION_V1: u32 = 1;
pub const KDF_NAME_ARGON2ID: &str = "argon2id";
pub const CIPHER_NAME_XCHACHA20POLY1305: &str = "xchacha20poly1305";
pub const PAYLOAD_FORMAT_CBOR: &str = "cbor";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileEnvelope {
    pub version: u32,
    pub kdf: KdfParams,
    pub cipher: CipherParams,
    pub payload: PayloadParams,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KdfParams {
    pub name: String,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub salt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CipherParams {
    pub name: String,
    pub nonce: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PayloadParams {
    pub format: String,
    pub ciphertext: String,
}
