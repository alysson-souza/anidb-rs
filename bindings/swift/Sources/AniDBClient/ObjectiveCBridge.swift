import Foundation

// MARK: - Objective-C Compatible Client

/// Objective-C compatible wrapper for AniDBClient
@available(macOS 13.0, *)
@objc(ANDClient)
public class ObjCAniDBClient: NSObject {
    
    private let swiftClient: AniDBClient
    
    /// Initialize with default configuration
    @objc
    public override init() {
        do {
            self.swiftClient = try AniDBClient()
        } catch {
            fatalError("Failed to initialize AniDB client: \(error)")
        }
        super.init()
    }
    
    /// Initialize with custom configuration
    @objc
    public init(configuration: ObjCConfiguration) throws {
        self.swiftClient = try AniDBClient(configuration: configuration.toSwiftConfiguration())
        super.init()
    }
    
    /// Process a file synchronously
    @objc
    public func processFile(
        at path: String,
        algorithms: [NSNumber],
        progressHandler: ((ObjCProgress) -> Void)?,
        completion: @escaping (ObjCFileResult?, NSError?) -> Void
    ) {
        let url = URL(fileURLWithPath: path)
        let swiftAlgorithms = Set(algorithms.compactMap { ObjCHashAlgorithm(rawValue: $0.intValue)?.toSwiftAlgorithm() })
        
        Task {
            do {
                let result = try await swiftClient.processFile(
                    at: url,
                    algorithms: swiftAlgorithms,
                    progress: progressHandler.map { handler in
                        { progress in
                            handler(ObjCProgress(from: progress))
                        }
                    }
                )
                
                await MainActor.run {
                    completion(ObjCFileResult(from: result), nil)
                }
            } catch {
                await MainActor.run {
                    completion(nil, error as NSError)
                }
            }
        }
    }
    
    /// Calculate hash for data
    @objc
    public func calculateHash(
        for data: Data,
        algorithm: ObjCHashAlgorithm
    ) throws -> String {
        try swiftClient.calculateHash(
            for: data,
            algorithm: algorithm.toSwiftAlgorithm()
        )
    }
    
    /// Clear cache
    @objc
    public func clearCache() throws {
        try swiftClient.clearCache()
    }
    
    /// Get cache statistics
    @objc
    public func cacheStatistics() throws -> ObjCCacheStatistics {
        let stats = try swiftClient.cacheStatistics()
        return ObjCCacheStatistics(from: stats)
    }
}

// MARK: - Objective-C Types

/// Objective-C compatible configuration
@objc(ANDConfiguration)
public class ObjCConfiguration: NSObject {
    @objc public var cacheDirectory: String?
    @objc public var maxConcurrentFiles: NSInteger = 4
    @objc public var chunkSize: NSInteger = 65536
    @objc public var maxMemoryUsage: NSInteger = 500_000_000
    @objc public var enableDebugLogging: Bool = false
    @objc public var username: String?
    @objc public var password: String?
    
    func toSwiftConfiguration() -> AniDBClient.Configuration {
        AniDBClient.Configuration(
            cacheDirectory: cacheDirectory.map { URL(fileURLWithPath: $0) },
            maxConcurrentFiles: maxConcurrentFiles,
            chunkSize: chunkSize,
            maxMemoryUsage: maxMemoryUsage,
            enableDebugLogging: enableDebugLogging,
            username: username,
            password: password
        )
    }
}

/// Objective-C compatible hash algorithm enum
@objc(ANDHashAlgorithm)
public enum ObjCHashAlgorithm: Int {
    case ed2k = 1
    case crc32 = 2
    case md5 = 3
    case sha1 = 4
    case tth = 5
    
    func toSwiftAlgorithm() -> AniDBClient.HashAlgorithm {
        switch self {
        case .ed2k: return .ed2k
        case .crc32: return .crc32
        case .md5: return .md5
        case .sha1: return .sha1
        case .tth: return .tth
        }
    }
}

/// Objective-C compatible file result
@objc(ANDFileResult)
public class ObjCFileResult: NSObject {
    @objc public let filePath: String
    @objc public let fileSize: UInt64
    @objc public let status: ObjCStatus
    @objc public let hashes: [String: String]
    @objc public let processingTime: TimeInterval
    @objc public let errorMessage: String?
    
    init(from result: AniDBClient.FileResult) {
        self.filePath = result.url.path
        self.fileSize = result.fileSize
        self.status = ObjCStatus(from: result.status)
        
        // Convert hash dictionary
        var objcHashes: [String: String] = [:]
        for (algo, hash) in result.hashes {
            objcHashes[algo.name] = hash
        }
        self.hashes = objcHashes
        
        self.processingTime = result.processingTime
        self.errorMessage = result.errorMessage
        super.init()
    }
}

/// Objective-C compatible status enum
@objc(ANDStatus)
public enum ObjCStatus: Int {
    case pending = 0
    case processing = 1
    case completed = 2
    case failed = 3
    case cancelled = 4
    
    init(from status: AniDBClient.Status) {
        switch status {
        case .pending: self = .pending
        case .processing: self = .processing
        case .completed: self = .completed
        case .failed: self = .failed
        case .cancelled: self = .cancelled
        }
    }
}

/// Objective-C compatible progress
@objc(ANDProgress)
public class ObjCProgress: NSObject {
    @objc public let percentage: Float
    @objc public let bytesProcessed: UInt64
    @objc public let totalBytes: UInt64
    
    init(from progress: AniDBClient.Progress) {
        self.percentage = progress.percentage
        self.bytesProcessed = progress.bytesProcessed
        self.totalBytes = progress.totalBytes
        super.init()
    }
}

/// Objective-C compatible cache statistics
@objc(ANDCacheStatistics)
public class ObjCCacheStatistics: NSObject {
    @objc public let totalEntries: NSInteger
    @objc public let sizeInBytes: UInt64
    @objc public let formattedSize: String
    
    init(from stats: AniDBClient.CacheStatistics) {
        self.totalEntries = stats.totalEntries
        self.sizeInBytes = stats.sizeInBytes
        self.formattedSize = stats.formattedSize
        super.init()
    }
}