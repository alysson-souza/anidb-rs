//! Example tests demonstrating the enhanced test utilities
//!
//! This file shows how to use the improved builder patterns to create
//! more readable and maintainable tests.
//!
//! To run these tests, use:
//! cargo test -p anidb_client_core --features test-utils test_utils_example_tests

#[cfg(feature = "test-utils")]
mod enhanced_test_examples {
    use anidb_client_core::test_utils::*;
    use anidb_client_core::{Error, HashAlgorithm};
    use std::time::Duration;

    #[tokio::test]
    async fn test_with_preset_configurations() {
        // Using preset configurations for common test cases
        let small_file = TestDataBuilder::small_file().build();
        assert!(small_file.file_size < 1024 * 1024);
        assert!(
            small_file
                .expected_hashes
                .contains_key(&HashAlgorithm::ED2K)
        );

        let episode = TestDataBuilder::anime_episode("One Piece", 1000).build();
        assert_eq!(episode.anime_title, "One Piece");
        assert_eq!(episode.episode_number, 1000);

        // ED2K boundary testing made simple
        let boundary_file = TestDataBuilder::ed2k_boundary_file().build();
        assert_eq!(boundary_file.file_size, 9728000);
    }

    #[tokio::test]
    async fn test_auto_generated_hashes() {
        // Auto-generate deterministic hashes
        let file = TestDataBuilder::new()
            .with_anime_title("Attack on Titan")
            .with_episode_number(87)
            .with_auto_hashes() // Generates deterministic hashes
            .build();

        // Hashes are automatically generated based on file properties
        assert!(file.expected_hashes.contains_key(&HashAlgorithm::ED2K));
        assert!(file.expected_hashes.contains_key(&HashAlgorithm::CRC32));
        assert!(file.expected_hashes.contains_key(&HashAlgorithm::MD5));
        assert!(file.expected_hashes.contains_key(&HashAlgorithm::SHA1));
    }

    #[tokio::test]
    async fn test_batch_processing_scenario() {
        // Create a complete season batch
        let scenario = ScenarioBuilder::batch_processing("Demon Slayer", 26).build();

        assert_eq!(scenario.files.len(), 26);
        for (i, file) in scenario.files.iter().enumerate() {
            assert_eq!(file.anime_title, "Demon Slayer");
            assert_eq!(file.episode_number, (i + 1) as u32);
        }
    }

    #[tokio::test]
    async fn test_error_recovery_scenario() {
        // Test error recovery with planned failures
        let scenario = ScenarioBuilder::new("custom_error_test")
            .add_file(TestDataBuilder::small_file().build())
            .add_error_at_file(1, ErrorType::NetworkTimeout)
            .add_file(TestDataBuilder::small_file().build())
            .add_error_at_file(2, ErrorType::FileNotFound)
            .add_file(TestDataBuilder::small_file().build())
            .build();

        assert_eq!(scenario.files.len(), 3);
        assert_eq!(scenario.errors.len(), 2);

        // Verify error injection points
        assert_eq!(scenario.errors[0].at_file_index, Some(1));
        assert!(matches!(
            scenario.errors[0].error_type,
            ErrorType::NetworkTimeout
        ));
    }

    #[tokio::test]
    async fn test_performance_scenario() {
        // Performance test with various file sizes
        let scenario = ScenarioBuilder::performance_test().build();

        assert_eq!(scenario.files.len(), 5);

        // Verify progressive file sizes
        assert_eq!(scenario.files[0].file_size, 1024); // 1KB
        assert_eq!(scenario.files[1].file_size, 1024 * 1024); // 1MB
        assert_eq!(scenario.files[2].file_size, 10 * 1024 * 1024); // 10MB
        assert_eq!(scenario.files[3].file_size, 100 * 1024 * 1024); // 100MB
        assert_eq!(scenario.files[4].file_size, 1024 * 1024 * 1024); // 1GB
    }

    #[tokio::test]
    async fn test_network_issues_scenario() {
        // Simulate network problems
        let scenario = ScenarioBuilder::network_issues().build();

        assert_eq!(scenario.network_conditions.latency, Duration::from_secs(2));
        assert_eq!(scenario.network_conditions.packet_loss, 0.1);
        assert!(!scenario.network_conditions.offline);

        // Has delays configured
        assert!(!scenario.delays.is_empty());
    }

    #[tokio::test]
    async fn test_mock_client_with_standard_responses() {
        let client = MockClientBuilder::new()
            .with_standard_responses()
            .with_latency(Duration::from_millis(100))
            .build();

        // Test AUTH command
        let response = client.send_command("AUTH").await.unwrap();
        assert_eq!(response.code, 200);
        assert!(response.data.contains("LOGIN ACCEPTED"));

        // Test FILE command
        let response = client.send_command("FILE").await.unwrap();
        assert_eq!(response.code, 220);
        assert!(response.data.contains("Test Anime"));
    }

