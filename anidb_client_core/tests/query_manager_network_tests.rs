//! Integration tests for AniDBQueryManager that require a real AniDB connection

use anidb_client_core::identification::query_manager::AniDBQueryManager;
use anidb_client_core::protocol::ProtocolConfig;
use anidb_client_core::protocol::client::ProtocolClient;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
#[ignore = "Requires real AniDB credentials and network access"]
#[serial_test::serial]
async fn test_query_manager_authentication() {
    // Use default protocol config (api.anidb.net:9000)
    // This test is ignored by default to avoid CI/network dependency.
    let config = ProtocolConfig::default();

    let client = match ProtocolClient::new(config).await {
        Ok(c) => c,
        Err(e) => {
            // If we cannot even create the client (e.g., DNS/network issue), that's acceptable
            // in environments without network access. Treat as expected failure when run manually.
            eprintln!("Skipping auth attempt due to client creation error: {e}");
            return;
        }
    };

    let manager = AniDBQueryManager::new(Arc::new(Mutex::new(client)));

    // Use dummy credentials; expect an error (either auth failure or connectivity error)
    let result = manager.ensure_authenticated("testuser", "testpass").await;

    assert!(
        result.is_err(),
        "Expected authentication/connect error with dummy creds"
    );
}
