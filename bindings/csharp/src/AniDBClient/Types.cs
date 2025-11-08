using System;
using System.Collections.Generic;

namespace AniDBClient
{
    /// <summary>
    /// Hash algorithm for file processing
    /// </summary>
    public enum HashAlgorithm
    {
        /// <summary>ED2K hash algorithm (default for AniDB)</summary>
        ED2K,
        /// <summary>CRC32 checksum</summary>
        CRC32,
        /// <summary>MD5 hash</summary>
        MD5,
        /// <summary>SHA1 hash</summary>
        SHA1,
        /// <summary>Tiger Tree Hash</summary>
        TTH
    }

    /// <summary>
    /// Processing status
    /// </summary>
    public enum ProcessingStatus
    {
        /// <summary>Processing pending</summary>
        Pending,
        /// <summary>Currently processing</summary>
        Processing,
        /// <summary>Processing completed</summary>
        Completed,
        /// <summary>Processing failed</summary>
        Failed,
        /// <summary>Processing cancelled</summary>
        Cancelled
    }

    /// <summary>
    /// Event type for processing events
    /// </summary>
    public enum EventType
    {
        /// <summary>File processing started</summary>
        FileStart,
        /// <summary>File processing completed</summary>
        FileComplete,
        /// <summary>Hash calculation started</summary>
        HashStart,
        /// <summary>Hash calculation completed</summary>
        HashComplete,
        /// <summary>Cache hit occurred</summary>
        CacheHit,
        /// <summary>Cache miss occurred</summary>
        CacheMiss,
        /// <summary>Network request started</summary>
        NetworkStart,
        /// <summary>Network request completed</summary>
        NetworkComplete,
        /// <summary>Memory threshold reached</summary>
        MemoryWarning
    }

    /// <summary>
    /// Client configuration
    /// </summary>
    public class ClientConfiguration
    {
        /// <summary>
        /// Cache directory path (default: .anidb_cache)
        /// </summary>
        public string CacheDirectory { get; set; } = ".anidb_cache";

        /// <summary>
        /// Maximum concurrent file operations (default: 4)
        /// </summary>
        public int MaxConcurrentFiles { get; set; } = 4;

        /// <summary>
        /// Chunk size for file processing in bytes (default: 65536)
        /// </summary>
        public int ChunkSize { get; set; } = 65536;

        /// <summary>
        /// Maximum memory usage in bytes (0 for default)
        /// </summary>
        public long MaxMemoryUsage { get; set; } = 0;

        /// <summary>
        /// Enable debug logging
        /// </summary>
        public bool EnableDebugLogging { get; set; } = false;

        /// <summary>
        /// AniDB username (optional)
        /// </summary>
        public string? Username { get; set; }

        /// <summary>
        /// AniDB password (optional)
        /// </summary>
        public string? Password { get; set; }
    }

    /// <summary>
    /// Options for file processing
    /// </summary>
    public class ProcessingOptions
    {
        /// <summary>
        /// Hash algorithms to calculate (default: ED2K)
        /// </summary>
        public HashAlgorithm[] Algorithms { get; set; } = new[] { HashAlgorithm.ED2K };

        /// <summary>
        /// Enable progress reporting
        /// </summary>
        public bool EnableProgress { get; set; } = true;

        /// <summary>
        /// Verify existing hashes in cache
        /// </summary>
        public bool VerifyExisting { get; set; } = false;

        /// <summary>
        /// Progress callback
        /// </summary>
        public Action<ProgressInfo>? ProgressCallback { get; set; }
    }

    /// <summary>
    /// Options for batch processing
    /// </summary>
    public class BatchOptions : ProcessingOptions
    {
        /// <summary>
        /// Maximum concurrent operations
        /// </summary>
        public int MaxConcurrent { get; set; } = 4;

        /// <summary>
        /// Continue processing on error
        /// </summary>
        public bool ContinueOnError { get; set; } = true;

        /// <summary>
        /// Skip files already in cache
        /// </summary>
        public bool SkipExisting { get; set; } = false;
    }

    /// <summary>
    /// Progress information
    /// </summary>
    public class ProgressInfo
    {
        /// <summary>
        /// Progress percentage (0.0 to 100.0)
        /// </summary>
        public float Percentage { get; init; }

        /// <summary>
        /// Number of bytes processed
        /// </summary>
        public long BytesProcessed { get; init; }

        /// <summary>
        /// Total number of bytes
        /// </summary>
        public long TotalBytes { get; init; }

        /// <summary>
        /// Current file being processed (batch operations)
        /// </summary>
        public string? CurrentFile { get; init; }

        /// <summary>
        /// Files completed (batch operations)
        /// </summary>
        public int? FilesCompleted { get; init; }

