use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use shared::utilities::errors::AppError;

pub struct EncryptionService {
    cipher: Aes256Gcm,
}

impl EncryptionService {
    /// Create new encryption service from a base64-encoded 32-byte key
    pub fn new(key_base64: &str) -> Result<Self, AppError> {
        let key_bytes = BASE64
            .decode(key_base64)
            .map_err(|e| AppError::InvalidKey(e.to_string()))?;

        if key_bytes.len() != 32 {
            return Err(AppError::InvalidKey("Key must be 32 bytes".to_string()));
        }

        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| AppError::InvalidKey(e.to_string()))?;

        Ok(Self { cipher })
    }

    /// Generate a new random encryption key (base64-encoded)
    pub fn generate_key() -> String {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        BASE64.encode(key)
    }

    /// Encrypt data and return bytes (nonce + ciphertext)
    pub fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>, AppError> {
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        #[allow(deprecated)]
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| AppError::EncryptionError(e.to_string()))?;

        // Store nonce + ciphertext together
        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt data from bytes (nonce + ciphertext)
    pub fn decrypt(&self, encrypted_data: &[u8]) -> Result<String, AppError> {
        if encrypted_data.len() < 12 {
            return Err(AppError::InvalidFormat);
        }

        let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
        #[allow(deprecated)]
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| AppError::DecryptionError(e.to_string()))?;

        String::from_utf8(plaintext).map_err(|e| AppError::FromUtf8Error(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_decryption() {
        let key = EncryptionService::generate_key();
        let service = EncryptionService::new(&key).unwrap();

        let original = "my-secret-password";
        let encrypted = service.encrypt(original).unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();

        assert_eq!(original, decrypted);
    }
}
