import Foundation
import CAniDB

// MARK: - Configuration

extension AniDBClient {
    /// Client configuration
    public struct Configuration: Equatable, Sendable {
        /// Cache directory URL
        public let cacheDirectory: URL?
        
        /// Maximum concurrent file operations
        public let maxConcurrentFiles: Int
        
        /// Chunk size for file processing
        public let chunkSize: Int
        
        /// Maximum memory usage in bytes
        public let maxMemoryUsage: Int
        
        /// Enable debug logging
        public let enableDebugLogging: Bool
        
        /// AniDB username
        public let username: String?
        
        /// AniDB password
        public let password: String?
        
        /// Default configuration
        public static let `default` = Configuration()
        
        public init(
            cacheDirectory: URL? = nil,
            maxConcurrentFiles: Int = 4,
            chunkSize: Int = 65536,
            maxMemoryUsage: Int = 500_000_000,
            enableDebugLogging: Bool = false,
            username: String? = nil,
            password: String? = nil
        ) {
            self.cacheDirectory = cacheDirectory
            self.maxConcurrentFiles = maxConcurrentFiles
            self.chunkSize = chunkSize
            self.maxMemoryUsage = maxMemoryUsage
            self.enableDebugLogging = enableDebugLogging
            self.username = username
            self.password = password
        }
        
        func toCConfig() -> anidb_config_t {
            let cacheDir = cacheDirectory?.path.withCString { strdup($0) }
            let user = username?.withCString { strdup($0) }
            let pass = password?.withCString { strdup($0) }
            
            return anidb_config_t(
                cache_dir: cacheDir,
                max_concurrent_files: maxConcurrentFiles,
                chunk_size: chunkSize,
                max_memory_usage: maxMemoryUsage,
                enable_debug_logging: enableDebugLogging ? 1 : 0,
                username: user,
                password: pass
            )
        }
    }
}

// MARK: - Hash Algorithm

extension AniDBClient {
    /// Supported hash algorithms
    public enum HashAlgorithm: CaseIterable, Hashable, Sendable {
        case ed2k
        case crc32
        case md5
        case sha1
        case tth
        
        var name: String {
            switch self {
            case .ed2k: return "ED2K"
            case .crc32: return "CRC32"
            case .md5: return "MD5"
            case .sha1: return "SHA1"
            case .tth: return "TTH"
            }
        }
        
        func toCType() -> anidb_hash_algorithm_t {
            switch self {
            case .ed2k: return ANIDB_HASH_ED2K
            case .crc32: return ANIDB_HASH_CRC32
            case .md5: return ANIDB_HASH_MD5
            case .sha1: return ANIDB_HASH_SHA1
            case .tth: return ANIDB_HASH_TTH
            }
        }
        
        init?(from cType: anidb_hash_algorithm_t) {
            switch cType {
            case ANIDB_HASH_ED2K: self = .ed2k
            case ANIDB_HASH_CRC32: self = .crc32
            case ANIDB_HASH_MD5: self = .md5
            case ANIDB_HASH_SHA1: self = .sha1
            case ANIDB_HASH_TTH: self = .tth
            default: return nil
            }
        }
    }
}

// MARK: - File Result

extension AniDBClient {
    /// Result of file processing
    public struct FileResult: Sendable {
        /// File URL
        public let url: URL
        
        /// File size in bytes
        public let fileSize: UInt64
        
        /// Processing status
        public let status: Status
        
        /// Calculated hashes
        public let hashes: [HashAlgorithm: String]
        
        /// Processing time
        public let processingTime: TimeInterval
        
        /// Error message if failed
        public let errorMessage: String?
        
        init(from cResult: anidb_file_result_t) {
            self.url = URL(fileURLWithPath: String(cString: cResult.file_path))
            self.fileSize = cResult.file_size
            self.status = Status(from: cResult.status)
            self.processingTime = TimeInterval(cResult.processing_time_ms) / 1000.0
            self.errorMessage = cResult.error_message.map { String(cString: $0) }
            
            // Convert hashes
            var hashes: [HashAlgorithm: String] = [:]
            if cResult.hash_count > 0 {
                let hashArray = Array(UnsafeBufferPointer(
                    start: cResult.hashes,
                    count: cResult.hash_count
                ))
                
                for hash in hashArray {
                    if let algo = HashAlgorithm(from: hash.algorithm) {
                        hashes[algo] = String(cString: hash.hash_value)
                    }
                }
            }
            self.hashes = hashes
        }
    }
    
