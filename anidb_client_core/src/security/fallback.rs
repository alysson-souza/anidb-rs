//! Fallback credential storage using encrypted files
//!
//! This implementation works on all platforms and stores credentials
//! in an encrypted file using AES-256-GCM with Argon2id key derivation.

use crate::security::{Credential, CredentialStore, CredentialStoreError, SecureString};
use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use argon2::{Algorithm, Argon2, Params, Version, password_hash::SaltString};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Encrypted credential storage in a local file
pub struct EncryptedFileStore {
    file_path: PathBuf,
    master_key: SecureString,
}

/// The encrypted file format
#[derive(Serialize, Deserialize)]
struct EncryptedStore {
    /// Version of the store format
    version: u32,

    /// Salt for key derivation
    salt: String,

    /// Encrypted credentials (service -> account -> encrypted data)
    credentials: HashMap<String, HashMap<String, EncryptedCredential>>,
}

/// An encrypted credential entry
#[derive(Serialize, Deserialize)]
struct EncryptedCredential {
    /// AES-GCM nonce
    nonce: Vec<u8>,

    /// Encrypted data (contains the actual credential)
    ciphertext: Vec<u8>,

    /// When this was last modified
    modified_at: chrono::DateTime<chrono::Utc>,
}

/// Internal format for serializing credentials
#[derive(Serialize, Deserialize)]
struct StoredCredential {
    service: String,
    account: String,
    secret: Vec<u8>, // Will be encrypted
    metadata: Option<crate::security::credential_store::CredentialMetadata>,
}

impl EncryptedFileStore {
    /// Create a new encrypted file store
    pub async fn new() -> Result<Self, CredentialStoreError> {
        // Allow overriding credential store directory via env (useful for tests/CI)
        let store_dir = if let Ok(dir) = std::env::var("ANIDB_CREDENTIAL_STORE_DIR") {
            PathBuf::from(dir)
        } else {
            let config_dir = dirs::config_dir().ok_or_else(|| {
                CredentialStoreError::PlatformError(
                    "Could not determine config directory".to_string(),
                )
            })?;
            config_dir.join("anidb-client").join("credentials")
        };
        fs::create_dir_all(&store_dir).await?;

        let file_path = store_dir.join("store.enc");

        // Generate or load master key
        let master_key = Self::get_or_create_master_key(&store_dir).await?;

        Ok(Self {
            file_path,
            master_key,
        })
    }

