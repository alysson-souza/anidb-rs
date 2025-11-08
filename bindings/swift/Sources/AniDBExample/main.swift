import Foundation
import AniDBClient

// MARK: - Example Application

@main
@available(macOS 13.0, *)
struct AniDBExample {
    static func main() async {
        print("AniDB Client Swift Example")
        print("==========================\n")
        
        do {
            // Create client with custom configuration
            let config = AniDBClient.Configuration(
                cacheDirectory: FileManager.default.temporaryDirectory.appendingPathComponent("anidb_example_cache"),
                maxConcurrentFiles: 4,
                enableDebugLogging: true
            )
            
            let client = try AniDBClient(configuration: config)
            
            // Example 1: Process a single file
            await processingSingleFileExample(client: client)
            
            // Example 2: Batch processing
            await batchProcessingExample(client: client)
            
            // Example 3: Event monitoring
            await eventMonitoringExample(client: client)
            
            // Example 4: Cache management
            await cacheManagementExample(client: client)
            
            // Example 5: Direct hash calculation
            await hashCalculationExample(client: client)
            
        } catch {
            print("Error: \(error)")
        }
    }
    
    // MARK: - Example 1: Single File Processing
    
    static func processingSingleFileExample(client: AniDBClient) async {
        print("\n📁 Example 1: Processing Single File")
        print("------------------------------------")
        
        // Create a test file
        let testFile = FileManager.default.temporaryDirectory.appendingPathComponent("example_video.mkv")
        let testData = Data(repeating: 0xAB, count: 5 * 1024 * 1024) // 5MB test file
        
        do {
            try testData.write(to: testFile)
            defer { try? FileManager.default.removeItem(at: testFile) }
            
            print("Processing: \(testFile.lastPathComponent)")
            print("File size: \(ByteCountFormatter.string(fromByteCount: Int64(testData.count), countStyle: .file))")
            
            var lastProgress: Float = 0
            
            let result = try await client.processFile(
                at: testFile,
                algorithms: [.ed2k, .md5, .crc32],
                progress: { progress in
                    // Only print significant progress updates
                    if progress.percentage - lastProgress > 10 {
                        print("Progress: \(String(format: "%.0f%%", progress.percentage))")
                        lastProgress = progress.percentage
                    }
                }
            )
            
            print("\n✅ Processing completed!")
            print("Processing time: \(String(format: "%.2f", result.processingTime))s")
            print("\nCalculated hashes:")
            for (algorithm, hash) in result.hashes.sorted(by: { $0.key.name < $1.key.name }) {
                print("  \(algorithm.name): \(hash)")
            }
            
        } catch {
            print("❌ Error processing file: \(error)")
        }
    }
    
    // MARK: - Example 2: Batch Processing
    
    static func batchProcessingExample(client: AniDBClient) async {
        print("\n📦 Example 2: Batch Processing")
        print("------------------------------")
        
        // Create multiple test files
        var testFiles: [URL] = []
        for i in 1...3 {
            let url = FileManager.default.temporaryDirectory
                .appendingPathComponent("episode_\(i).mkv")
            let data = Data(repeating: UInt8(i), count: 2 * 1024 * 1024) // 2MB each
            do {
                try data.write(to: url)
                testFiles.append(url)
            } catch {
                print("Failed to create test file: \(error)")
            }
        }
        
        defer {
            for url in testFiles {
                try? FileManager.default.removeItem(at: url)
            }
        }
        
        print("Processing \(testFiles.count) files...")
        
        do {
            let result = try await client.processBatch(
                urls: testFiles,
                algorithms: [.ed2k],
                maxConcurrent: 2,
                progress: { progress in
                    print("Batch progress: \(progress.completedFiles)/\(progress.totalFiles) files")
                    if let current = progress.currentFile {
                        print("  Currently processing: \(current.lastPathComponent)")
                    }
                }
            )
            
            print("\n✅ Batch processing completed!")
            print("Success rate: \(String(format: "%.0f%%", result.successRate * 100))")
            print("Total files: \(result.totalFiles)")
            print("Successful: \(result.successful.count)")
            print("Failed: \(result.failed.count)")
            
            if !result.failed.isEmpty {
                print("\nFailed files:")
                for (url, error) in result.failed {
                    print("  - \(url.lastPathComponent): \(error)")
                }
            }
            
        } catch {
            print("❌ Batch processing error: \(error)")
        }
    }
    
    // MARK: - Example 3: Event Monitoring
    
