//! Tests for protocol rate limiting behavior
//!
//! These tests specifically focus on the rate limiting behavior between
//! consecutive API calls, particularly the FILE -> ANIME query sequence.

use anidb_client_core::protocol::ProtocolConfig;
use anidb_client_core::protocol::client::ProtocolClient;
use anidb_client_core::protocol::messages::Command;
use std::time::{Duration, Instant};
use tokio::time::timeout;

#[tokio::test]
#[ignore] // Requires real AniDB connection and credentials
async fn test_consecutive_queries_with_rate_limiting() {
    // This test verifies that consecutive API calls respect the 2-second rate limit
    let config = ProtocolConfig::default();
    let client = ProtocolClient::new(config).await.unwrap();

    // Connect and authenticate
    client.connect().await.unwrap();
    let _session = client
        .authenticate("testuser".to_string(), "testpass".to_string())
        .await
        .unwrap();

    // Send FILE command
    let file_cmd = Command::file()
        .by_hash(175244080, "9CAA4E5CC4CAB8C0A5E6C5CF5056DBD8") // Test file
        .build()
        .unwrap();

    let start = Instant::now();
    let _file_response = client.send_command(file_cmd).await.unwrap();
    let _file_time = start.elapsed();

    // Send ANIME command - this should wait ~2 seconds due to rate limiting
    let anime_cmd = Command::anime(5975); // Test anime ID

    let start = Instant::now();
    let anime_response = timeout(Duration::from_secs(10), client.send_command(anime_cmd)).await;
    let anime_time = start.elapsed();

    // Verify the response was received (not hanging)
    assert!(anime_response.is_ok(), "ANIME query should not timeout");

    // Verify rate limiting was applied (should take approximately 2 seconds)
    assert!(
        anime_time >= Duration::from_secs(1),
        "Rate limiting should add delay"
    );
    assert!(
        anime_time < Duration::from_secs(5),
        "Should not take too long"
    );

    // Clean up
    client.logout().await.unwrap();
}

#[tokio::test]
async fn test_rate_limiter_timing() {
    // Test the rate limiter behavior by checking command creation timing
    use anidb_client_core::protocol::messages::Command;

    // First command should be created immediately
    let start = Instant::now();
    let _file_cmd = Command::file()
        .by_hash(175244080, "9CAA4E5CC4CAB8C0A5E6C5CF5056DBD8")
        .build()
        .unwrap();
    let first_elapsed = start.elapsed();
    assert!(
        first_elapsed < Duration::from_millis(100),
        "First command creation should be immediate"
    );

    // Second command should also be created immediately (rate limiting happens in send_command)
    let start = Instant::now();
    let _anime_cmd = Command::anime(5975);
    let second_elapsed = start.elapsed();
    assert!(
        second_elapsed < Duration::from_millis(100),
        "Second command creation should be immediate"
    );
}

#[tokio::test]
async fn test_anime_command_creation() {
    // Test that ANIME commands are created correctly
    let anime_cmd = Command::anime(5975);

    // Verify command properties
    assert_eq!(anime_cmd.name(), "ANIME");
    assert!(anime_cmd.requires_auth());

    // Test encoding
    let encoded = anime_cmd.encode().unwrap();
    assert!(encoded.contains("ANIME"));
    assert!(encoded.contains("aid=5975"));
}

#[tokio::test]
async fn test_file_to_anime_query_sequence() {
    // Mock test for the FILE -> ANIME sequence without network calls
    use anidb_client_core::protocol::messages::Command;

    // Create FILE command
    let file_cmd = Command::file()
        .by_hash(175244080, "9CAA4E5CC4CAB8C0A5E6C5CF5056DBD8")
        .build()
        .unwrap();

    assert_eq!(file_cmd.name(), "FILE");
    assert!(file_cmd.requires_auth());

    // Create ANIME command
    let anime_cmd = Command::anime(5975);

    assert_eq!(anime_cmd.name(), "ANIME");
    assert!(anime_cmd.requires_auth());

    // Both commands should encode properly
    let file_encoded = file_cmd.encode().unwrap();
    let anime_encoded = anime_cmd.encode().unwrap();

    assert!(file_encoded.contains("FILE"));
    assert!(anime_encoded.contains("ANIME"));
}

#[tokio::test]
async fn test_query_manager_no_deadlock() {
    // Test that the query manager doesn't deadlock when making multiple queries
    use anidb_client_core::identification::query_manager::AniDBQueryManager;
    use anidb_client_core::protocol::ProtocolConfig;
    use anidb_client_core::protocol::client::ProtocolClient;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let config = ProtocolConfig {
        server: "127.0.0.1:9999".to_string(), // Non-existent server
        ..Default::default()
    };

    // Create a protocol client (will fail to connect, but that's fine for this test)
    let protocol_client = ProtocolClient::new(config).await.unwrap();
    let manager = AniDBQueryManager::new(Arc::new(Mutex::new(protocol_client)));

    // These should not hang or deadlock (they will fail with network errors, but that's expected)
    let anime_future = manager.query_anime(5975);
    let episode_future = manager.query_episode(123);
    let group_future = manager.query_group(456);

    // Use timeout to ensure the methods don't hang indefinitely. The timeout must account for
    // the enforced protocol rate limit. We derive it from the current
    // RATE_LIMIT_REQUESTS_PER_SECOND constant plus a safety margin so that changes to
    // the rate limit do not cause spurious test failures.
    let per_request_ms =
        (1000.0 / anidb_client_core::protocol::RATE_LIMIT_REQUESTS_PER_SECOND).ceil() as u64;
    // Allow one full rate-limit interval + 1s margin.
    let timeout_duration = Duration::from_millis(per_request_ms + 1000);

    let anime_result = tokio::time::timeout(timeout_duration, anime_future).await;
    let episode_result = tokio::time::timeout(timeout_duration, episode_future).await;
    let group_result = tokio::time::timeout(timeout_duration, group_future).await;

    // All should complete (with errors) rather than timing out
    assert!(
        anime_result.is_ok(),
        "query_anime should not hang (rate-limit aware timeout)"
    );
    assert!(
        episode_result.is_ok(),
        "query_episode should not hang (rate-limit aware timeout)"
    );
    assert!(
        group_result.is_ok(),
        "query_group should not hang (rate-limit aware timeout)"
    );
}
