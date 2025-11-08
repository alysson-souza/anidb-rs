using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using FluentAssertions;
using Xunit;

namespace AniDBClient.Tests
{
    [Collection("Integration")]
    public class IntegrationTests : IDisposable
    {
        private readonly string _testDir;
        private readonly List<string> _testFiles = new();

        public IntegrationTests()
        {
            _testDir = Path.Combine(Path.GetTempPath(), $"anidb_integration_{Guid.NewGuid()}");
            Directory.CreateDirectory(_testDir);
            CreateTestFiles();
        }

        public void Dispose()
        {
            if (Directory.Exists(_testDir))
            {
                Directory.Delete(_testDir, true);
            }
        }

        private void CreateTestFiles()
        {
            // Create small test files with different content
            for (int i = 0; i < 5; i++)
            {
                var filePath = Path.Combine(_testDir, $"test_{i}.dat");
                var content = new byte[1024 * (i + 1)]; // Different sizes
                new Random(i).NextBytes(content);
                File.WriteAllBytes(filePath, content);
                _testFiles.Add(filePath);
            }
        }

        [Fact]
        public async Task ProcessFile_WithRealFile_CalculatesHashes()
        {
            // Arrange
            using var client = new AniDBClient();
            var options = new ProcessingOptions
            {
                Algorithms = new[] { HashAlgorithm.ED2K, HashAlgorithm.MD5, HashAlgorithm.CRC32 }
            };

            // Act
            var result = await client.ProcessFileAsync(_testFiles[0], options);

            // Assert
            result.Should().NotBeNull();
            result.FilePath.Should().Be(_testFiles[0]);
            result.FileSize.Should().Be(1024);
            result.Status.Should().Be(ProcessingStatus.Completed);
            result.Hashes.Should().HaveCount(3);
            result.ProcessingTime.Should().BeGreaterThan(TimeSpan.Zero);
            result.ErrorMessage.Should().BeNull();
        }

        [Fact]
        public async Task ProcessFile_WithProgress_ReportsProgress()
        {
            // Arrange
            using var client = new AniDBClient();
            var progressReports = new List<ProgressInfo>();
            
            var options = new ProcessingOptions
            {
                EnableProgress = true,
                ProgressCallback = info => progressReports.Add(info)
            };

            // Create a larger file for progress reporting
            var largeFile = Path.Combine(_testDir, "large.dat");
            File.WriteAllBytes(largeFile, new byte[10 * 1024 * 1024]); // 10MB

            // Act
            await client.ProcessFileAsync(largeFile, options);

            // Assert
            progressReports.Should().NotBeEmpty();
            progressReports.Should().BeInAscendingOrder(p => p.Percentage);
            progressReports.Last().Percentage.Should().BeApproximately(100.0f, 0.1f);
        }

        [Fact]
        public async Task ProcessBatch_WithMultipleFiles_ProcessesAll()
        {
            // Arrange
            using var client = new AniDBClient();
            var options = new BatchOptions
            {
                Algorithms = new[] { HashAlgorithm.ED2K },
                MaxConcurrent = 2,
                ContinueOnError = true
            };

            // Act
            var result = await client.ProcessBatchAsync(_testFiles, options);

            // Assert
            result.Should().NotBeNull();
            result.TotalFiles.Should().Be(_testFiles.Count);
            result.SuccessfulFiles.Should().BeGreaterThan(0);
            result.TotalTime.Should().BeGreaterThan(TimeSpan.Zero);
        }

        [Fact]
        public void ProcessFile_Synchronous_Works()
        {
            // Arrange
            using var client = new AniDBClient();
            var options = new ProcessingOptions
            {
                Algorithms = new[] { HashAlgorithm.ED2K }
            };

            // Act
            var result = client.ProcessFile(_testFiles[0], options);

            // Assert
            result.Should().NotBeNull();
            result.Status.Should().Be(ProcessingStatus.Completed);
            result.Hashes.Should().ContainSingle()
                .Which.Algorithm.Should().Be(HashAlgorithm.ED2K);
        }

