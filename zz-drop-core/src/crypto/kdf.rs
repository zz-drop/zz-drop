use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::profile::format::ProfileCryptoError;

use super::aead::KEY_LEN;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Argon2idConfig {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Argon2idConfig {
    pub const DEFAULT: Self = Self {
        memory_kib: 194_560,
        iterations: 3,
        parallelism: 1,
    };
}

impl Default for Argon2idConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

pub(crate) fn derive_key(
    passphrase: &str,
    salt: &[u8],
    config: &Argon2idConfig,
) -> Result<Zeroizing<[u8; KEY_LEN]>, ProfileCryptoError> {
    let params = Params::new(
        config.memory_kib,
        config.iterations,
        config.parallelism,
        Some(KEY_LEN),
    )
    .map_err(|e| ProfileCryptoError::Kdf(e.to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, key.as_mut())
        .map_err(|_| ProfileCryptoError::Kdf("hash_password_into failed".into()))?;

    Ok(key)
}
