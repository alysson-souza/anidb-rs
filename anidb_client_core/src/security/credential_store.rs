//! Credential storage trait and common types
//!
//! This module defines the common interface for credential storage
//! implementations across different platforms.

use crate::security::SecureString;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Errors that can occur during credential operations
#[derive(Debug, Error)]
pub enum CredentialStoreError {
    /// The requested credential was not found
    #[error("Credential not found: {0}")]
    NotFound(String),

    /// Failed to access the credential store
    #[error("Failed to access credential store: {0}")]
    AccessDenied(String),

    /// The credential data is corrupted or invalid
    #[error("Corrupted credential data: {0}")]
    CorruptedData(String),

    /// Platform-specific error
    #[error("Platform error: {0}")]
    PlatformError(String),

    /// Encryption or decryption failed
    #[error("Cryptographic error: {0}")]
    CryptoError(String),

    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// The operation is not supported on this platform
    #[error("Operation not supported on this platform")]
    NotSupported,
}

/// A credential entry containing account information
#[derive(Clone)]
pub struct Credential {
    /// Service identifier (e.g., "anidb")
    pub service: String,

    /// Account identifier (username)
    pub account: String,

    /// The secret data (password, API key, etc.)
    pub secret: SecureString,

    /// Additional metadata (optional)
    pub metadata: Option<CredentialMetadata>,
}

impl Credential {
    /// Create a new credential
    pub fn new(
        service: impl Into<String>,
        account: impl Into<String>,
        secret: impl Into<SecureString>,
    ) -> Self {
        Self {
            service: service.into(),
            account: account.into(),
            secret: secret.into(),
            metadata: None,
        }
    }

    /// Create a credential with metadata
    pub fn with_metadata(mut self, metadata: CredentialMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credential")
            .field("service", &self.service)
            .field("account", &self.account)
            .field("secret", &"***")
            .field("metadata", &self.metadata)
            .finish()
    }
}

/// Additional metadata for credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialMetadata {
    /// When the credential was created
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,

    /// When the credential was last modified
    pub modified_at: Option<chrono::DateTime<chrono::Utc>>,

    /// When the credential expires (if applicable)
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Additional tags or labels
    pub tags: Vec<String>,

    /// Custom key-value pairs
    pub custom: std::collections::HashMap<String, String>,
}

impl Default for CredentialMetadata {
    fn default() -> Self {
        Self {
            created_at: Some(chrono::Utc::now()),
            modified_at: Some(chrono::Utc::now()),
            expires_at: None,
            tags: Vec::new(),
            custom: std::collections::HashMap::new(),
        }
    }
}

/// Trait for credential storage implementations
#[async_trait]
pub trait CredentialStore: Send + Sync {
    /// Store a credential
    async fn store(&self, credential: &Credential) -> Result<(), CredentialStoreError>;

    /// Retrieve a credential by service and account
    async fn retrieve(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Credential, CredentialStoreError>;

    /// Delete a credential
    async fn delete(&self, service: &str, account: &str) -> Result<(), CredentialStoreError>;

    /// List all accounts for a service
    async fn list_accounts(&self, service: &str) -> Result<Vec<String>, CredentialStoreError>;

    /// List all stored services
    async fn list_services(&self) -> Result<Vec<String>, CredentialStoreError>;

    /// Check if a credential exists
    async fn exists(&self, service: &str, account: &str) -> Result<bool, CredentialStoreError> {
        match self.retrieve(service, account).await {
            Ok(_) => Ok(true),
            Err(CredentialStoreError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Update a credential (default implementation)
    async fn update(&self, credential: &Credential) -> Result<(), CredentialStoreError> {
        // Default implementation: delete and re-store
        self.delete(&credential.service, &credential.account)
            .await?;
        self.store(credential).await
    }

    /// Get the name of this credential store implementation
    fn name(&self) -> &str;

    /// Check if this store is available on the current platform
    async fn is_available(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_creation() {
        let cred = Credential::new("anidb", "testuser", "testpass");
        assert_eq!(cred.service, "anidb");
        assert_eq!(cred.account, "testuser");
        assert_eq!(cred.secret.as_str(), "testpass");
        assert!(cred.metadata.is_none());
    }

    #[test]
    fn test_credential_debug() {
        let cred = Credential::new("anidb", "testuser", "secret_password");
        let debug_str = format!("{cred:?}");
        assert!(debug_str.contains("anidb"));
        assert!(debug_str.contains("testuser"));
        assert!(!debug_str.contains("secret_password"));
        assert!(debug_str.contains("***"));
    }

    #[test]
    fn test_credential_with_metadata() {
        let metadata = CredentialMetadata::default();
        let cred = Credential::new("anidb", "testuser", "testpass").with_metadata(metadata.clone());

        assert!(cred.metadata.is_some());
        assert!(cred.metadata.unwrap().created_at.is_some());
    }

    #[test]
    fn test_error_display() {
        let err = CredentialStoreError::NotFound("test".to_string());
        assert_eq!(err.to_string(), "Credential not found: test");

        let err = CredentialStoreError::CryptoError("decryption failed".to_string());
        assert_eq!(err.to_string(), "Cryptographic error: decryption failed");
    }
}