        [Fact]
        public void CalculateHash_ForFile_ProducesCorrectHash()
        {
            // Arrange
            using var client = new AniDBClient();

            // Act
            var hash = client.CalculateHash(_testFiles[0], HashAlgorithm.MD5);

            // Assert
            hash.Should().NotBeNullOrEmpty();
            hash.Should().MatchRegex("^[a-fA-F0-9]{32}$"); // MD5 is 32 hex chars
        }

        [Fact]
        public void CalculateHash_ForBuffer_ProducesCorrectHash()
        {
            // Arrange
            using var client = new AniDBClient();
            var data = new byte[] { 1, 2, 3, 4, 5 };

            // Act
            var hash = client.CalculateHash(data, HashAlgorithm.CRC32);

            // Assert
            hash.Should().NotBeNullOrEmpty();
            hash.Should().MatchRegex("^[a-fA-F0-9]{8}$"); // CRC32 is 8 hex chars
        }

        [Fact]
        public async Task Cache_SecondProcessing_IsFromCache()
        {
            // Arrange
            using var client = new AniDBClient();
            var options = new ProcessingOptions
            {
                Algorithms = new[] { HashAlgorithm.ED2K }
            };

            // Act
            var result1 = await client.ProcessFileAsync(_testFiles[0], options);
            var result2 = await client.ProcessFileAsync(_testFiles[0], options);

            // Assert
            result1.Should().NotBeNull();
            result2.Should().NotBeNull();
            
            // Second result should be much faster (from cache)
            result2.ProcessingTime.Should().BeLessThan(result1.ProcessingTime);
        }

        [Fact]
        public void IsFileCached_AfterProcessing_ReturnsTrue()
        {
            // Arrange
            using var client = new AniDBClient();
            var options = new ProcessingOptions
            {
                Algorithms = new[] { HashAlgorithm.ED2K }
            };

            // Act
            var beforeCache = client.IsFileCached(_testFiles[0], HashAlgorithm.ED2K);
            client.ProcessFile(_testFiles[0], options);
            var afterCache = client.IsFileCached(_testFiles[0], HashAlgorithm.ED2K);

            // Assert
            beforeCache.Should().BeFalse();
            afterCache.Should().BeTrue();
        }

        [Fact]
        public async Task Events_DuringProcessing_AreFired()
        {
            // Arrange
            using var client = new AniDBClient();
            var events = new List<ProcessingEvent>();
            
            client.EventReceived += (sender, evt) => events.Add(evt);

            // Act
            await client.ProcessFileAsync(_testFiles[0]);

            // Assert
            events.Should().NotBeEmpty();
            events.Should().Contain(e => e.Type == EventType.FileStart);
            events.Should().Contain(e => e.Type == EventType.FileComplete);
        }

        [Fact]
        public void MultipleAlgorithms_ProduceDifferentHashes()
        {
            // Arrange
            using var client = new AniDBClient();
            var data = File.ReadAllBytes(_testFiles[0]);

            // Act
            var ed2k = client.CalculateHash(data, HashAlgorithm.ED2K);
            var md5 = client.CalculateHash(data, HashAlgorithm.MD5);
            var sha1 = client.CalculateHash(data, HashAlgorithm.SHA1);

            // Assert
            ed2k.Should().NotBe(md5);
            md5.Should().NotBe(sha1);
            sha1.Should().NotBe(ed2k);
        }

        [Fact]
        public async Task ProcessFile_NonExistentFile_ThrowsFileNotFoundException()
        {
            // Arrange
            using var client = new AniDBClient();
            var nonExistentFile = Path.Combine(_testDir, "does_not_exist.txt");

            // Act
            Func<Task> act = async () => await client.ProcessFileAsync(nonExistentFile);

            // Assert
            await act.Should().ThrowAsync<FileNotFoundException>();
        }

        [Fact]
        public void ClearCache_RemovesCachedData()
        {
            // Arrange
            using var client = new AniDBClient();
            
            // Process a file to cache it
            client.ProcessFile(_testFiles[0]);
            var cachedBefore = client.IsFileCached(_testFiles[0], HashAlgorithm.ED2K);

            // Act
            client.ClearCache();
            var cachedAfter = client.IsFileCached(_testFiles[0], HashAlgorithm.ED2K);

            // Assert
            cachedBefore.Should().BeTrue();
            cachedAfter.Should().BeFalse();
        }
    }
}