    /// Processing status
    public enum Status: Sendable {
        case pending
        case processing
        case completed
        case failed
        case cancelled
        
        init(from cStatus: anidb_status_t) {
            switch cStatus {
            case ANIDB_STATUS_PENDING: self = .pending
            case ANIDB_STATUS_PROCESSING: self = .processing
            case ANIDB_STATUS_COMPLETED: self = .completed
            case ANIDB_STATUS_FAILED: self = .failed
            case ANIDB_STATUS_CANCELLED: self = .cancelled
            default: self = .failed
            }
        }
    }
}

// MARK: - Batch Result

extension AniDBClient {
    /// Result of batch processing
    public struct BatchResult: Sendable {
        /// Successfully processed files
        public let successful: [FileResult]
        
        /// Failed files with errors
        public let failed: [(URL, Error)]
        
        /// Total processing time
        public let totalTime: TimeInterval
        
        /// Total number of files
        public var totalFiles: Int {
            successful.count + failed.count
        }
        
        /// Success rate
        public var successRate: Double {
            guard totalFiles > 0 else { return 0 }
            return Double(successful.count) / Double(totalFiles)
        }
    }
    
    /// Batch processing progress
    public struct BatchProgress: Sendable {
        /// Total number of files
        public let totalFiles: Int
        
        /// Number of completed files
        public let completedFiles: Int
        
        /// Currently processing file
        public let currentFile: URL?
        
        /// Overall progress percentage
        public let overallProgress: Float
    }
}

// MARK: - Progress

extension AniDBClient {
    /// File processing progress
    public struct Progress: Sendable {
        /// Progress percentage (0-100)
        public let percentage: Float
        
        /// Bytes processed so far
        public let bytesProcessed: UInt64
        
        /// Total bytes to process
        public let totalBytes: UInt64
        
        /// Estimated time remaining
        public var estimatedTimeRemaining: TimeInterval? {
            guard bytesProcessed > 0 && percentage > 0 else { return nil }
            let rate = Double(bytesProcessed) / Double(percentage)
            let remaining = Double(totalBytes - bytesProcessed)
            return remaining / rate
        }
    }
}

// MARK: - Anime Info

extension AniDBClient {
    /// Anime identification information
    public struct AnimeInfo: Sendable {
        /// AniDB anime ID
        public let animeID: UInt64
        
        /// AniDB episode ID
        public let episodeID: UInt64
        
        /// Anime title
        public let title: String
        
        /// Episode number
        public let episodeNumber: UInt32
        
        /// Confidence score (0.0 to 1.0)
        public let confidence: Double
        
        /// Source of identification
        public let source: IdentificationSource
        
        init(from cInfo: anidb_anime_info_t) {
            self.animeID = cInfo.anime_id
            self.episodeID = cInfo.episode_id
            self.title = String(cString: cInfo.title)
            self.episodeNumber = cInfo.episode_number
            self.confidence = cInfo.confidence
            self.source = IdentificationSource(rawValue: cInfo.source) ?? .unknown
        }
    }
    
    /// Source of anime identification
    public enum IdentificationSource: Int, Sendable {
        case anidb = 0
        case cache = 1
        case filename = 2
        case unknown = -1
    }
}

// MARK: - Cache Statistics

extension AniDBClient {
    /// Cache statistics
    public struct CacheStatistics: Sendable {
        /// Total number of entries
        public let totalEntries: Int
        
        /// Total size in bytes
        public let sizeInBytes: UInt64
        
        /// Human-readable size
        public var formattedSize: String {
            ByteCountFormatter.string(fromByteCount: Int64(sizeInBytes), countStyle: .file)
        }
    }
}

