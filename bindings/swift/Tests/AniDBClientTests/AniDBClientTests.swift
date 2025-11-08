import XCTest
@testable import AniDBClient

@available(macOS 13.0, *)
final class AniDBClientTests: XCTestCase {
    
    var client: AniDBClient!
    var testFileURL: URL!
    
    override func setUpWithError() throws {
        // Create test client with custom config
        let config = AniDBClient.Configuration(
            cacheDirectory: FileManager.default.temporaryDirectory.appendingPathComponent("anidb_test_cache"),
            maxConcurrentFiles: 2,
            enableDebugLogging: true
        )
        
        client = try AniDBClient(configuration: config)
        
        // Create test file
        testFileURL = FileManager.default.temporaryDirectory.appendingPathComponent("test_file.bin")
        let testData = Data(repeating: 0xAB, count: 1024 * 1024) // 1MB test file
        try testData.write(to: testFileURL)
    }
    
    override func tearDownWithError() throws {
        // Clean up test file
        try? FileManager.default.removeItem(at: testFileURL)
        
        // Clean up cache
        try? client.clearCache()
        
        client = nil
    }
    
    // MARK: - Initialization Tests
    
    func testDefaultInitialization() throws {
        let defaultClient = try AniDBClient()
        XCTAssertNotNil(defaultClient)
        XCTAssertEqual(defaultClient.configuration, .default)
    }
    
    func testCustomConfiguration() throws {
        let customConfig = AniDBClient.Configuration(
            maxConcurrentFiles: 8,
            chunkSize: 128 * 1024,
            enableDebugLogging: true
        )
        
        let customClient = try AniDBClient(configuration: customConfig)
        XCTAssertEqual(customClient.configuration.maxConcurrentFiles, 8)
        XCTAssertEqual(customClient.configuration.chunkSize, 128 * 1024)
        XCTAssertTrue(customClient.configuration.enableDebugLogging)
    }
    
    // MARK: - File Processing Tests
    
    func testProcessSingleFile() async throws {
        let result = try await client.processFile(
            at: testFileURL,
            algorithms: [.ed2k, .md5]
        )
        
        XCTAssertEqual(result.url, testFileURL)
        XCTAssertEqual(result.fileSize, 1024 * 1024)
        XCTAssertEqual(result.status, .completed)
        XCTAssertNotNil(result.hashes[.ed2k])
        XCTAssertNotNil(result.hashes[.md5])
        XCTAssertNil(result.errorMessage)
    }
    
    func testProcessFileWithProgress() async throws {
        var progressUpdates: [AniDBClient.Progress] = []
        
        let result = try await client.processFile(
            at: testFileURL,
            algorithms: [.ed2k],
            progress: { progress in
                progressUpdates.append(progress)
            }
        )
        
        XCTAssertEqual(result.status, .completed)
        XCTAssertFalse(progressUpdates.isEmpty)
        
        // Verify progress updates are in order
        for i in 1..<progressUpdates.count {
            XCTAssertGreaterThanOrEqual(
                progressUpdates[i].bytesProcessed,
                progressUpdates[i-1].bytesProcessed
            )
        }
    }
    
    func testProcessNonExistentFile() async {
        let nonExistentURL = URL(fileURLWithPath: "/non/existent/file.txt")
        
        do {
            _ = try await client.processFile(at: nonExistentURL)
            XCTFail("Expected error for non-existent file")
        } catch {
            XCTAssertTrue(error is AniDBError)
        }
    }
    
    // MARK: - Batch Processing Tests
    
    func testBatchProcessing() async throws {
        // Create multiple test files
        var testFiles: [URL] = []
        for i in 0..<3 {
            let url = FileManager.default.temporaryDirectory
                .appendingPathComponent("batch_test_\(i).bin")
            let data = Data(repeating: UInt8(i), count: 512 * 1024) // 512KB each
            try data.write(to: url)
            testFiles.append(url)
        }
        
        defer {
            // Clean up
            for url in testFiles {
                try? FileManager.default.removeItem(at: url)
            }
        }
        
        var progressUpdates: [AniDBClient.BatchProgress] = []
        
        let result = try await client.processBatch(
            urls: testFiles,
            algorithms: [.ed2k, .crc32],
            maxConcurrent: 2,
            progress: { progress in
                progressUpdates.append(progress)
            }
        )
        
        XCTAssertEqual(result.successful.count, 3)
        XCTAssertEqual(result.failed.count, 0)
        XCTAssertEqual(result.totalFiles, 3)
        XCTAssertEqual(result.successRate, 1.0)
        
        // Verify all files have hashes
        for fileResult in result.successful {
            XCTAssertNotNil(fileResult.hashes[.ed2k])
            XCTAssertNotNil(fileResult.hashes[.crc32])
        }
        
        // Verify progress updates
        XCTAssertFalse(progressUpdates.isEmpty)
        if let lastProgress = progressUpdates.last {
            XCTAssertEqual(lastProgress.completedFiles, 3)
        }
    }
    
    // MARK: - Hash Calculation Tests
    
