//! Credential encryption service.
//!
//! Provides AES-256-GCM encryption/decryption for credential data,
//! matching n8n's implementation in @n8n/credentials package.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Size of the AES-256-GCM nonce (IV) in bytes.
const NONCE_SIZE: usize = 12;

/// Size of the AES-256 key in bytes.
const KEY_SIZE: usize = 32;

/// Errors that can occur during credential operations.
#[derive(Error, Debug)]
pub enum CredentialError {
    #[error("Encryption failed: {0}")]
    EncryptionError(String),

    #[error("Decryption failed: {0}")]
    DecryptionError(String),

    #[error("Invalid key: {0}")]
    InvalidKey(String),

    #[error("Invalid data format: {0}")]
    InvalidFormat(String),

    #[error("Base64 decode error: {0}")]
    Base64Error(#[from] base64::DecodeError),

    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Credential encryption service.
///
/// Uses AES-256-GCM for encryption/decryption of credential data.
/// Compatible with n8n's credential encryption format.
#[derive(Clone)]
pub struct CredentialService {
    /// Derived encryption key from the master key.
    key: [u8; KEY_SIZE],
}

impl CredentialService {
    /// Create a new credential service with the given encryption key.
    ///
    /// The key can be any string - it will be hashed using SHA-256
    /// to produce the actual encryption key.
    pub fn new(encryption_key: &str) -> Self {
        let key = derive_key(encryption_key);
        Self { key }
    }

    /// Create a credential service from a pre-derived key (32 bytes).
    pub fn from_key(key: [u8; KEY_SIZE]) -> Self {
        Self { key }
    }

    /// Encrypt credential data.
    ///
    /// Takes a JSON value and returns a base64-encoded encrypted string.
    /// Format: base64(nonce || ciphertext || tag)
    pub fn encrypt(&self, data: &serde_json::Value) -> Result<String, CredentialError> {
        let plaintext = serde_json::to_string(data)?;
        self.encrypt_string(&plaintext)
    }

    /// Encrypt a string directly.
    pub fn encrypt_string(&self, plaintext: &str) -> Result<String, CredentialError> {
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| CredentialError::InvalidKey(e.to_string()))?;

        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| CredentialError::EncryptionError(e.to_string()))?;

        // Combine nonce + ciphertext (ciphertext includes auth tag)
        let mut combined = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        Ok(BASE64.encode(&combined))
    }

    /// Decrypt credential data.
    ///
    /// Takes a base64-encoded encrypted string and returns the decrypted JSON value.
    pub fn decrypt(&self, encrypted: &str) -> Result<serde_json::Value, CredentialError> {
        let plaintext = self.decrypt_string(encrypted)?;
        Ok(serde_json::from_str(&plaintext)?)
    }

    /// Decrypt a string directly.
    pub fn decrypt_string(&self, encrypted: &str) -> Result<String, CredentialError> {
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| CredentialError::InvalidKey(e.to_string()))?;

        // Decode base64
        let combined = BASE64.decode(encrypted)?;

        if combined.len() < NONCE_SIZE {
            return Err(CredentialError::InvalidFormat(
                "Encrypted data too short".to_string(),
            ));
        }

        // Split nonce and ciphertext
        let (nonce_bytes, ciphertext) = combined.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| CredentialError::DecryptionError(e.to_string()))?;

        String::from_utf8(plaintext)
            .map_err(|e| CredentialError::DecryptionError(e.to_string()))
    }

    /// Re-encrypt credential data with a new key.
    ///
    /// This is useful for key rotation.
    pub fn re_encrypt(
        &self,
        encrypted: &str,
        new_key: &str,
    ) -> Result<String, CredentialError> {
        let decrypted = self.decrypt(encrypted)?;
        let new_service = CredentialService::new(new_key);
        new_service.encrypt(&decrypted)
    }
}

/// Derive an encryption key from a password/key string using SHA-256.
///
/// This matches n8n's key derivation for simple key hashing.
fn derive_key(key: &str) -> [u8; KEY_SIZE] {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let result = hasher.finalize();
    let mut key_bytes = [0u8; KEY_SIZE];
    key_bytes.copy_from_slice(&result);
    key_bytes
}

/// Derive an encryption key using PBKDF2-HMAC-SHA256.
///
/// This is more secure than simple hashing and matches n8n's
/// advanced key derivation option.
pub fn derive_key_pbkdf2(password: &str, salt: &[u8], iterations: u32) -> [u8; KEY_SIZE] {
    use pbkdf2::pbkdf2_hmac;

    let mut key = [0u8; KEY_SIZE];
    pbkdf2_hmac::<sha2::Sha256>(password.as_bytes(), salt, iterations, &mut key);
    key
}

/// Generate a random salt for PBKDF2 key derivation.
pub fn generate_salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

/// Decrypted credential data structure.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecryptedCredentialData {
    /// The actual credential values.
    #[serde(flatten)]
    pub data: serde_json::Value,
}

impl DecryptedCredentialData {
    pub fn new(data: serde_json::Value) -> Self {
        Self { data }
    }

    /// Get a string value from the credential data.
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.data.get(key).and_then(|v| v.as_str())
    }

    /// Get a number value from the credential data.
    pub fn get_number(&self, key: &str) -> Option<f64> {
        self.data.get(key).and_then(|v| v.as_f64())
    }

    /// Get a boolean value from the credential data.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.data.get(key).and_then(|v| v.as_bool())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let service = CredentialService::new("test-encryption-key-12345");

        let data = serde_json::json!({
            "apiKey": "sk-1234567890",
            "apiSecret": "secret-value"
        });

        let encrypted = service.encrypt(&data).unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();

        assert_eq!(data, decrypted);
    }

    #[test]
    fn test_encrypt_string() {
        let service = CredentialService::new("test-key");

        let plaintext = "Hello, World!";
        let encrypted = service.encrypt_string(plaintext).unwrap();
        let decrypted = service.decrypt_string(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_different_keys_fail() {
        let service1 = CredentialService::new("key1");
        let service2 = CredentialService::new("key2");

        let data = serde_json::json!({"secret": "value"});
        let encrypted = service1.encrypt(&data).unwrap();

        let result = service2.decrypt(&encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_pbkdf2_key_derivation() {
        let password = "my-secure-password";
        let salt = generate_salt();

        let key1 = derive_key_pbkdf2(password, &salt, 10000);
        let key2 = derive_key_pbkdf2(password, &salt, 10000);

        // Same password and salt should produce same key
        assert_eq!(key1, key2);

        // Different salt should produce different key
        let different_salt = generate_salt();
        let key3 = derive_key_pbkdf2(password, &different_salt, 10000);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_re_encrypt() {
        let old_service = CredentialService::new("old-key");
        let new_key = "new-key";

        let data = serde_json::json!({"token": "abc123"});
        let encrypted_old = old_service.encrypt(&data).unwrap();

        let encrypted_new = old_service.re_encrypt(&encrypted_old, new_key).unwrap();

        // Verify new key can decrypt
        let new_service = CredentialService::new(new_key);
        let decrypted = new_service.decrypt(&encrypted_new).unwrap();

        assert_eq!(data, decrypted);
    }
}