        /// <summary>
        /// Total files (batch operations)
        /// </summary>
        public int? TotalFiles { get; init; }
    }

    /// <summary>
    /// Hash result
    /// </summary>
    public class HashResult
    {
        /// <summary>
        /// Hash algorithm used
        /// </summary>
        public HashAlgorithm Algorithm { get; init; }

        /// <summary>
        /// Hash value as hexadecimal string
        /// </summary>
        public string Value { get; init; } = string.Empty;
    }

    /// <summary>
    /// File processing result
    /// </summary>
    public class FileResult
    {
        /// <summary>
        /// File path
        /// </summary>
        public string FilePath { get; init; } = string.Empty;

        /// <summary>
        /// File size in bytes
        /// </summary>
        public long FileSize { get; init; }

        /// <summary>
        /// Processing status
        /// </summary>
        public ProcessingStatus Status { get; init; }

        /// <summary>
        /// Hash results
        /// </summary>
        public IReadOnlyList<HashResult> Hashes { get; init; } = Array.Empty<HashResult>();

        /// <summary>
        /// Processing time
        /// </summary>
        public TimeSpan ProcessingTime { get; init; }

        /// <summary>
        /// Error message (if failed)
        /// </summary>
        public string? ErrorMessage { get; init; }

        /// <summary>
        /// Whether the result was from cache
        /// </summary>
        public bool FromCache { get; init; }
    }

    /// <summary>
    /// Batch processing result
    /// </summary>
    public class BatchResult
    {
        /// <summary>
        /// Total number of files
        /// </summary>
        public int TotalFiles { get; init; }

        /// <summary>
        /// Number of successfully processed files
        /// </summary>
        public int SuccessfulFiles { get; init; }

        /// <summary>
        /// Number of failed files
        /// </summary>
        public int FailedFiles { get; init; }

        /// <summary>
        /// Individual file results
        /// </summary>
        public IReadOnlyList<FileResult> Results { get; init; } = Array.Empty<FileResult>();

        /// <summary>
        /// Total processing time
        /// </summary>
        public TimeSpan TotalTime { get; init; }
    }

    /// <summary>
    /// Anime identification information
    /// </summary>
    public class AnimeInfo
    {
        /// <summary>
        /// AniDB anime ID
        /// </summary>
        public long AnimeId { get; init; }

        /// <summary>
        /// AniDB episode ID
        /// </summary>
        public long EpisodeId { get; init; }

        /// <summary>
        /// Anime title
        /// </summary>
        public string Title { get; init; } = string.Empty;

        /// <summary>
        /// Episode number
        /// </summary>
        public int EpisodeNumber { get; init; }

        /// <summary>
        /// Confidence score (0.0 to 1.0)
        /// </summary>
        public double Confidence { get; init; }

        /// <summary>
        /// Source of identification
        /// </summary>
        public IdentificationSource Source { get; init; }
    }

    /// <summary>
    /// Source of anime identification
    /// </summary>
    public enum IdentificationSource
    {
        /// <summary>From AniDB API</summary>
        AniDB,
        /// <summary>From local cache</summary>
        Cache,
        /// <summary>From filename parsing</summary>
        Filename
    }

    /// <summary>
    /// Cache statistics
    /// </summary>
    public class CacheStatistics
    {
        /// <summary>
        /// Total number of entries
        /// </summary>
        public long TotalEntries { get; init; }

        /// <summary>
        /// Cache size in bytes
        /// </summary>
        public long SizeInBytes { get; init; }

        /// <summary>
        /// Hit rate percentage
        /// </summary>
        public double HitRate { get; init; }
    }

    /// <summary>
    /// Processing event
    /// </summary>
    public class ProcessingEvent
    {
        /// <summary>
        /// Event type
        /// </summary>
        public EventType Type { get; init; }

        /// <summary>
        /// Event timestamp
        /// </summary>
        public DateTime Timestamp { get; init; }

        /// <summary>
        /// File path (if applicable)
        /// </summary>
        public string? FilePath { get; init; }

        /// <summary>
        /// File size (if applicable)
        /// </summary>
        public long? FileSize { get; init; }

        /// <summary>
        /// Hash algorithm (if applicable)
        /// </summary>
        public HashAlgorithm? Algorithm { get; init; }

        /// <summary>
        /// Hash value (if applicable)
        /// </summary>
        public string? HashValue { get; init; }

        /// <summary>
        /// Memory usage information (if applicable)
        /// </summary>
        public (long Current, long Max)? MemoryUsage { get; init; }

        /// <summary>
        /// Additional context
        /// </summary>
        public string? Context { get; init; }
    }
}