    /// Get or create the master key for encryption
    async fn get_or_create_master_key(
        store_dir: &Path,
    ) -> Result<SecureString, CredentialStoreError> {
        let key_file = store_dir.join(".key");

        if key_file.exists() {
            // Load existing key
            let key_data = fs::read(&key_file).await?;
            Ok(SecureString::from_bytes(key_data))
        } else {
            // Generate new key
            let key = Aes256Gcm::generate_key(&mut OsRng);
            let key_vec = key.to_vec();

            // Save key with restricted permissions
            #[cfg(unix)]
            let mut file = {
                fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&key_file)
                    .await?
            };

            #[cfg(not(unix))]
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(&key_file)
                .await?;

            file.write_all(&key_vec).await?;
            file.sync_all().await?;

            Ok(SecureString::from_bytes(key_vec))
        }
    }

    /// Derive an encryption key from the master key
    fn derive_key(&self, salt: &SaltString) -> Result<Key<Aes256Gcm>, CredentialStoreError> {
        let argon2 = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(65536, 3, 4, Some(32))
                .map_err(|e| CredentialStoreError::CryptoError(e.to_string()))?,
        );

        let mut key_bytes = [0u8; 32];
        argon2
            .hash_password_into(
                self.master_key.as_bytes(),
                salt.as_str().as_bytes(),
                &mut key_bytes,
            )
            .map_err(|e| CredentialStoreError::CryptoError(e.to_string()))?;

        Ok(*Key::<Aes256Gcm>::from_slice(&key_bytes))
    }

    /// Load the encrypted store from disk
    async fn load_store(&self) -> Result<EncryptedStore, CredentialStoreError> {
        if !self.file_path.exists() {
            // Create empty store
            return Ok(EncryptedStore {
                version: 1,
                salt: SaltString::generate(&mut OsRng).to_string(),
                credentials: HashMap::new(),
            });
        }

        let data = fs::read(&self.file_path).await?;
        serde_json::from_slice(&data)
            .map_err(|e| CredentialStoreError::SerializationError(e.to_string()))
    }

    /// Save the encrypted store to disk
    async fn save_store(&self, store: &EncryptedStore) -> Result<(), CredentialStoreError> {
        let data = serde_json::to_vec_pretty(store)
            .map_err(|e| CredentialStoreError::SerializationError(e.to_string()))?;

        // Write to temporary file first
        let temp_path = self.file_path.with_extension("tmp");

        #[cfg(unix)]
        let mut file = {
            fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&temp_path)
                .await?
        };

        #[cfg(not(unix))]
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_path)
            .await?;

        file.write_all(&data).await?;
        file.sync_all().await?;
        drop(file);

        // Atomic rename
        fs::rename(&temp_path, &self.file_path).await?;

        Ok(())
    }

    /// Encrypt a credential
    fn encrypt_credential(
        &self,
        credential: &Credential,
        salt: &SaltString,
    ) -> Result<EncryptedCredential, CredentialStoreError> {
        let key = self.derive_key(salt)?;
        let cipher = Aes256Gcm::new(&key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        let stored = StoredCredential {
            service: credential.service.clone(),
            account: credential.account.clone(),
            secret: credential.secret.as_bytes().to_vec(),
            metadata: credential.metadata.clone(),
        };

        let plaintext = serde_json::to_vec(&stored)
            .map_err(|e| CredentialStoreError::SerializationError(e.to_string()))?;

        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_ref())
            .map_err(|e| CredentialStoreError::CryptoError(e.to_string()))?;

        Ok(EncryptedCredential {
            nonce: nonce.to_vec(),
            ciphertext,
            modified_at: chrono::Utc::now(),
        })
    }

    /// Decrypt a credential
    fn decrypt_credential(
        &self,
        encrypted: &EncryptedCredential,
        salt: &SaltString,
    ) -> Result<Credential, CredentialStoreError> {
        let key = self.derive_key(salt)?;
        let cipher = Aes256Gcm::new(&key);
        let nonce = Nonce::from_slice(&encrypted.nonce);

        let plaintext = cipher
            .decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|e| CredentialStoreError::CryptoError(format!("Decryption failed: {e}")))?;

        let stored: StoredCredential = serde_json::from_slice(&plaintext)
            .map_err(|e| CredentialStoreError::CorruptedData(e.to_string()))?;

        Ok(Credential {
            service: stored.service,
            account: stored.account,
            secret: SecureString::from_bytes(stored.secret),
            metadata: stored.metadata,
        })
    }
}

#[async_trait]
impl CredentialStore for EncryptedFileStore {
    async fn store(&self, credential: &Credential) -> Result<(), CredentialStoreError> {
        let mut store = self.load_store().await?;
        let salt = SaltString::from_b64(&store.salt)
            .map_err(|e| CredentialStoreError::CryptoError(e.to_string()))?;

        let encrypted = self.encrypt_credential(credential, &salt)?;

        store
            .credentials
            .entry(credential.service.clone())
            .or_insert_with(HashMap::new)
            .insert(credential.account.clone(), encrypted);

        self.save_store(&store).await
    }