// MARK: - Events

extension AniDBClient {
    /// Event types
    public enum Event: Sendable {
        case fileStart(url: URL, size: UInt64)
        case fileComplete(url: URL, size: UInt64, duration: TimeInterval)
        case hashStart(algorithm: HashAlgorithm)
        case hashComplete(algorithm: HashAlgorithm, hash: String)
        case cacheHit(url: URL, algorithm: HashAlgorithm)
        case cacheMiss(url: URL, algorithm: HashAlgorithm)
        case networkStart(endpoint: String)
        case networkComplete(endpoint: String, statusCode: Int)
        case memoryWarning(current: UInt64, max: UInt64)
        
        init(from cEvent: anidb_event_t) {
            let timestamp = Date(timeIntervalSince1970: TimeInterval(cEvent.timestamp) / 1000.0)
            
            switch cEvent.type {
            case ANIDB_EVENT_FILE_START:
                let path = String(cString: cEvent.data.file.file_path)
                self = .fileStart(url: URL(fileURLWithPath: path), size: cEvent.data.file.file_size)
                
            case ANIDB_EVENT_FILE_COMPLETE:
                let path = String(cString: cEvent.data.file.file_path)
                // Extract duration from context if available
                let duration: TimeInterval = 0 // Would need to parse from context
                self = .fileComplete(
                    url: URL(fileURLWithPath: path),
                    size: cEvent.data.file.file_size,
                    duration: duration
                )
                
            case ANIDB_EVENT_HASH_START:
                if let algo = HashAlgorithm(from: cEvent.data.hash.algorithm) {
                    self = .hashStart(algorithm: algo)
                } else {
                    self = .hashStart(algorithm: .ed2k)
                }
                
            case ANIDB_EVENT_HASH_COMPLETE:
                if let algo = HashAlgorithm(from: cEvent.data.hash.algorithm) {
                    let hash = String(cString: cEvent.data.hash.hash_value)
                    self = .hashComplete(algorithm: algo, hash: hash)
                } else {
                    self = .hashComplete(algorithm: .ed2k, hash: "")
                }
                
            case ANIDB_EVENT_CACHE_HIT:
                let path = String(cString: cEvent.data.cache.file_path)
                if let algo = HashAlgorithm(from: cEvent.data.cache.algorithm) {
                    self = .cacheHit(url: URL(fileURLWithPath: path), algorithm: algo)
                } else {
                    self = .cacheHit(url: URL(fileURLWithPath: path), algorithm: .ed2k)
                }
                
            case ANIDB_EVENT_CACHE_MISS:
                let path = String(cString: cEvent.data.cache.file_path)
                if let algo = HashAlgorithm(from: cEvent.data.cache.algorithm) {
                    self = .cacheMiss(url: URL(fileURLWithPath: path), algorithm: algo)
                } else {
                    self = .cacheMiss(url: URL(fileURLWithPath: path), algorithm: .ed2k)
                }
                
            case ANIDB_EVENT_NETWORK_START:
                let endpoint = String(cString: cEvent.data.network.endpoint)
                self = .networkStart(endpoint: endpoint)
                
            case ANIDB_EVENT_NETWORK_COMPLETE:
                let endpoint = String(cString: cEvent.data.network.endpoint)
                self = .networkComplete(endpoint: endpoint, statusCode: Int(cEvent.data.network.status_code))
                
            case ANIDB_EVENT_MEMORY_WARNING:
                self = .memoryWarning(
                    current: cEvent.data.memory.current_usage,
                    max: cEvent.data.memory.max_usage
                )
                
            default:
                // Unknown event type, default to memory warning
                self = .memoryWarning(current: 0, max: 0)
            }
        }
    }
}

// MARK: - Helper Extensions

extension anidb_config_t {
    mutating func cleanup() {
        if let cacheDir = cache_dir {
            free(UnsafeMutablePointer(mutating: cacheDir))
        }
        if let user = username {
            free(UnsafeMutablePointer(mutating: user))
        }
        if let pass = password {
            free(UnsafeMutablePointer(mutating: pass))
        }
    }
}