    func testCalculateHashForData() throws {
        let testData = "Hello, AniDB!".data(using: .utf8)!
        
        let md5Hash = try client.calculateHash(for: testData, algorithm: .md5)
        XCTAssertFalse(md5Hash.isEmpty)
        
        let crc32Hash = try client.calculateHash(for: testData, algorithm: .crc32)
        XCTAssertFalse(crc32Hash.isEmpty)
        
        // Verify different algorithms produce different hashes
        XCTAssertNotEqual(md5Hash, crc32Hash)
    }
    
    // MARK: - Cache Tests
    
    func testCacheOperations() async throws {
        // Process a file to populate cache
        _ = try await client.processFile(at: testFileURL, algorithms: [.ed2k])
        
        // Check cache statistics
        let stats = try client.cacheStatistics()
        XCTAssertGreaterThan(stats.totalEntries, 0)
        XCTAssertGreaterThan(stats.sizeInBytes, 0)
        
        // Check if file is cached
        let isCached = try client.isCached(url: testFileURL, algorithm: .ed2k)
        XCTAssertTrue(isCached)
        
        // Clear cache
        try client.clearCache()
        
        // Verify cache is empty
        let newStats = try client.cacheStatistics()
        XCTAssertEqual(newStats.totalEntries, 0)
    }
    
    // MARK: - Event Stream Tests
    
    func testEventStream() async throws {
        let expectation = XCTestExpectation(description: "Receive events")
        var receivedEvents: [AniDBClient.Event] = []
        
        // Start listening to events
        Task {
            for await event in client.events.stream {
                receivedEvents.append(event)
                
                // Stop after receiving a few events
                if receivedEvents.count >= 2 {
                    expectation.fulfill()
                    break
                }
            }
        }
        
        // Process a file to generate events
        _ = try await client.processFile(at: testFileURL, algorithms: [.ed2k])
        
        await fulfillment(of: [expectation], timeout: 5.0)
        
        // Verify we received events
        XCTAssertFalse(receivedEvents.isEmpty)
        
        // Check for file start event
        XCTAssertTrue(receivedEvents.contains { event in
            if case .fileStart = event { return true }
            return false
        })
    }
    
    // MARK: - Error Handling Tests
    
    func testErrorDescriptions() {
        let errors: [AniDBError] = [
            .fileNotFound(path: "/test/path"),
            .outOfMemory,
            .networkError(message: "Connection failed"),
            .permissionDenied(path: "/restricted/file")
        ]
        
        for error in errors {
            XCTAssertNotNil(error.errorDescription)
            XCTAssertNotNil(error.recoverySuggestion)
        }
    }
    
    // MARK: - Objective-C Bridge Tests
    
    func testObjectiveCBridge() throws {
        let objcClient = ObjCAniDBClient()
        
        let expectation = XCTestExpectation(description: "ObjC processing")
        
        objcClient.processFile(
            at: testFileURL.path,
            algorithms: [NSNumber(value: ObjCHashAlgorithm.ed2k.rawValue)],
            progressHandler: nil
        ) { result, error in
            XCTAssertNotNil(result)
            XCTAssertNil(error)
            XCTAssertEqual(result?.status, .completed)
            expectation.fulfill()
        }
        
        wait(for: [expectation], timeout: 5.0)
    }
    
    // MARK: - Performance Tests
    
    func testLargeFilePerformance() async throws {
        // Create a larger test file (10MB)
        let largeFileURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("large_test.bin")
        let largeData = Data(repeating: 0xFF, count: 10 * 1024 * 1024)
        try largeData.write(to: largeFileURL)
        
        defer {
            try? FileManager.default.removeItem(at: largeFileURL)
        }
        
        let startTime = Date()
        
        let result = try await client.processFile(
            at: largeFileURL,
            algorithms: [.ed2k, .md5, .sha1]
        )
        
        let elapsed = Date().timeIntervalSince(startTime)
        
        XCTAssertEqual(result.status, .completed)
        XCTAssertEqual(result.hashes.count, 3)
        
        // Performance assertion - should process 10MB in reasonable time
        XCTAssertLessThan(elapsed, 2.0, "Processing 10MB took too long: \(elapsed)s")
        
        print("Processed 10MB in \(elapsed)s")
        print("Throughput: \(10.0 / elapsed) MB/s")
    }
    
    // MARK: - Concurrency Tests
    
    func testConcurrentOperations() async throws {
        // Create multiple test files
        var urls: [URL] = []
        for i in 0..<5 {
            let url = FileManager.default.temporaryDirectory
                .appendingPathComponent("concurrent_\(i).bin")
            let data = Data(repeating: UInt8(i), count: 1024 * 1024)
            try data.write(to: url)
            urls.append(url)
        }
        
        defer {
            for url in urls {
                try? FileManager.default.removeItem(at: url)
            }
        }
        
        // Process files concurrently
        let results = try await withThrowingTaskGroup(of: AniDBClient.FileResult.self) { group in
            for url in urls {
                group.addTask { [weak self] in
                    guard let self = self else { throw AniDBError.clientDestroyed }
                    return try await self.client.processFile(at: url, algorithms: [.ed2k])
                }
            }
            
            var results: [AniDBClient.FileResult] = []
            for try await result in group {
                results.append(result)
            }
            return results
        }
        
        XCTAssertEqual(results.count, 5)
        for result in results {
            XCTAssertEqual(result.status, .completed)
            XCTAssertNotNil(result.hashes[.ed2k])
        }
    }
}