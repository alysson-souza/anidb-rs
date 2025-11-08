import Foundation
import CAniDB

/// Main AniDB client class providing Swift-native interface to the AniDB library
@available(macOS 13.0, *)
public final class AniDBClient: @unchecked Sendable {
    
    // MARK: - Properties
    
    /// Internal handle to the C client
    private var handle: OpaquePointer?
    
    /// Serial queue for thread-safe operations
    private let queue = DispatchQueue(label: "com.anidb.client", qos: .userInitiated)
    
    /// Current configuration
    public let configuration: Configuration
    
    /// Event publisher for reactive programming support
    public let events = AsyncStream<Event>.makeStream()
    private let eventsContinuation: AsyncStream<Event>.Continuation
    
    /// Active operations tracking
    private var activeOperations = Set<UUID>()
    
    // MARK: - Initialization
    
    /// Initialize AniDB client with configuration
    /// - Parameter configuration: Client configuration
    /// - Throws: AniDBError if initialization fails
    public init(configuration: Configuration = .default) throws {
        // Initialize the library once
        Self.initializeLibrary()
        
        self.configuration = configuration
        self.eventsContinuation = events.1
        
        // Create client handle
        try queue.sync {
            var clientHandle: OpaquePointer?
            
            if configuration == .default {
                let result = anidb_client_create(&clientHandle)
                if result != ANIDB_SUCCESS {
                    throw AniDBError(result: result)
                }
            } else {
                // Convert Swift config to C config
                var cConfig = configuration.toCConfig()
                defer { cConfig.cleanup() }
                
                let result = withUnsafePointer(to: &cConfig) { configPtr in
                    anidb_client_create_with_config(configPtr, &clientHandle)
                }
                
                if result != ANIDB_SUCCESS {
                    throw AniDBError(result: result)
                }
            }
            
            self.handle = clientHandle
        }
        
        // Set up event handling
        setupEventHandling()
    }
    
    deinit {
        // Clean up event handling
        eventsContinuation.finish()
        
        // Disconnect events
        if let handle = handle {
            anidb_event_disconnect(handle)
        }
        
        // Destroy client
        queue.sync {
            if let handle = handle {
                anidb_client_destroy(handle)
            }
        }
    }
    
    // MARK: - Public Methods
    
    /// Process a file and calculate hashes
    /// - Parameters:
    ///   - url: URL of the file to process
    ///   - algorithms: Hash algorithms to calculate
    ///   - progress: Optional progress handler
    /// - Returns: File processing result
    /// - Throws: AniDBError if processing fails
    public func processFile(
        at url: URL,
        algorithms: Set<HashAlgorithm> = [.ed2k],
        progress: ((Progress) -> Void)? = nil
    ) async throws -> FileResult {
        
        return try await withCheckedThrowingContinuation { continuation in
            queue.async { [weak self] in
                guard let self = self, let handle = self.handle else {
                    continuation.resume(throwing: AniDBError.clientDestroyed)
                    return
                }
                
                let operationID = UUID()
                self.activeOperations.insert(operationID)
                
                // Convert algorithms
                let algoArray = algorithms.map { $0.toCType() }
                
                // Set up progress callback if needed
                var progressContext: ProgressContext?
                if let progress = progress {
                    progressContext = ProgressContext(handler: progress)
                }
                
                // Create options
                var options = anidb_process_options_t(
                    algorithms: algoArray.withUnsafeBufferPointer { $0.baseAddress },
                    algorithm_count: algoArray.count,
                    enable_progress: progress != nil ? 1 : 0,
                    verify_existing: 0,
                    progress_callback: progressContext != nil ? progressCallback : nil,
                    user_data: progressContext.map { Unmanaged.passUnretained($0).toOpaque() }
                )
                
                // Process file
                url.path.withCString { pathCStr in
                    var result: UnsafeMutablePointer<anidb_file_result_t>?
                    
                    let status = withUnsafeMutablePointer(to: &options) { optionsPtr in
                        anidb_process_file(handle, pathCStr, optionsPtr, &result)
                    }
                    
                    self.activeOperations.remove(operationID)
                    
                    if status == ANIDB_SUCCESS, let result = result {
                        // Convert C result to Swift
                        let fileResult = FileResult(from: result.pointee)
                        anidb_free_file_result(result)
                        continuation.resume(returning: fileResult)
                    } else {
                        continuation.resume(throwing: AniDBError(result: status))
                    }
                }
            }
        }
    }
    