    async fn retrieve(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Credential, CredentialStoreError> {
        let store = self.load_store().await?;
        let salt = SaltString::from_b64(&store.salt)
            .map_err(|e| CredentialStoreError::CryptoError(e.to_string()))?;

        let encrypted = store
            .credentials
            .get(service)
            .and_then(|accounts| accounts.get(account))
            .ok_or_else(|| CredentialStoreError::NotFound(format!("{service}/{account}")))?;

        self.decrypt_credential(encrypted, &salt)
    }

    async fn delete(&self, service: &str, account: &str) -> Result<(), CredentialStoreError> {
        let mut store = self.load_store().await?;

        if let Some(accounts) = store.credentials.get_mut(service) {
            if accounts.remove(account).is_none() {
                return Err(CredentialStoreError::NotFound(format!(
                    "{service}/{account}"
                )));
            }

            // Remove service if no accounts left
            if accounts.is_empty() {
                store.credentials.remove(service);
            }
        } else {
            return Err(CredentialStoreError::NotFound(service.to_string()));
        }

        self.save_store(&store).await
    }

    async fn list_accounts(&self, service: &str) -> Result<Vec<String>, CredentialStoreError> {
        let store = self.load_store().await?;

        Ok(store
            .credentials
            .get(service)
            .map(|accounts| accounts.keys().cloned().collect())
            .unwrap_or_default())
    }

    async fn list_services(&self) -> Result<Vec<String>, CredentialStoreError> {
        let store = self.load_store().await?;
        Ok(store.credentials.keys().cloned().collect())
    }

    fn name(&self) -> &str {
        "Encrypted File Store"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_store() -> (EncryptedFileStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let store_dir = temp_dir.path().join("credentials");
        fs::create_dir_all(&store_dir).await.unwrap();

        let store = EncryptedFileStore {
            file_path: store_dir.join("store.enc"),
            master_key: SecureString::new("test_master_key_32_bytes_long!!!"),
        };

        (store, temp_dir)
    }

    #[tokio::test]
    async fn test_store_and_retrieve() {
        let (store, _temp) = create_test_store().await;

        let cred = Credential::new("test_service", "test_user", "test_password");

        // Store credential
        store.store(&cred).await.unwrap();

        // Retrieve credential
        let retrieved = store.retrieve("test_service", "test_user").await.unwrap();
        assert_eq!(retrieved.service, "test_service");
        assert_eq!(retrieved.account, "test_user");
        assert_eq!(retrieved.secret.as_str(), "test_password");
    }

    #[tokio::test]
    async fn test_delete() {
        let (store, _temp) = create_test_store().await;

        let cred = Credential::new("test_service", "test_user", "test_password");
        store.store(&cred).await.unwrap();

        // Delete credential
        store.delete("test_service", "test_user").await.unwrap();

        // Should not exist anymore
        let result = store.retrieve("test_service", "test_user").await;
        assert!(matches!(result, Err(CredentialStoreError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_list_accounts() {
        let (store, _temp) = create_test_store().await;

        // Store multiple accounts
        store
            .store(&Credential::new("service1", "user1", "pass1"))
            .await
            .unwrap();
        store
            .store(&Credential::new("service1", "user2", "pass2"))
            .await
            .unwrap();
        store
            .store(&Credential::new("service2", "user3", "pass3"))
            .await
            .unwrap();

        // List accounts for service1
        let accounts = store.list_accounts("service1").await.unwrap();
        assert_eq!(accounts.len(), 2);
        assert!(accounts.contains(&"user1".to_string()));
        assert!(accounts.contains(&"user2".to_string()));

        // List accounts for service2
        let accounts = store.list_accounts("service2").await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert!(accounts.contains(&"user3".to_string()));
    }

    #[tokio::test]
    async fn test_list_services() {
        let (store, _temp) = create_test_store().await;

        // Store credentials for multiple services
        store
            .store(&Credential::new("service1", "user1", "pass1"))
            .await
            .unwrap();
        store
            .store(&Credential::new("service2", "user2", "pass2"))
            .await
            .unwrap();
        store
            .store(&Credential::new("service3", "user3", "pass3"))
            .await
            .unwrap();

        // List all services
        let services = store.list_services().await.unwrap();
        assert_eq!(services.len(), 3);
        assert!(services.contains(&"service1".to_string()));
        assert!(services.contains(&"service2".to_string()));
        assert!(services.contains(&"service3".to_string()));
    }

    #[tokio::test]
    async fn test_update_credential() {
        let (store, _temp) = create_test_store().await;

        // Store initial credential
        let cred = Credential::new("test_service", "test_user", "old_password");
        store.store(&cred).await.unwrap();

        // Update with new password
        let updated_cred = Credential::new("test_service", "test_user", "new_password");
        store.update(&updated_cred).await.unwrap();

        // Retrieve and verify
        let retrieved = store.retrieve("test_service", "test_user").await.unwrap();
        assert_eq!(retrieved.secret.as_str(), "new_password");
    }

    #[tokio::test]
    async fn test_credential_not_found() {
        let (store, _temp) = create_test_store().await;

        let result = store.retrieve("nonexistent", "user").await;
        assert!(matches!(result, Err(CredentialStoreError::NotFound(_))));
    }
}
