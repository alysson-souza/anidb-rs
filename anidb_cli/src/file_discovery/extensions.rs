//! Default media file extensions module
//!
//! Provides default media file extensions for automatic file discovery
//! when no explicit patterns are provided.

/// Default media file extensions for video files
#[allow(dead_code)]
pub const DEFAULT_VIDEO_EXTENSIONS: &[&str] = &[
    "wmv", "asf", "ts", "mpg", "mpeg", "mkv", "ogm", "rm", "rmvb", "divx", "xvid", "mp4", "m4v",
    "m2ts", "mov", "webm", "ogv", "mts", "flv", "avi", "vob", "m2v", "3gp", "3g2", "f4v",
];

/// Default media file extensions for subtitle files
#[allow(dead_code)]
pub const DEFAULT_SUBTITLE_EXTENSIONS: &[&str] = &["ass", "ssa", "srt", "idx", "sub", "vtt", "sup"];

/// Default media file extensions for audio files
#[allow(dead_code)]
pub const DEFAULT_AUDIO_EXTENSIONS: &[&str] = &[
    "ogg", "wav", "mp3", "mp2", "flac", "mpc", "mka", "m4a", "wma", "ra", "opus", "aac", "ac3",
    "dts", "ape", "wv", "tta",
];

/// All default media extensions combined
pub const DEFAULT_MEDIA_EXTENSIONS: &[&str] = &[
    // Video
    "wmv", "asf", "ts", "mpg", "mpeg", "mkv", "ogm", "rm", "rmvb", "divx", "xvid", "mp4", "m4v",
    "m2ts", "mov", "webm", "ogv", "mts", "flv", "avi", "vob", "m2v", "3gp", "3g2", "f4v",
    // Subtitles
    "ass", "ssa", "srt", "idx", "sub", "vtt", "sup", // Audio
    "ogg", "wav", "mp3", "mp2", "flac", "mpc", "mka", "m4a", "wma", "ra", "opus", "aac", "ac3",
    "dts", "ape", "wv", "tta",
];

/// Convert extensions to glob patterns
pub fn extensions_to_patterns(extensions: &[&str]) -> Vec<String> {
    extensions
        .iter()
        .flat_map(|ext| vec![format!("*.{}", ext), format!("*.{}", ext.to_uppercase())])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_extensions_are_unique() {
        use std::collections::HashSet;

        let mut seen = HashSet::new();
        for ext in DEFAULT_MEDIA_EXTENSIONS {
            assert!(seen.insert(ext), "Duplicate extension found: {ext}");
        }
    }

    #[test]
    fn test_extensions_to_patterns() {
        let patterns = extensions_to_patterns(&["mkv", "mp4"]);
        assert_eq!(patterns.len(), 4);
        assert!(patterns.contains(&"*.mkv".to_string()));
        assert!(patterns.contains(&"*.MKV".to_string()));
        assert!(patterns.contains(&"*.mp4".to_string()));
        assert!(patterns.contains(&"*.MP4".to_string()));
    }
}