    /// Process multiple files in batch
    /// - Parameters:
    ///   - urls: URLs of files to process
    ///   - algorithms: Hash algorithms to calculate
    ///   - maxConcurrent: Maximum concurrent operations
    ///   - continueOnError: Whether to continue processing on errors
    ///   - progress: Optional progress handler
    /// - Returns: Batch processing results
    /// - Throws: AniDBError if batch processing fails
    public func processBatch(
        urls: [URL],
        algorithms: Set<HashAlgorithm> = [.ed2k],
        maxConcurrent: Int = 4,
        continueOnError: Bool = true,
        progress: ((BatchProgress) -> Void)? = nil
    ) async throws -> BatchResult {
        
        // Use async/await with task groups for batch processing
        return try await withThrowingTaskGroup(of: FileResult?.self) { group in
            let semaphore = AsyncSemaphore(count: maxConcurrent)
            var results: [FileResult] = []
            var errors: [(URL, Error)] = []
            
            for url in urls {
                group.addTask { [weak self] in
                    await semaphore.wait()
                    defer { semaphore.signal() }
                    
                    do {
                        let result = try await self?.processFile(at: url, algorithms: algorithms)
                        
                        // Report progress
                        if let progress = progress {
                            let completed = results.count + errors.count + 1
                            let batchProgress = BatchProgress(
                                totalFiles: urls.count,
                                completedFiles: completed,
                                currentFile: url,
                                overallProgress: Float(completed) / Float(urls.count)
                            )
                            await MainActor.run {
                                progress(batchProgress)
                            }
                        }
                        
                        return result
                    } catch {
                        if !continueOnError {
                            throw error
                        }
                        await MainActor.run {
                            errors.append((url, error))
                        }
                        return nil
                    }
                }
            }
            
            // Collect results
            for try await result in group {
                if let result = result {
                    results.append(result)
                }
            }
            
            return BatchResult(
                successful: results,
                failed: errors,
                totalTime: 0 // Will be calculated
            )
        }
    }
    
    /// Calculate hash for data
    /// - Parameters:
    ///   - data: Data to hash
    ///   - algorithm: Hash algorithm to use
    /// - Returns: Hex string of the hash
    /// - Throws: AniDBError if hashing fails
    public func calculateHash(for data: Data, algorithm: HashAlgorithm) throws -> String {
        try queue.sync {
            let bufferSize = anidb_hash_buffer_size(algorithm.toCType())
            var buffer = [CChar](repeating: 0, count: bufferSize)
            
            let result = data.withUnsafeBytes { bytes in
                anidb_calculate_hash_buffer(
                    bytes.bindMemory(to: UInt8.self).baseAddress,
                    data.count,
                    algorithm.toCType(),
                    &buffer,
                    bufferSize
                )
            }
            
            if result != ANIDB_SUCCESS {
                throw AniDBError(result: result)
            }
            
            return String(cString: buffer)
        }
    }
    
    /// Identify anime from file
    /// - Parameters:
    ///   - ed2kHash: ED2K hash of the file
    ///   - fileSize: Size of the file in bytes
    /// - Returns: Anime identification info
    /// - Throws: AniDBError if identification fails
    public func identifyAnime(ed2kHash: String, fileSize: UInt64) async throws -> AnimeInfo {
        try await withCheckedThrowingContinuation { continuation in
            queue.async { [weak self] in
                guard let self = self, let handle = self.handle else {
                    continuation.resume(throwing: AniDBError.clientDestroyed)
                    return
                }
                
                ed2kHash.withCString { hashCStr in
                    var info: UnsafeMutablePointer<anidb_anime_info_t>?
                    
                    let result = anidb_identify_file(handle, hashCStr, fileSize, &info)
                    
                    if result == ANIDB_SUCCESS, let info = info {
                        let animeInfo = AnimeInfo(from: info.pointee)
                        anidb_free_anime_info(info)
                        continuation.resume(returning: animeInfo)
                    } else {
                        continuation.resume(throwing: AniDBError(result: result))
                    }
                }
            }
        }
    }
    
    // MARK: - Cache Management
    
    /// Clear the hash cache
    /// - Throws: AniDBError if clearing fails
    public func clearCache() throws {
        try queue.sync {
            guard let handle = handle else {
                throw AniDBError.clientDestroyed
            }
            
            let result = anidb_cache_clear(handle)
            if result != ANIDB_SUCCESS {
                throw AniDBError(result: result)
            }
        }
    }
    
