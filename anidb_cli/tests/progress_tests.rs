#[cfg(test)]
mod progress_tests {
    use anidb_cli::progress::renderer::ProgressRenderer;
    use anidb_cli::progress::{format_bytes, format_duration, format_throughput};
    use anidb_client_core::progress::ProgressUpdate;
    use std::path::PathBuf;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(60), "1m");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3600), "1h");
        assert_eq!(format_duration(3660), "1h 1m");
        assert_eq!(format_duration(7200), "2h");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }

    #[test]
    fn test_format_throughput() {
        assert_eq!(format_throughput(0.1), "102 KB/s");
        assert_eq!(format_throughput(1.0), "1.0 MB/s");
        assert_eq!(format_throughput(10.5), "10.5 MB/s");
        assert_eq!(format_throughput(100.0), "100.0 MB/s");
    }

    #[test]
    fn test_progress_renderer_handles_updates() {
        let mut renderer = ProgressRenderer::new();

        // Send a file progress update
        renderer.handle_update(ProgressUpdate::FileProgress {
            path: PathBuf::from("/tmp/test_file.mkv"),
            bytes_processed: 512_000,
            total_bytes: 1_000_000,
            operation: "Hashing".to_string(),
            throughput_mbps: Some(5.0),
            memory_usage_bytes: Some(10_000_000),
            buffer_size: Some(64 * 1024),
        });

        // And a hash progress update
        renderer.handle_update(ProgressUpdate::HashProgress {
            algorithm: "MD5".to_string(),
            bytes_processed: 1_000_000,
            total_bytes: 1_000_000,
        });

        // Finish should not panic
        renderer.finish();
    }
}