    static func eventMonitoringExample(client: AniDBClient) async {
        print("\n📊 Example 3: Event Monitoring")
        print("------------------------------")
        
        // Create test file
        let testFile = FileManager.default.temporaryDirectory.appendingPathComponent("event_test.mkv")
        let testData = Data(repeating: 0xEF, count: 1024 * 1024) // 1MB
        
        do {
            try testData.write(to: testFile)
            defer { try? FileManager.default.removeItem(at: testFile) }
            
            // Start monitoring events
            let eventTask = Task {
                print("Starting event monitor...")
                var eventCount = 0
                
                for await event in client.events.stream {
                    eventCount += 1
                    
                    switch event {
                    case .fileStart(let url, let size):
                        print("📄 File start: \(url.lastPathComponent) (\(ByteCountFormatter.string(fromByteCount: Int64(size), countStyle: .file)))")
                        
                    case .fileComplete(let url, _, let duration):
                        print("✓ File complete: \(url.lastPathComponent) (took \(String(format: "%.2f", duration))s)")
                        
                    case .hashStart(let algorithm):
                        print("🔐 Hash start: \(algorithm.name)")
                        
                    case .hashComplete(let algorithm, let hash):
                        print("✓ Hash complete: \(algorithm.name) = \(String(hash.prefix(16)))...")
                        
                    case .cacheHit(let url, let algorithm):
                        print("💾 Cache hit: \(url.lastPathComponent) [\(algorithm.name)]")
                        
                    case .cacheMiss(let url, let algorithm):
                        print("❌ Cache miss: \(url.lastPathComponent) [\(algorithm.name)]")
                        
                    case .memoryWarning(let current, let max):
                        print("⚠️ Memory warning: \(current)/\(max) bytes")
                        
                    default:
                        print("📍 Event: \(event)")
                    }
                    
                    // Stop after processing is done
                    if eventCount > 5 {
                        break
                    }
                }
            }
            
            // Process file to generate events
            _ = try await client.processFile(at: testFile, algorithms: [.ed2k, .md5])
            
            // Wait a bit for events to be processed
            try await Task.sleep(nanoseconds: 500_000_000) // 0.5 seconds
            
            eventTask.cancel()
            
        } catch {
            print("❌ Event monitoring error: \(error)")
        }
    }
    
    // MARK: - Example 4: Cache Management
    
    static func cacheManagementExample(client: AniDBClient) async {
        print("\n💾 Example 4: Cache Management")
        print("------------------------------")
        
        do {
            // Check initial cache statistics
            let initialStats = try client.cacheStatistics()
            print("Initial cache state:")
            print("  Entries: \(initialStats.totalEntries)")
            print("  Size: \(initialStats.formattedSize)")
            
            // Create and process a file
            let testFile = FileManager.default.temporaryDirectory.appendingPathComponent("cache_test.mkv")
            let testData = Data(repeating: 0xCD, count: 3 * 1024 * 1024) // 3MB
            try testData.write(to: testFile)
            defer { try? FileManager.default.removeItem(at: testFile) }
            
            // First processing (should miss cache)
            print("\nFirst processing (expecting cache miss)...")
            _ = try await client.processFile(at: testFile, algorithms: [.ed2k])
            
            // Check if cached
            let isCached = try client.isCached(url: testFile, algorithm: .ed2k)
            print("File is cached: \(isCached)")
            
            // Check cache statistics after processing
            let afterStats = try client.cacheStatistics()
            print("\nCache after processing:")
            print("  Entries: \(afterStats.totalEntries)")
            print("  Size: \(afterStats.formattedSize)")
            
            // Second processing (should hit cache)
            print("\nSecond processing (expecting cache hit)...")
            let start = Date()
            _ = try await client.processFile(at: testFile, algorithms: [.ed2k])
            let cacheTime = Date().timeIntervalSince(start)
            print("Cache hit processing time: \(String(format: "%.3f", cacheTime))s")
            
            // Clear cache
            print("\nClearing cache...")
            try client.clearCache()
            
            let clearedStats = try client.cacheStatistics()
            print("Cache after clearing:")
            print("  Entries: \(clearedStats.totalEntries)")
            print("  Size: \(clearedStats.formattedSize)")
            
        } catch {
            print("❌ Cache management error: \(error)")
        }
    }
    
    // MARK: - Example 5: Direct Hash Calculation
    
    static func hashCalculationExample(client: AniDBClient) async {
        print("\n#️⃣ Example 5: Direct Hash Calculation")
        print("--------------------------------------")
        
        let testString = "Hello, AniDB Swift bindings!"
        let testData = testString.data(using: .utf8)!
        
        print("Test string: \"\(testString)\"")
        print("Data size: \(testData.count) bytes")
        print("\nCalculating hashes:")
        
        do {
            for algorithm in AniDBClient.HashAlgorithm.allCases {
                let hash = try client.calculateHash(for: testData, algorithm: algorithm)
                print("  \(algorithm.name): \(hash)")
            }
        } catch {
            print("❌ Hash calculation error: \(error)")
        }
    }
}

// MARK: - Helpers

extension ByteCountFormatter {
    static func string(fromByteCount byteCount: Int64, countStyle: CountStyle) -> String {
        let formatter = ByteCountFormatter()
        formatter.countStyle = countStyle
        return formatter.string(fromByteCount: byteCount)
    }
}