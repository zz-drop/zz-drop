use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::Aead,
};

use crate::profile::format::ProfileCryptoError;

pub(crate) const KEY_LEN: usize = 32;
pub(crate) const NONCE_LEN: usize = 24;
pub(crate) const SALT_LEN: usize = 16;

pub(crate) fn aead_encrypt(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    plaintext: &[u8],
) -> Result<Vec<u8>, ProfileCryptoError> {
    let cipher =
        XChaCha20Poly1305::new_from_slice(key).map_err(|_| ProfileCryptoError::Aead)?;
    let nonce = XNonce::from_slice(nonce);
    cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| ProfileCryptoError::Aead)
}

pub(crate) fn aead_decrypt(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    ciphertext: &[u8],
) -> Result<Vec<u8>, ProfileCryptoError> {
    let cipher =
        XChaCha20Poly1305::new_from_slice(key).map_err(|_| ProfileCryptoError::Aead)?;
    let nonce = XNonce::from_slice(nonce);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| ProfileCryptoError::Aead)
}
