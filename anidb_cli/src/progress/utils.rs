//! Utility functions for progress formatting
//!
//! This module provides formatting utilities used throughout the CLI.

use std::time::Duration;

/// Format bytes as human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

/// Format throughput as human-readable string
pub fn format_throughput(mbps: f64) -> String {
    if mbps >= 1.0 {
        format!("{mbps:.1} MB/s")
    } else {
        format!("{:.0} KB/s", mbps * 1024.0)
    }
}

/// Format duration as human-readable string
#[allow(dead_code)]
pub fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        let minutes = seconds / 60;
        let remaining_seconds = seconds % 60;
        if remaining_seconds > 0 {
            format!("{minutes}m {remaining_seconds}s")
        } else {
            format!("{minutes}m")
        }
    } else {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        if minutes > 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{hours}h")
        }
    }
}

/// Format duration from Duration type
#[allow(dead_code)]
pub fn format_duration_from_duration(duration: Duration) -> String {
    format_duration(duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(format_throughput(0.5), "512 KB/s");
        assert_eq!(format_throughput(1.0), "1.0 MB/s");
        assert_eq!(format_throughput(100.5), "100.5 MB/s");
    }

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
}