    #[tokio::test]
    async fn test_mock_client_offline_mode() {
        let client = MockClientBuilder::new().offline_mode().build();

        // All commands should fail when offline
        let result = client.send_command("AUTH").await;
        assert!(result.is_err());

        if let Err(e) = result {
            // Check if it's the right error type
            assert!(matches!(e, Error::Protocol(_)));
        }
    }

    #[tokio::test]
    async fn test_fixture_generator_anime_season() {
        let mut generator = FixtureGenerator::new();

        // Generate a complete anime season
        let season = generator.generate_anime_season("My Hero Academia");

        assert_eq!(season.len(), 12);

        for (i, episode) in season.iter().enumerate() {
            assert_eq!(episode.anime_title, "My Hero Academia");
            assert_eq!(episode.episode_number, (i + 1) as u32);
            // Each episode has slightly different size
            assert!(episode.file_size > 350 * 1024 * 1024);
        }
    }

    #[tokio::test]
    async fn test_fixture_generator_content_patterns() {
        let generator = FixtureGenerator::new();

        // Test different content patterns
        let zeros = generator.generate_file_content(100, ContentPattern::Zeros);
        assert_eq!(zeros.len(), 100);
        assert!(zeros.iter().all(|&b| b == 0));

        let repeating = generator.generate_file_content(100, ContentPattern::Repeating(0x42));
        assert!(repeating.iter().all(|&b| b == 0x42));

        let gradient = generator.generate_file_content(256, ContentPattern::Gradient);
        for (i, &byte) in gradient.iter().enumerate() {
            assert_eq!(byte, i as u8);
        }

        // Real video pattern has headers
        let video = generator.generate_file_content(10000, ContentPattern::RealVideo);
        assert_eq!(&video[0..4], b"ftyp");
        assert_eq!(&video[4..8], b"isom");
    }

    #[tokio::test]
    async fn test_create_actual_files() {
        let mut generator = FixtureGenerator::new();

        // Create test data
        let files = vec![
            TestDataBuilder::small_file()
                .with_anime_title("Test1")
                .build(),
            TestDataBuilder::small_file()
                .with_anime_title("Test2")
                .build(),
        ];

        // Create actual files
        let paths = generator.create_test_files(&files).unwrap();

        assert_eq!(paths.len(), 2);

        // Verify files exist
        for path in &paths {
            assert!(path.exists());

            let metadata = std::fs::metadata(path).unwrap();
            assert!(metadata.len() > 0);
        }

        // Cleanup happens automatically when generator is dropped
    }

    #[tokio::test]
    async fn test_complex_scenario_execution() {
        // Create a complex scenario
        let scenario = ScenarioBuilder::new("complex_test")
            .add_file(TestDataBuilder::small_file().build())
            .add_delay_at_file(0, Duration::from_millis(100))
            .add_file(TestDataBuilder::large_file().build())
            .add_error_at_file(2, ErrorType::NetworkTimeout)
            .add_file(TestDataBuilder::ed2k_boundary_file().build())
            .with_network_latency(Duration::from_millis(50))
            .build();

        // Create mock client
        let mock_client = MockClientBuilder::new().with_standard_responses().build();

        // Run the scenario
        let results = run_scenario_with_mock(scenario, mock_client).await;

        assert_eq!(results.len(), 3);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
        // Note: The third file would have an error due to our scenario configuration
    }

    #[test]
    fn test_deterministic_hash_generation() {
        // Test that hashes are deterministic
        let file1 = TestDataBuilder::new()
            .with_anime_title("Test")
            .with_episode_number(1)
            .with_file_size(1000)
            .with_auto_hashes()
            .build();

        let file2 = TestDataBuilder::new()
            .with_anime_title("Test")
            .with_episode_number(1)
            .with_file_size(1000)
            .with_auto_hashes()
            .build();

        // Same properties should generate same hashes
        assert_eq!(
            file1.expected_hashes.get(&HashAlgorithm::ED2K),
            file2.expected_hashes.get(&HashAlgorithm::ED2K)
        );

        // Different properties should generate different hashes
        let file3 = TestDataBuilder::new()
            .with_anime_title("Different")
            .with_episode_number(1)
            .with_file_size(1000)
            .with_auto_hashes()
            .build();

        assert_ne!(
            file1.expected_hashes.get(&HashAlgorithm::ED2K),
            file3.expected_hashes.get(&HashAlgorithm::ED2K)
        );
    }

    #[test]
    fn test_builder_method_chaining() {
        // Demonstrate clean method chaining
        let file = TestDataBuilder::new()
            .with_anime_title("Steins;Gate")
            .with_episode_number(23)
            .with_file_size(400 * 1024 * 1024)
            .with_content_pattern(ContentPattern::RealVideo)
            .with_standard_hashes()
            .build();

        assert_eq!(file.anime_title, "Steins;Gate");
        assert_eq!(file.episode_number, 23);
        assert_eq!(file.file_size, 400 * 1024 * 1024);
        assert!(matches!(file.content_pattern, ContentPattern::RealVideo));
        assert_eq!(file.expected_hashes.len(), 4); // ED2K, CRC32, MD5, SHA1
    }
}
