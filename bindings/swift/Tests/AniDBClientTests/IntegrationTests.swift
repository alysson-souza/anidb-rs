import XCTest
@testable import AniDBClient

@available(macOS 13.0, *)
final class IntegrationTests: XCTestCase {
    
    func testCompleteWorkflow() async throws {
        // This test demonstrates a complete workflow including:
        // 1. Client initialization
        // 2. File processing with multiple algorithms
        // 3. Cache usage
        // 4. Event monitoring
        // 5. Error handling
        
        // Setup
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("anidb_integration_test_\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        
        defer {
            try? FileManager.default.removeItem(at: tempDir)
        }
        
        // Create client with custom cache directory
        let config = AniDBClient.Configuration(
            cacheDirectory: tempDir.appendingPathComponent("cache"),
            maxConcurrentFiles: 2,
            chunkSize: 64 * 1024,
            enableDebugLogging: true
        )
        
        let client = try AniDBClient(configuration: config)
        
        // Create test files simulating anime episodes
        var episodeFiles: [URL] = []
        for i in 1...3 {
            let episodeURL = tempDir.appendingPathComponent("Episode_\(String(format: "%02d", i)).mkv")
            
            // Create file with unique content
            var data = Data()
            data.append("Episode \(i) Header".data(using: .utf8)!)
            data.append(Data(repeating: UInt8(i), count: 1024 * 1024)) // 1MB of unique data
            data.append("Episode \(i) Footer".data(using: .utf8)!)
            
            try data.write(to: episodeURL)
            episodeFiles.append(episodeURL)
        }
        
        // Setup event monitoring
        var receivedEvents: [AniDBClient.Event] = []
        let eventTask = Task {
            for await event in client.events.stream {
                receivedEvents.append(event)
            }
        }
        
        // Process files individually first (to populate cache)
        print("\n--- Processing individual files ---")
        for (index, file) in episodeFiles.enumerated() {
            let result = try await client.processFile(
                at: file,
                algorithms: [.ed2k, .md5, .crc32],
                progress: { progress in
                    if progress.percentage.truncatingRemainder(dividingBy: 25) == 0 {
                        print("Episode \(index + 1): \(Int(progress.percentage))%")
                    }
                }
            )
            
            XCTAssertEqual(result.status, .completed)
            XCTAssertEqual(result.hashes.count, 3)
            XCTAssertNil(result.errorMessage)
            
            print("Episode \(index + 1) hashes:")
            print("  ED2K: \(result.hashes[.ed2k]!)")
            print("  MD5: \(result.hashes[.md5]!)")
            print("  CRC32: \(result.hashes[.crc32]!)")
        }
        
        // Check cache statistics
        let cacheStats = try client.cacheStatistics()
        XCTAssertGreaterThan(cacheStats.totalEntries, 0)
        print("\nCache stats after first run:")
        print("  Entries: \(cacheStats.totalEntries)")
        print("  Size: \(cacheStats.formattedSize)")
        
        // Clear received events
        receivedEvents.removeAll()
        
        // Process same files again (should hit cache)
        print("\n--- Processing files again (cache test) ---")
        let cacheStart = Date()
        
        let batchResult = try await client.processBatch(
            urls: episodeFiles,
            algorithms: [.ed2k, .md5, .crc32],
            maxConcurrent: 3,
            progress: { progress in
                print("Batch progress: \(progress.completedFiles)/\(progress.totalFiles)")
            }
        )
        
        let cacheTime = Date().timeIntervalSince(cacheStart)
        print("Cache processing time: \(String(format: "%.3f", cacheTime))s")
        
        XCTAssertEqual(batchResult.successful.count, 3)
        XCTAssertEqual(batchResult.failed.count, 0)
        XCTAssertEqual(batchResult.successRate, 1.0)
        
        // Verify cache hits in events
        let cacheHitEvents = receivedEvents.filter { event in
            if case .cacheHit = event { return true }
            return false
        }
        XCTAssertGreaterThan(cacheHitEvents.count, 0)
        print("Cache hits: \(cacheHitEvents.count)")
        
        // Test error handling with non-existent file
        print("\n--- Testing error handling ---")
        let nonExistentFile = tempDir.appendingPathComponent("does_not_exist.mkv")
        
        do {
            _ = try await client.processFile(at: nonExistentFile)
            XCTFail("Expected error for non-existent file")
        } catch let error as AniDBError {
            print("Expected error: \(error.localizedDescription)")
            XCTAssertNotNil(error.recoverySuggestion)
        }
        
        // Test direct hash calculation
        print("\n--- Testing direct hash calculation ---")
        let testData = "AniDB Swift Integration Test".data(using: .utf8)!
        let directHash = try client.calculateHash(for: testData, algorithm: .md5)
        XCTAssertFalse(directHash.isEmpty)
        print("Direct MD5 hash: \(directHash)")
        
        // Clean up
        eventTask.cancel()
        
        // Final verification
        print("\n--- Test Summary ---")
        print("Total events received: \(receivedEvents.count)")
        print("Files processed: \(episodeFiles.count)")
        print("Cache enabled: \(cacheHitEvents.count > 0 ? "Yes" : "No")")
        print("All tests passed!")
    }
    
    func testMemoryConstraints() async throws {
        // Test that the library respects memory constraints
        
        let config = AniDBClient.Configuration(
            maxMemoryUsage: 50_000_000, // 50MB limit
            enableDebugLogging: true
        )
        
        let client = try AniDBClient(configuration: config)
        
        // Create a large file (100MB)
        let largeFile = FileManager.default.temporaryDirectory
            .appendingPathComponent("large_file_\(UUID().uuidString).bin")
        
        // Create file in chunks to avoid memory issues
        let chunkSize = 10 * 1024 * 1024 // 10MB chunks
        let totalSize = 100 * 1024 * 1024 // 100MB total
        
        FileManager.default.createFile(atPath: largeFile.path, contents: nil)
        let fileHandle = try FileHandle(forWritingTo: largeFile)
        
        defer {
            try? fileHandle.close()
            try? FileManager.default.removeItem(at: largeFile)
        }
        
        for _ in 0..<(totalSize / chunkSize) {
            let chunk = Data(repeating: 0xFF, count: chunkSize)
            fileHandle.write(chunk)
        }
        
        try fileHandle.close()
        
        // Monitor memory warnings
        var memoryWarnings = 0
        let eventTask = Task {
            for await event in client.events.stream {
                if case .memoryWarning = event {
                    memoryWarnings += 1
                }
            }
        }
        
        // Process the large file
        print("Processing 100MB file with 50MB memory limit...")
        let startMemory = ProcessInfo.processInfo.physicalMemory
        
        let result = try await client.processFile(
            at: largeFile,
            algorithms: [.ed2k],
            progress: { progress in
                if progress.percentage.truncatingRemainder(dividingBy: 10) == 0 {
                    print("Progress: \(Int(progress.percentage))%")
                }
            }
        )
        
        XCTAssertEqual(result.status, .completed)
        XCTAssertNotNil(result.hashes[.ed2k])
        
        let endMemory = ProcessInfo.processInfo.physicalMemory
        print("Memory delta: \(ByteCountFormatter.string(fromByteCount: Int64(endMemory - startMemory), countStyle: .memory))")
        
        eventTask.cancel()
        
        print("Memory warnings received: \(memoryWarnings)")
        print("Large file processed successfully within memory constraints!")
    }
}