    /// Get cache statistics
    /// - Returns: Cache statistics
    /// - Throws: AniDBError if retrieval fails
    public func cacheStatistics() throws -> CacheStatistics {
        try queue.sync {
            guard let handle = handle else {
                throw AniDBError.clientDestroyed
            }
            
            var entries: Int = 0
            var sizeBytes: UInt64 = 0
            
            let result = anidb_cache_get_stats(handle, &entries, &sizeBytes)
            if result != ANIDB_SUCCESS {
                throw AniDBError(result: result)
            }
            
            return CacheStatistics(totalEntries: entries, sizeInBytes: sizeBytes)
        }
    }
    
    /// Check if a file is cached
    /// - Parameters:
    ///   - url: URL of the file
    ///   - algorithm: Hash algorithm
    /// - Returns: True if cached
    /// - Throws: AniDBError if check fails
    public func isCached(url: URL, algorithm: HashAlgorithm) throws -> Bool {
        try queue.sync {
            guard let handle = handle else {
                throw AniDBError.clientDestroyed
            }
            
            var cached: Int32 = 0
            
            let result = url.path.withCString { pathCStr in
                anidb_cache_check_file(handle, pathCStr, algorithm.toCType(), &cached)
            }
            
            if result != ANIDB_SUCCESS {
                throw AniDBError(result: result)
            }
            
            return cached != 0
        }
    }
    
    // MARK: - Private Methods
    
    private static var libraryInitialized = false
    private static let libraryQueue = DispatchQueue(label: "com.anidb.library")
    
    private static func initializeLibrary() {
        libraryQueue.sync {
            guard !libraryInitialized else { return }
            
            let result = anidb_init(UInt32(ANIDB_ABI_VERSION))
            if result == ANIDB_SUCCESS {
                libraryInitialized = true
                
                // Register cleanup on app termination
                atexit {
                    anidb_cleanup()
                }
            }
        }
    }
    
    private func setupEventHandling() {
        guard let handle = handle else { return }
        
        // Create a retained reference for the callback
        let eventHandler = Unmanaged.passRetained(EventHandler { [weak self] event in
            self?.eventsContinuation.yield(event)
        })
        
        // Connect to event system
        let result = anidb_event_connect(handle, eventCallback, eventHandler.toOpaque())
        
        if result != ANIDB_SUCCESS {
            // Clean up retained reference on failure
            eventHandler.release()
        }
    }
}

// MARK: - Progress Callback

private class ProgressContext {
    let handler: (AniDBClient.Progress) -> Void
    
    init(handler: @escaping (AniDBClient.Progress) -> Void) {
        self.handler = handler
    }
}

private func progressCallback(
    percentage: Float,
    bytesProcessed: UInt64,
    totalBytes: UInt64,
    userData: UnsafeMutableRawPointer?
) {
    guard let userData = userData else { return }
    
    let context = Unmanaged<ProgressContext>.fromOpaque(userData).takeUnretainedValue()
    
    let progress = AniDBClient.Progress(
        percentage: percentage,
        bytesProcessed: bytesProcessed,
        totalBytes: totalBytes
    )
    
    context.handler(progress)
}

// MARK: - Event Callback

private class EventHandler {
    let handler: (AniDBClient.Event) -> Void
    
    init(handler: @escaping (AniDBClient.Event) -> Void) {
        self.handler = handler
    }
}

private func eventCallback(
    eventPtr: UnsafePointer<anidb_event_t>?,
    userData: UnsafeMutableRawPointer?
) {
    guard let eventPtr = eventPtr, let userData = userData else { return }
    
    let handler = Unmanaged<EventHandler>.fromOpaque(userData).takeUnretainedValue()
    let event = AniDBClient.Event(from: eventPtr.pointee)
    
    handler.handler(event)
}

// MARK: - Async Semaphore

private actor AsyncSemaphore {
    private var count: Int
    private var waiters: [CheckedContinuation<Void, Never>] = []
    
    init(count: Int) {
        self.count = count
    }
    
    func wait() async {
        if count > 0 {
            count -= 1
        } else {
            await withCheckedContinuation { continuation in
                waiters.append(continuation)
            }
        }
    }
    
    func signal() {
        if let waiter = waiters.first {
            waiters.removeFirst()
            waiter.resume()
        } else {
            count += 1
        }
    }
}