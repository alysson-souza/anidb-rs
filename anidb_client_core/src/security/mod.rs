//! Security module for credential management and protection
//!
//! This module provides secure credential storage with platform-specific
//! implementations and memory protection features.

pub mod credential_store;
pub mod secure_string;

// Platform-specific implementations will be added in future sprints
// For now, we use the fallback implementation on all platforms

// Fallback implementation for all platforms
pub mod fallback;

// Re-export main types
pub use credential_store::{Credential, CredentialStore, CredentialStoreError};
pub use secure_string::SecureString;

// For Sprint 1, we use the fallback implementation on all platforms
pub type DefaultCredentialStore = fallback::EncryptedFileStore;

/// Create a new credential store instance
///
/// This function returns the appropriate credential store for the current platform.
/// Currently uses the encrypted file store on all platforms.
pub async fn create_credential_store() -> Result<Box<dyn CredentialStore>, CredentialStoreError> {
    // For Sprint 1, we use the encrypted file store on all platforms
    fallback::EncryptedFileStore::new()
        .await
        .map(|store| Box::new(store) as Box<dyn CredentialStore>)
}
