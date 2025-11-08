using System;
using System.IO;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using FluentAssertions;
using Xunit;

namespace AniDBClient.Tests
{
    public class ClientTests : IDisposable
    {
        private readonly string _testCacheDir;

        public ClientTests()
        {
            _testCacheDir = Path.Combine(Path.GetTempPath(), $"anidb_test_{Guid.NewGuid()}");
            Directory.CreateDirectory(_testCacheDir);
        }

        public void Dispose()
        {
            if (Directory.Exists(_testCacheDir))
            {
                Directory.Delete(_testCacheDir, true);
            }
        }

        [Fact]
        public void Constructor_WithDefaultConfig_CreatesClient()
        {
            // Act
            using var client = new AniDBClient();

            // Assert
            client.Should().NotBeNull();
        }

        [Fact]
        public void Constructor_WithCustomConfig_CreatesClient()
        {
            // Arrange
            var config = new ClientConfiguration
            {
                CacheDirectory = _testCacheDir,
                MaxConcurrentFiles = 8,
                ChunkSize = 128 * 1024,
                EnableDebugLogging = true
            };

            // Act
            using var client = new AniDBClient(config);

            // Assert
            client.Should().NotBeNull();
        }

        [Fact]
        public void Constructor_WithNullConfig_ThrowsArgumentNullException()
        {
            // Act
            Action act = () => new AniDBClient(null!);

            // Assert
            act.Should().Throw<ArgumentNullException>()
                .WithParameterName("configuration");
        }

        [Fact]
        public void ProcessFile_WithNullPath_ThrowsArgumentException()
        {
            // Arrange
            using var client = new AniDBClient();

            // Act
            Action act = () => client.ProcessFile(null!);

            // Assert
            act.Should().Throw<ArgumentException>()
                .WithParameterName("filePath");
        }

        [Fact]
        public void ProcessFile_WithEmptyPath_ThrowsArgumentException()
        {
            // Arrange
            using var client = new AniDBClient();

            // Act
            Action act = () => client.ProcessFile(string.Empty);

            // Assert
            act.Should().Throw<ArgumentException>()
                .WithParameterName("filePath");
        }

        [Fact]
        public async Task ProcessFileAsync_WithNullPath_ThrowsArgumentException()
        {
            // Arrange
            using var client = new AniDBClient();

            // Act
            Func<Task> act = async () => await client.ProcessFileAsync(null!);

            // Assert
            await act.Should().ThrowAsync<ArgumentException>()
                .WithParameterName("filePath");
        }

        [Fact]
        public void CalculateHash_WithValidData_ReturnsHash()
        {
            // Arrange
            using var client = new AniDBClient();
            var data = new byte[] { 1, 2, 3, 4, 5 };

            // Act & Assert - should not throw
            Action act = () => client.CalculateHash(data, HashAlgorithm.MD5);
            act.Should().NotThrow();
        }

        [Fact]
        public void CalculateHash_WithNullData_ThrowsArgumentNullException()
        {
            // Arrange
            using var client = new AniDBClient();

            // Act
            Action act = () => client.CalculateHash((byte[])null!, HashAlgorithm.MD5);

            // Assert
            act.Should().Throw<ArgumentNullException>()
                .WithParameterName("data");
        }

        [Fact]
        public void ClearCache_DoesNotThrow()
        {
            // Arrange
            using var client = new AniDBClient();

            // Act & Assert
            Action act = () => client.ClearCache();
            act.Should().NotThrow();
        }

        [Fact]
        public void GetCacheStatistics_ReturnsStatistics()
        {
            // Arrange
            using var client = new AniDBClient();

            // Act
            var stats = client.GetCacheStatistics();

            // Assert
            stats.Should().NotBeNull();
            stats.TotalEntries.Should().BeGreaterOrEqualTo(0);
            stats.SizeInBytes.Should().BeGreaterOrEqualTo(0);
        }

        [Fact]
        public void IsFileCached_WithValidPath_ReturnsBool()
        {
            // Arrange
            using var client = new AniDBClient();
            var testFile = Path.GetTempFileName();

            try
            {
                // Act
                var result = client.IsFileCached(testFile, HashAlgorithm.ED2K);

                // Assert
                result.Should().BeFalse(); // New file shouldn't be cached
            }
            finally
            {
                File.Delete(testFile);
            }
        }

        [Fact]
        public async Task ProcessBatchAsync_WithEmptyPaths_ThrowsArgumentException()
        {
            // Arrange
            using var client = new AniDBClient();

            // Act
            Func<Task> act = async () => await client.ProcessBatchAsync(Array.Empty<string>());

            // Assert
            await act.Should().ThrowAsync<ArgumentException>()
                .WithParameterName("filePaths");
        }

