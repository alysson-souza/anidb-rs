//! Tests for identification authentication handling

use anidb_client_core::ClientConfig;
use anidb_client_core::identification::{
    FileIdentificationService, IdentificationOptions, IdentificationService, ServiceConfig,
};
use tempfile::TempDir;

#[tokio::test]
#[serial_test::serial]
async fn test_identify_without_credentials_returns_helpful_error() {
    let temp_dir = TempDir::new().unwrap();
    // Isolate credential store to avoid using real user credentials
    let cred_dir = temp_dir.path().join("creds1");
    let cred_dir_str = cred_dir.to_string_lossy().to_string();
    unsafe {
        std::env::set_var("ANIDB_CREDENTIAL_STORE_DIR", cred_dir_str);
    }

    let client_config = ClientConfig::test();

    let service = FileIdentificationService::new(client_config, ServiceConfig::default())
        .await
        .unwrap();

    // Try to identify without credentials - should fail with helpful error
    // Note: We're not setting offline_mode here because we want to test the credential check
    let options = IdentificationOptions {
        timeout: std::time::Duration::from_millis(100), // Short timeout to fail fast
        ..Default::default()
    };

    let result = service.identify_hash("test_hash", 1000000, options).await;

    match result {
        Err(err) => {
            // Check that the error is about missing credentials
            let error_str = err.to_string();
            assert!(
                error_str.contains("credential")
                    || error_str.contains("auth")
                    || error_str.contains("login"),
                "Expected error about missing credentials, got: {error_str}"
            );
        }
        Ok(_) => panic!("Expected error when no credentials exist"),
    }
}

#[tokio::test]
#[serial_test::serial]
async fn test_identify_with_offline_mode_works_without_credentials() {
    let temp_dir = TempDir::new().unwrap();
    // Isolate credential store to avoid using real user credentials
    let cred_dir = temp_dir.path().join("creds2");
    let cred_dir_str2 = cred_dir.to_string_lossy().to_string();
    unsafe {
        std::env::set_var("ANIDB_CREDENTIAL_STORE_DIR", cred_dir_str2);
    }

    let client_config = ClientConfig::test();

    let service = FileIdentificationService::new(client_config, ServiceConfig::default())
        .await
        .unwrap();

    let options = IdentificationOptions {
        offline_mode: true,
        ..Default::default()
    };

    // In offline mode, should work without credentials
    let result = service.identify_hash("test_hash", 1000000, options).await;

    assert!(
        result.is_ok(),
        "Offline mode should work without credentials"
    );
}
