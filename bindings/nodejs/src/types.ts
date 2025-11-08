/**
 * Type definitions for AniDB Client
 */

/**
 * Client configuration options
 */
export interface AniDBConfig {
  /** Cache directory path (default: .anidb_cache) */
  cacheDir?: string;
  
  /** Maximum concurrent file operations (default: 4) */
  maxConcurrentFiles?: number;
  
  /** Chunk size for file processing in bytes (default: 65536) */
  chunkSize?: number;
  
  /** Maximum memory usage in bytes (default: automatic) */
  maxMemoryUsage?: number;
  
  /** Enable debug logging (default: false) */
  enableDebugLogging?: boolean;
  
  /** AniDB username (optional) */
  username?: string;
  
  /** AniDB password (optional) */
  password?: string;
}

/**
 * Hash algorithm identifiers
 */
export enum HashAlgorithm {
  /** ED2K hash algorithm (default for AniDB) */
  ED2K = 1,
  
  /** CRC32 checksum */
  CRC32 = 2,
  
  /** MD5 hash */
  MD5 = 3,
  
  /** SHA1 hash */
  SHA1 = 4,
  
  /** Tiger Tree Hash */
  TTH = 5
}

/**
 * Processing status codes
 */
export enum Status {
  /** Processing pending */
  PENDING = 0,
  
  /** Currently processing */
  PROCESSING = 1,
  
  /** Processing completed */
  COMPLETED = 2,
  
  /** Processing failed */
  FAILED = 3,
  
  /** Processing cancelled */
  CANCELLED = 4
}

/**
 * Error codes
 */
export enum ErrorCode {
  /** Operation completed successfully */
  SUCCESS = 0,
  
  /** Invalid handle provided */
  INVALID_HANDLE = 1,
  
  /** Invalid parameter provided */
  INVALID_PARAMETER = 2,
  
  /** File not found */
  FILE_NOT_FOUND = 3,
  
  /** Error during processing */
  PROCESSING = 4,
  
  /** Out of memory */
  OUT_OF_MEMORY = 5,
  
  /** I/O error */
  IO = 6,
  
  /** Network error */
  NETWORK = 7,
  
  /** Operation cancelled */
  CANCELLED = 8,
  
  /** Invalid UTF-8 string */
  INVALID_UTF8 = 9,
  
  /** Version mismatch */
  VERSION_MISMATCH = 10,
  
  /** Operation timeout */
  TIMEOUT = 11,
  
  /** Permission denied */
  PERMISSION_DENIED = 12,
  
  /** Cache error */
  CACHE = 13,
  
  /** Resource busy */
  BUSY = 14,
  
  /** Unknown error */
  UNKNOWN = 99
}

/**
 * File processing options
 */
export interface ProcessOptions {
  /** Hash algorithms to calculate (default: ['ed2k']) */
  algorithms?: (HashAlgorithm | string)[];
  
  /** Enable progress reporting (default: false) */
  enableProgress?: boolean;
  
  /** Verify existing hashes in cache (default: false) */
  verifyExisting?: boolean;
  
  /** Progress callback (alternative to events) */
  onProgress?: (progress: ProgressInfo) => void;
}

/**
 * Batch processing options
 */
export interface BatchOptions {
  /** Hash algorithms to calculate (default: ['ed2k']) */
  algorithms?: (HashAlgorithm | string)[];
  
  /** Maximum concurrent operations (default: 4) */
  maxConcurrent?: number;
  
  /** Continue processing on error (default: false) */
  continueOnError?: boolean;
  
  /** Skip files already in cache (default: false) */
  skipExisting?: boolean;
  
  /** Progress callback */
  onProgress?: (progress: BatchProgressInfo) => void;
  
  /** File completion callback */
  onFileComplete?: (result: FileResult) => void;
}

/**
 * File processing result
 */
export interface FileResult {
  /** File path */
  filePath: string;
  
  /** File size in bytes */
  fileSize: number;
  
  /** Processing status */
  status: Status;
  
  /** Hash results (key: algorithm name, value: hash string) */
  hashes: Record<string, string>;
  
  /** Processing time in milliseconds */
  processingTimeMs: number;
  
  /** Error message if failed */
  error?: string;
}

/**
 * Batch processing result
 */
export interface BatchResult {
  /** Total number of files */
  totalFiles: number;
  
  /** Number of successfully processed files */
  successfulFiles: number;
  
  /** Number of failed files */
  failedFiles: number;
  
  /** Individual file results */
  results: FileResult[];
  
  /** Total processing time in milliseconds */
  totalTimeMs: number;
}

/**
 * Anime identification information
 */
export interface AnimeInfo {
  /** AniDB anime ID */
  animeId: number;
  
  /** AniDB episode ID */
  episodeId: number;
  
  /** Anime title */
  title: string;
  
  /** Episode number */
  episodeNumber: number;
  
  /** Confidence score (0.0 to 1.0) */
  confidence: number;
  
  /** Source of identification */
  source: 'anidb' | 'cache' | 'filename';
}

/**
 * Progress information
 */
export interface ProgressInfo {
  /** Progress percentage (0.0 to 100.0) */
  percentage: number;
  
  /** Bytes processed so far */
  bytesProcessed: number;
  
  /** Total bytes to process */
  totalBytes: number;
  
  /** Current file being processed */
  currentFile?: string;
  
  /** Current operation */
  operation?: string;
}

/**
 * Batch progress information
 */
export interface BatchProgressInfo extends ProgressInfo {
  /** Number of files completed */
  filesCompleted: number;
  
  /** Total number of files */
  totalFiles: number;
  
  /** Current file index */
  currentFileIndex: number;
}

/**
 * Event types
 */
export enum EventType {
  /** File processing started */
  FILE_START = 1,
  
  /** File processing completed */
  FILE_COMPLETE = 2,
  
  /** Hash calculation started */
  HASH_START = 3,
  
  /** Hash calculation completed */
  HASH_COMPLETE = 4,
  
  /** Cache hit occurred */
  CACHE_HIT = 5,
  
  /** Cache miss occurred */
  CACHE_MISS = 6,
  
  /** Network request started */
  NETWORK_START = 7,
  
  /** Network request completed */
  NETWORK_COMPLETE = 8,
  
  /** Memory threshold reached */
  MEMORY_WARNING = 9
}

/**
 * Event data
 */
export interface AniDBEvent {
  /** Event type */
  type: EventType;
  
  /** Timestamp (milliseconds since epoch) */
  timestamp: number;
  
  /** Event-specific data */
  data: {
    file?: {
      filePath: string;
      fileSize: number;
    };
    hash?: {
      algorithm: HashAlgorithm;
      hashValue: string;
    };
    cache?: {
      filePath: string;
      algorithm: HashAlgorithm;
    };
    network?: {
      endpoint: string;
      statusCode: number;
    };
    memory?: {
      currentUsage: number;
      maxUsage: number;
    };
  };
  
  /** Additional context */
  context?: string;
}

/**
 * Callback types
 */
export enum CallbackType {
  /** Progress update callback */
  PROGRESS = 1,
  
  /** Error notification callback */
  ERROR = 2,
  
  /** Operation completion callback */
  COMPLETION = 3,
  
  /** General event callback */
  EVENT = 4
}

/**
 * Stream event types
 */
export interface StreamEvent {
  type: 'progress' | 'complete' | 'error';
  percentage?: number;
  bytesProcessed?: number;
  totalBytes?: number;
  result?: FileResult;
  error?: Error;
}