        [Fact]
        public async Task ProcessBatchAsync_WithNullPaths_ThrowsArgumentNullException()
        {
            // Arrange
            using var client = new AniDBClient();

            // Act
            Func<Task> act = async () => await client.ProcessBatchAsync(null!);

            // Assert
            await act.Should().ThrowAsync<ArgumentNullException>()
                .WithParameterName("filePaths");
        }

        [Fact]
        public void GetVersion_ReturnsVersionString()
        {
            // Act
            var version = AniDBClient.GetVersion();

            // Assert
            version.Should().NotBeNullOrEmpty();
        }

        [Fact]
        public void GetAbiVersion_ReturnsVersionNumber()
        {
            // Act
            var version = AniDBClient.GetAbiVersion();

            // Assert
            version.Should().BeGreaterThan(0);
        }

        [Fact]
        public void Dispose_MultipleTimes_DoesNotThrow()
        {
            // Arrange
            var client = new AniDBClient();

            // Act & Assert
            client.Dispose();
            Action act = () => client.Dispose();
            act.Should().NotThrow();
        }

        [Fact]
        public async Task DisposeAsync_MultipleTimes_DoesNotThrow()
        {
            // Arrange
            var client = new AniDBClient();

            // Act & Assert
            await client.DisposeAsync();
            Func<Task> act = async () => await client.DisposeAsync();
            await act.Should().NotThrowAsync();
        }

        [Fact]
        public void AfterDispose_MethodsThrowObjectDisposedException()
        {
            // Arrange
            var client = new AniDBClient();
            client.Dispose();

            // Act & Assert
            Action act1 = () => client.ProcessFile("test.txt");
            act1.Should().Throw<ObjectDisposedException>();

            Action act2 = () => client.ClearCache();
            act2.Should().Throw<ObjectDisposedException>();

            Action act3 = () => client.GetCacheStatistics();
            act3.Should().Throw<ObjectDisposedException>();
        }

        [Fact]
        public async Task ProcessFileAsync_WithCancellation_ThrowsOperationCancelledException()
        {
            // Arrange
            using var client = new AniDBClient();
            using var cts = new CancellationTokenSource();
            cts.Cancel();

            // Create a test file
            var testFile = Path.GetTempFileName();
            File.WriteAllBytes(testFile, new byte[1024]);

            try
            {
                // Act
                Func<Task> act = async () => await client.ProcessFileAsync(
                    testFile, 
                    cancellationToken: cts.Token);

                // Assert
                await act.Should().ThrowAsync<OperationCancelledException>();
            }
            finally
            {
                File.Delete(testFile);
            }
        }

        [Fact]
        public void ProcessingOptions_DefaultValues_AreCorrect()
        {
            // Arrange & Act
            var options = new ProcessingOptions();

            // Assert
            options.Algorithms.Should().ContainSingle()
                .Which.Should().Be(HashAlgorithm.ED2K);
            options.EnableProgress.Should().BeTrue();
            options.VerifyExisting.Should().BeFalse();
            options.ProgressCallback.Should().BeNull();
        }

        [Fact]
        public void BatchOptions_DefaultValues_AreCorrect()
        {
            // Arrange & Act
            var options = new BatchOptions();

            // Assert
            options.MaxConcurrent.Should().Be(4);
            options.ContinueOnError.Should().BeTrue();
            options.SkipExisting.Should().BeFalse();
        }

        [Fact]
        public async Task IdentifyFileAsync_WithInvalidHash_ReturnsNull()
        {
            // Arrange
            using var client = new AniDBClient();

            // Act
            var result = await client.IdentifyFileAsync("invalid_hash", 12345);

            // Assert
            result.Should().BeNull();
        }

        [Fact]
        public async Task IdentifyFileAsync_WithNullHash_ThrowsArgumentException()
        {
            // Arrange
            using var client = new AniDBClient();

            // Act
            Func<Task> act = async () => await client.IdentifyFileAsync(null!, 12345);

            // Assert
            await act.Should().ThrowAsync<ArgumentException>()
                .WithParameterName("ed2kHash");
        }

        [Fact]
        public async Task IdentifyFileAsync_WithEmptyHash_ThrowsArgumentException()
        {
            // Arrange
            using var client = new AniDBClient();

            // Act
            Func<Task> act = async () => await client.IdentifyFileAsync(string.Empty, 12345);

            // Assert
            await act.Should().ThrowAsync<ArgumentException>()
                .WithParameterName("ed2kHash");
        }
    }
}