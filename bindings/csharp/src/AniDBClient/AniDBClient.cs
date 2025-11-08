using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;
using AniDBClient.Native;

namespace AniDBClient
{
    /// <summary>
    /// High-performance client for AniDB file processing
    /// </summary>
    public sealed class AniDBClient : IDisposable, IAsyncDisposable
    {
        private static readonly object _initLock = new();
        private static bool _initialized;
        private const uint ABI_VERSION = 1;

        private readonly SafeClientHandle _handle;
        private readonly Dictionary<ulong, RegisteredCallback> _callbacks = new();
        private readonly SemaphoreSlim _callbackLock = new(1, 1);
        
        private GCHandle _eventCallbackHandle;
        private AniDBEventCallback? _nativeEventCallback;
        private EventHandler<ProcessingEvent>? _eventReceived;
        
        private bool _disposed;

        /// <summary>
        /// Event raised when a processing event occurs
        /// </summary>
        public event EventHandler<ProcessingEvent>? EventReceived
        {
            add
            {
                if (_disposed) throw new ObjectDisposedException(nameof(AniDBClient));
                
                var firstSubscriber = _eventReceived == null;
                _eventReceived += value;
                
                if (firstSubscriber && value != null)
                {
                    ConnectEventSystem();
                }
            }
            remove
            {
                _eventReceived -= value;
                
                if (_eventReceived == null)
                {
                    DisconnectEventSystem();
                }
            }
        }

        /// <summary>
        /// Initializes a new instance of the AniDBClient class with default configuration
        /// </summary>
        public AniDBClient() : this(new ClientConfiguration())
        {
        }

        /// <summary>
        /// Initializes a new instance of the AniDBClient class with custom configuration
        /// </summary>
        /// <param name="configuration">Client configuration</param>
        public AniDBClient(ClientConfiguration configuration)
        {
            if (configuration == null)
                throw new ArgumentNullException(nameof(configuration));

            EnsureLibraryInitialized();

            // Create native config
            var cacheDir = Marshal.StringToHGlobalAnsi(configuration.CacheDirectory);
            var username = configuration.Username != null ? 
                Marshal.StringToHGlobalAnsi(configuration.Username) : IntPtr.Zero;
            var password = configuration.Password != null ? 
                Marshal.StringToHGlobalAnsi(configuration.Password) : IntPtr.Zero;

            try
            {
                var nativeConfig = new AniDBConfig
                {
                    CacheDir = cacheDir,
                    MaxConcurrentFiles = (UIntPtr)configuration.MaxConcurrentFiles,
                    ChunkSize = (UIntPtr)configuration.ChunkSize,
                    MaxMemoryUsage = (UIntPtr)configuration.MaxMemoryUsage,
                    EnableDebugLogging = configuration.EnableDebugLogging ? 1 : 0,
                    Username = username,
                    Password = password
                };

                var result = NativeMethods.anidb_client_create_with_config(in nativeConfig, out var handle);
                if (result != AniDBResult.Success)
                {
                    throw AniDBException.FromResult(result);
                }

                _handle = new SafeClientHandle();
                _handle.SetHandle(handle);
            }
            finally
            {
                Marshal.FreeHGlobal(cacheDir);
                if (username != IntPtr.Zero) Marshal.FreeHGlobal(username);
                if (password != IntPtr.Zero) Marshal.FreeHGlobal(password);
            }
        }

        /// <summary>
        /// Process a single file asynchronously
        /// </summary>
        /// <param name="filePath">Path to the file</param>
        /// <param name="options">Processing options</param>
        /// <param name="cancellationToken">Cancellation token</param>
        /// <returns>File processing result</returns>
        public async Task<FileResult> ProcessFileAsync(
            string filePath, 
            ProcessingOptions? options = null,
            CancellationToken cancellationToken = default)
        {
            ThrowIfDisposed();
            
            if (string.IsNullOrEmpty(filePath))
                throw new ArgumentException("File path cannot be null or empty", nameof(filePath));

            options ??= new ProcessingOptions();

            var tcs = new TaskCompletionSource<FileResult>(TaskCreationOptions.RunContinuationsAsynchronously);
            
            // Handle cancellation
            using var registration = cancellationToken.Register(() =>
            {
                tcs.TrySetCanceled(cancellationToken);
            });

            // Create native options
            var algorithms = options.Algorithms.Select(ConvertHashAlgorithm).ToArray();
            var algorithmsHandle = GCHandle.Alloc(algorithms, GCHandleType.Pinned);
            
            ProgressCallbackContext? progressContext = null;
            GCHandle progressHandle = default;

            try
            {
                var nativeOptions = new AniDBProcessOptions
                {
                    Algorithms = algorithmsHandle.AddrOfPinnedObject(),
                    AlgorithmCount = (UIntPtr)algorithms.Length,
                    EnableProgress = options.EnableProgress ? 1 : 0,
                    VerifyExisting = options.VerifyExisting ? 1 : 0,
                    ProgressCallback = IntPtr.Zero,
                    UserData = IntPtr.Zero
                };

                // Set up progress callback if requested
                if (options.ProgressCallback != null && options.EnableProgress)
                {
                    progressContext = new ProgressCallbackContext(options.ProgressCallback);
                    progressHandle = GCHandle.Alloc(progressContext);
                    
                    nativeOptions.ProgressCallback = Marshal.GetFunctionPointerForDelegate(progressContext.NativeCallback);
                    nativeOptions.UserData = GCHandle.ToIntPtr(progressHandle);
                }

                // Start async operation
                var result = NativeMethods.anidb_process_file_async(
                    _handle.DangerousGetHandle(),
                    filePath,
                    in nativeOptions,
                    out var operationHandle);

                if (result != AniDBResult.Success)
                {
                    throw AniDBException.FromResult(result);
                }

                using var operation = new SafeOperationHandle();
                operation.SetHandle(operationHandle);

                // Poll for completion
                var fileResult = await Task.Run(async () =>
                {
                    while (!cancellationToken.IsCancellationRequested)
                    {
                        var statusResult = NativeMethods.anidb_operation_get_status(
                            operation.DangerousGetHandle(), 
                            out var status);
                        
                        if (statusResult != AniDBResult.Success)
                        {
                            throw AniDBException.FromResult(statusResult);
                        }

                        if (status == AniDBStatus.Completed || status == AniDBStatus.Failed)
                        {
                            var getResult = NativeMethods.anidb_operation_get_result(
                                operation.DangerousGetHandle(),
                                out var resultPtr);
                            
                            if (getResult != AniDBResult.Success)
                            {
                                throw AniDBException.FromResult(getResult);
                            }

                            using var resultHandle = new SafeFileResultHandle();
                            resultHandle.SetHandle(resultPtr);
                            
                            return ConvertFileResult(resultPtr);
                        }

                        if (status == AniDBStatus.Cancelled)
                        {
                            throw new OperationCancelledException("Operation was cancelled");
                        }

                        await Task.Delay(10, cancellationToken).ConfigureAwait(false);
                    }

                    // Cancel the operation if token was cancelled
                    NativeMethods.anidb_operation_cancel(operation.DangerousGetHandle());
                    throw new OperationCancelledException("Operation was cancelled");
                }, cancellationToken).ConfigureAwait(false);

                return fileResult;
            }
            finally
            {
                algorithmsHandle.Free();
                if (progressHandle.IsAllocated)
                {
                    progressHandle.Free();
                }
            }
        }

        /// <summary>
        /// Process a single file synchronously
        /// </summary>
        /// <param name="filePath">Path to the file</param>
        /// <param name="options">Processing options</param>
        /// <returns>File processing result</returns>
        public FileResult ProcessFile(string filePath, ProcessingOptions? options = null)
        {
            ThrowIfDisposed();
            
            if (string.IsNullOrEmpty(filePath))
                throw new ArgumentException("File path cannot be null or empty", nameof(filePath));

            options ??= new ProcessingOptions();

            // Create native options
            var algorithms = options.Algorithms.Select(ConvertHashAlgorithm).ToArray();
            var algorithmsHandle = GCHandle.Alloc(algorithms, GCHandleType.Pinned);
            
            ProgressCallbackContext? progressContext = null;
            GCHandle progressHandle = default;

            try
            {
                var nativeOptions = new AniDBProcessOptions
                {
                    Algorithms = algorithmsHandle.AddrOfPinnedObject(),
                    AlgorithmCount = (UIntPtr)algorithms.Length,
                    EnableProgress = options.EnableProgress ? 1 : 0,
                    VerifyExisting = options.VerifyExisting ? 1 : 0,
                    ProgressCallback = IntPtr.Zero,
                    UserData = IntPtr.Zero
                };

                // Set up progress callback if requested
                if (options.ProgressCallback != null && options.EnableProgress)
                {
                    progressContext = new ProgressCallbackContext(options.ProgressCallback);
                    progressHandle = GCHandle.Alloc(progressContext);
                    
                    nativeOptions.ProgressCallback = Marshal.GetFunctionPointerForDelegate(progressContext.NativeCallback);
                    nativeOptions.UserData = GCHandle.ToIntPtr(progressHandle);
                }

                var result = NativeMethods.anidb_process_file(
                    _handle.DangerousGetHandle(),
                    filePath,
                    in nativeOptions,
                    out var resultPtr);

                if (result != AniDBResult.Success)
                {
                    throw AniDBException.FromResult(result, GetLastError());
                }

                using var resultHandle = new SafeFileResultHandle();
                resultHandle.SetHandle(resultPtr);
                
                return ConvertFileResult(resultPtr);
            }
            finally
            {
                algorithmsHandle.Free();
                if (progressHandle.IsAllocated)
                {
                    progressHandle.Free();
                }
            }
        }

        /// <summary>
        /// Process multiple files in a batch asynchronously
        /// </summary>
        /// <param name="filePaths">Paths to the files</param>
        /// <param name="options">Batch processing options</param>
        /// <param name="cancellationToken">Cancellation token</param>
        /// <returns>Batch processing result</returns>
        public async Task<BatchResult> ProcessBatchAsync(
            IEnumerable<string> filePaths,
            BatchOptions? options = null,
            CancellationToken cancellationToken = default)
        {
            ThrowIfDisposed();
            
            if (filePaths == null)
                throw new ArgumentNullException(nameof(filePaths));

            var paths = filePaths.ToArray();
            if (paths.Length == 0)
                throw new ArgumentException("File paths cannot be empty", nameof(filePaths));

            options ??= new BatchOptions();

            // Create native options
            var algorithms = options.Algorithms.Select(ConvertHashAlgorithm).ToArray();
            var algorithmsHandle = GCHandle.Alloc(algorithms, GCHandleType.Pinned);
            
            ProgressCallbackContext? progressContext = null;
            GCHandle progressHandle = default;

            try
            {
                var nativeOptions = new AniDBBatchOptions
                {
                    Algorithms = algorithmsHandle.AddrOfPinnedObject(),
                    AlgorithmCount = (UIntPtr)algorithms.Length,
                    MaxConcurrent = (UIntPtr)options.MaxConcurrent,
                    ContinueOnError = options.ContinueOnError ? 1 : 0,
                    SkipExisting = options.SkipExisting ? 1 : 0,
                    ProgressCallback = IntPtr.Zero,
                    CompletionCallback = IntPtr.Zero,
                    UserData = IntPtr.Zero
                };

                // Set up progress callback if requested
                if (options.ProgressCallback != null && options.EnableProgress)
                {
                    progressContext = new ProgressCallbackContext(options.ProgressCallback);
                    progressHandle = GCHandle.Alloc(progressContext);
                    
                    nativeOptions.ProgressCallback = Marshal.GetFunctionPointerForDelegate(progressContext.NativeCallback);
                    nativeOptions.UserData = GCHandle.ToIntPtr(progressHandle);
                }

                // Start async batch operation
                var result = NativeMethods.anidb_process_batch_async(
                    _handle.DangerousGetHandle(),
                    paths,
                    (UIntPtr)paths.Length,
                    in nativeOptions,
                    out var batchHandle);

                if (result != AniDBResult.Success)
                {
                    throw AniDBException.FromResult(result);
                }

                using var batch = new SafeBatchHandle();
                batch.SetHandle(batchHandle);

                // Poll for completion
                var batchResult = await Task.Run(async () =>
                {
                    while (!cancellationToken.IsCancellationRequested)
                    {
                        var progressResult = NativeMethods.anidb_batch_get_progress(
                            batch.DangerousGetHandle(),
                            out var completed,
                            out var total);
                        
                        if (progressResult != AniDBResult.Success)
                        {
                            throw AniDBException.FromResult(progressResult);
                        }

                        if ((ulong)completed >= (ulong)total)
                        {
                            // Get results - this is a simplified version
                            // In a real implementation, we'd need to retrieve the actual results
                            return new BatchResult
                            {
                                TotalFiles = (int)(ulong)total,
                                SuccessfulFiles = (int)(ulong)completed,
                                FailedFiles = 0,
                                Results = Array.Empty<FileResult>(),
                                TotalTime = TimeSpan.Zero
                            };
                        }

                        await Task.Delay(50, cancellationToken).ConfigureAwait(false);
                    }

                    // Cancel the batch if token was cancelled
                    NativeMethods.anidb_batch_cancel(batch.DangerousGetHandle());
                    throw new OperationCancelledException("Batch operation was cancelled");
                }, cancellationToken).ConfigureAwait(false);

                return batchResult;
            }
            finally
            {
                algorithmsHandle.Free();
                if (progressHandle.IsAllocated)
                {
                    progressHandle.Free();
                }
            }
        }

        /// <summary>
        /// Calculate hash for a file
        /// </summary>
        /// <param name="filePath">Path to the file</param>
        /// <param name="algorithm">Hash algorithm to use</param>
        /// <returns>Hash value as hexadecimal string</returns>
        public string CalculateHash(string filePath, HashAlgorithm algorithm)
        {
            ThrowIfDisposed();
            
            if (string.IsNullOrEmpty(filePath))
                throw new ArgumentException("File path cannot be null or empty", nameof(filePath));

            var nativeAlgorithm = ConvertHashAlgorithm(algorithm);
            var bufferSize = NativeMethods.anidb_hash_buffer_size(nativeAlgorithm);
            var buffer = Marshal.AllocHGlobal((int)bufferSize);

            try
            {
                var result = NativeMethods.anidb_calculate_hash(
                    filePath,
                    nativeAlgorithm,
                    buffer,
                    bufferSize);

                if (result != AniDBResult.Success)
                {
                    throw AniDBException.FromResult(result);
                }

                return Marshal.PtrToStringAnsi(buffer) ?? string.Empty;
            }
            finally
            {
                Marshal.FreeHGlobal(buffer);
            }
        }

        /// <summary>
        /// Calculate hash for a memory buffer
        /// </summary>
        /// <param name="data">Data to hash</param>
        /// <param name="algorithm">Hash algorithm to use</param>
        /// <returns>Hash value as hexadecimal string</returns>
        public string CalculateHash(byte[] data, HashAlgorithm algorithm)
        {
            ThrowIfDisposed();
            
            if (data == null)
                throw new ArgumentNullException(nameof(data));

            var nativeAlgorithm = ConvertHashAlgorithm(algorithm);
            var bufferSize = NativeMethods.anidb_hash_buffer_size(nativeAlgorithm);
            var hashBuffer = Marshal.AllocHGlobal((int)bufferSize);
            var dataHandle = GCHandle.Alloc(data, GCHandleType.Pinned);

            try
            {
                var result = NativeMethods.anidb_calculate_hash_buffer(
                    dataHandle.AddrOfPinnedObject(),
                    (UIntPtr)data.Length,
                    nativeAlgorithm,
                    hashBuffer,
                    bufferSize);

                if (result != AniDBResult.Success)
                {
                    throw AniDBException.FromResult(result);
                }

                return Marshal.PtrToStringAnsi(hashBuffer) ?? string.Empty;
            }
            finally
            {
                Marshal.FreeHGlobal(hashBuffer);
                dataHandle.Free();
            }
        }

        /// <summary>
        /// Clear the hash cache
        /// </summary>
        public void ClearCache()
        {
            ThrowIfDisposed();
            
            var result = NativeMethods.anidb_cache_clear(_handle.DangerousGetHandle());
            if (result != AniDBResult.Success)
            {
                throw AniDBException.FromResult(result);
            }
        }

        /// <summary>
        /// Get cache statistics
        /// </summary>
        /// <returns>Cache statistics</returns>
        public CacheStatistics GetCacheStatistics()
        {
            ThrowIfDisposed();
            
            var result = NativeMethods.anidb_cache_get_stats(
                _handle.DangerousGetHandle(),
                out var totalEntries,
                out var cacheSize);

            if (result != AniDBResult.Success)
            {
                throw AniDBException.FromResult(result);
            }

            return new CacheStatistics
            {
                TotalEntries = (long)(ulong)totalEntries,
                SizeInBytes = (long)cacheSize,
                HitRate = 0.0 // Would need additional API support
            };
        }

        /// <summary>
        /// Check if a file hash is in cache
        /// </summary>
        /// <param name="filePath">Path to the file</param>
        /// <param name="algorithm">Hash algorithm</param>
        /// <returns>True if cached, false otherwise</returns>
        public bool IsFileCached(string filePath, HashAlgorithm algorithm)
        {
            ThrowIfDisposed();
            
            if (string.IsNullOrEmpty(filePath))
                throw new ArgumentException("File path cannot be null or empty", nameof(filePath));

            var result = NativeMethods.anidb_cache_check_file(
                _handle.DangerousGetHandle(),
                filePath,
                ConvertHashAlgorithm(algorithm),
                out var isCached);

            if (result != AniDBResult.Success)
            {
                throw AniDBException.FromResult(result);
            }

            return isCached != 0;
        }

        /// <summary>
        /// Identify an anime file by hash and size
        /// </summary>
        /// <param name="ed2kHash">ED2K hash of the file</param>
        /// <param name="fileSize">File size in bytes</param>
        /// <returns>Anime identification information</returns>
        public async Task<AnimeInfo?> IdentifyFileAsync(string ed2kHash, long fileSize)
        {
            ThrowIfDisposed();
            
            if (string.IsNullOrEmpty(ed2kHash))
                throw new ArgumentException("ED2K hash cannot be null or empty", nameof(ed2kHash));

            return await Task.Run(() =>
            {
                var result = NativeMethods.anidb_identify_file(
                    _handle.DangerousGetHandle(),
                    ed2kHash,
                    (ulong)fileSize,
                    out var infoPtr);

                if (result == AniDBResult.ErrorNetwork)
                {
                    return null; // Not found or network unavailable
                }

                if (result != AniDBResult.Success)
                {
                    throw AniDBException.FromResult(result);
                }

                if (infoPtr == IntPtr.Zero)
                {
                    return null;
                }

                using var infoHandle = new SafeAnimeInfoHandle();
                infoHandle.SetHandle(infoPtr);
                
                return ConvertAnimeInfo(infoPtr);
            }).ConfigureAwait(false);
        }

        /// <summary>
        /// Get the library version
        /// </summary>
        /// <returns>Version string</returns>
        public static string GetVersion()
        {
            var ptr = NativeMethods.anidb_get_version();
            return Marshal.PtrToStringAnsi(ptr) ?? "Unknown";
        }

        /// <summary>
        /// Get the library ABI version
        /// </summary>
        /// <returns>ABI version number</returns>
        public static uint GetAbiVersion()
        {
            return NativeMethods.anidb_get_abi_version();
        }

        #region Private Methods

        private static void EnsureLibraryInitialized()
        {
            if (_initialized) return;

            lock (_initLock)
            {
                if (_initialized) return;

                var result = NativeMethods.anidb_init(ABI_VERSION);
                if (result != AniDBResult.Success)
                {
                    throw AniDBException.FromResult(result);
                }

                _initialized = true;
            }
        }

        private string? GetLastError()
        {
            const int bufferSize = 1024;
            var buffer = Marshal.AllocHGlobal(bufferSize);

            try
            {
                var result = NativeMethods.anidb_client_get_last_error(
                    _handle.DangerousGetHandle(),
                    buffer,
                    (UIntPtr)bufferSize);

                if (result != AniDBResult.Success)
                {
                    return null;
                }

                return Marshal.PtrToStringAnsi(buffer);
            }
            finally
            {
                Marshal.FreeHGlobal(buffer);
            }
        }

        private static AniDBHashAlgorithm ConvertHashAlgorithm(HashAlgorithm algorithm)
        {
            return algorithm switch
            {
                HashAlgorithm.ED2K => AniDBHashAlgorithm.ED2K,
                HashAlgorithm.CRC32 => AniDBHashAlgorithm.CRC32,
                HashAlgorithm.MD5 => AniDBHashAlgorithm.MD5,
                HashAlgorithm.SHA1 => AniDBHashAlgorithm.SHA1,
                HashAlgorithm.TTH => AniDBHashAlgorithm.TTH,
                _ => throw new ArgumentException($"Unknown hash algorithm: {algorithm}")
            };
        }

        private static HashAlgorithm ConvertHashAlgorithm(AniDBHashAlgorithm algorithm)
        {
            return algorithm switch
            {
                AniDBHashAlgorithm.ED2K => HashAlgorithm.ED2K,
                AniDBHashAlgorithm.CRC32 => HashAlgorithm.CRC32,
                AniDBHashAlgorithm.MD5 => HashAlgorithm.MD5,
                AniDBHashAlgorithm.SHA1 => HashAlgorithm.SHA1,
                AniDBHashAlgorithm.TTH => HashAlgorithm.TTH,
                _ => throw new ArgumentException($"Unknown native hash algorithm: {algorithm}")
            };
        }

        private static ProcessingStatus ConvertStatus(AniDBStatus status)
        {
            return status switch
            {
                AniDBStatus.Pending => ProcessingStatus.Pending,
                AniDBStatus.Processing => ProcessingStatus.Processing,
                AniDBStatus.Completed => ProcessingStatus.Completed,
                AniDBStatus.Failed => ProcessingStatus.Failed,
                AniDBStatus.Cancelled => ProcessingStatus.Cancelled,
                _ => ProcessingStatus.Failed
            };
        }

        private static FileResult ConvertFileResult(IntPtr resultPtr)
        {
            var nativeResult = Marshal.PtrToStructure<AniDBFileResult>(resultPtr);
            
            var filePath = Marshal.PtrToStringAnsi(nativeResult.FilePath) ?? string.Empty;
            var errorMessage = nativeResult.ErrorMessage != IntPtr.Zero ?
                Marshal.PtrToStringAnsi(nativeResult.ErrorMessage) : null;

            var hashes = new List<HashResult>();
            if (nativeResult.Hashes != IntPtr.Zero && nativeResult.HashCount != UIntPtr.Zero)
            {
                var hashCount = (int)(ulong)nativeResult.HashCount;
                var hashSize = Marshal.SizeOf<AniDBHashResult>();
                
                for (int i = 0; i < hashCount; i++)
                {
                    var hashPtr = IntPtr.Add(nativeResult.Hashes, i * hashSize);
                    var nativeHash = Marshal.PtrToStructure<AniDBHashResult>(hashPtr);
                    
                    var hashValue = Marshal.PtrToStringAnsi(nativeHash.HashValue) ?? string.Empty;
                    
                    hashes.Add(new HashResult
                    {
                        Algorithm = ConvertHashAlgorithm(nativeHash.Algorithm),
                        Value = hashValue
                    });
                }
            }

            return new FileResult
            {
                FilePath = filePath,
                FileSize = (long)nativeResult.FileSize,
                Status = ConvertStatus(nativeResult.Status),
                Hashes = hashes,
                ProcessingTime = TimeSpan.FromMilliseconds(nativeResult.ProcessingTimeMs),
                ErrorMessage = errorMessage,
                FromCache = false // Would need additional API support
            };
        }

        private static AnimeInfo ConvertAnimeInfo(IntPtr infoPtr)
        {
            var nativeInfo = Marshal.PtrToStructure<AniDBAnimeInfo>(infoPtr);
            
            var title = Marshal.PtrToStringAnsi(nativeInfo.Title) ?? string.Empty;
            
            return new AnimeInfo
            {
                AnimeId = (long)nativeInfo.AnimeId,
                EpisodeId = (long)nativeInfo.EpisodeId,
                Title = title,
                EpisodeNumber = (int)nativeInfo.EpisodeNumber,
                Confidence = nativeInfo.Confidence,
                Source = (IdentificationSource)nativeInfo.Source
            };
        }

        private void ConnectEventSystem()
        {
            if (_nativeEventCallback != null) return;

            _nativeEventCallback = HandleNativeEvent;
            _eventCallbackHandle = GCHandle.Alloc(this);

            var result = NativeMethods.anidb_event_connect(
                _handle.DangerousGetHandle(),
                _nativeEventCallback,
                GCHandle.ToIntPtr(_eventCallbackHandle));

            if (result != AniDBResult.Success)
            {
                _eventCallbackHandle.Free();
                _nativeEventCallback = null;
                throw AniDBException.FromResult(result);
            }
        }

        private void DisconnectEventSystem()
        {
            if (_nativeEventCallback == null) return;

            NativeMethods.anidb_event_disconnect(_handle.DangerousGetHandle());
            
            if (_eventCallbackHandle.IsAllocated)
            {
                _eventCallbackHandle.Free();
            }
            
            _nativeEventCallback = null;
        }

        private static void HandleNativeEvent(ref AniDBEvent evt, IntPtr userData)
        {
            try
            {
                var handle = GCHandle.FromIntPtr(userData);
                var client = (AniDBClient)handle.Target!;
                
                var processingEvent = ConvertEvent(in evt);
                client._eventReceived?.Invoke(client, processingEvent);
            }
            catch
            {
                // Don't let exceptions propagate to native code
            }
        }

        private static ProcessingEvent ConvertEvent(in AniDBEvent evt)
        {
            var timestamp = DateTimeOffset.FromUnixTimeMilliseconds((long)evt.Timestamp).DateTime;
            var context = evt.Context != IntPtr.Zero ? 
                Marshal.PtrToStringAnsi(evt.Context) : null;

            var processingEvent = new ProcessingEvent
            {
                Type = ConvertEventType(evt.Type),
                Timestamp = timestamp,
                Context = context
            };

            // Set event-specific data based on type
            switch (evt.Type)
            {
                case AniDBEventType.FileStart:
                case AniDBEventType.FileComplete:
                    unsafe
                    {
                        var filePath = evt.Data.File.FilePath != IntPtr.Zero ?
                            Marshal.PtrToStringAnsi(evt.Data.File.FilePath) : null;
                        processingEvent.FilePath = filePath;
                        processingEvent.FileSize = (long)evt.Data.File.FileSize;
                    }
                    break;

                case AniDBEventType.HashStart:
                case AniDBEventType.HashComplete:
                    unsafe
                    {
                        var hashValue = evt.Data.Hash.HashValue != IntPtr.Zero ?
                            Marshal.PtrToStringAnsi(evt.Data.Hash.HashValue) : null;
                        processingEvent.Algorithm = ConvertHashAlgorithm(evt.Data.Hash.Algorithm);
                        processingEvent.HashValue = hashValue;
                    }
                    break;

                case AniDBEventType.MemoryWarning:
                    unsafe
                    {
                        processingEvent.MemoryUsage = (
                            (long)evt.Data.Memory.CurrentUsage,
                            (long)evt.Data.Memory.MaxUsage
                        );
                    }
                    break;
            }

            return processingEvent;
        }

        private static EventType ConvertEventType(AniDBEventType type)
        {
            return type switch
            {
                AniDBEventType.FileStart => EventType.FileStart,
                AniDBEventType.FileComplete => EventType.FileComplete,
                AniDBEventType.HashStart => EventType.HashStart,
                AniDBEventType.HashComplete => EventType.HashComplete,
                AniDBEventType.CacheHit => EventType.CacheHit,
                AniDBEventType.CacheMiss => EventType.CacheMiss,
                AniDBEventType.NetworkStart => EventType.NetworkStart,
                AniDBEventType.NetworkComplete => EventType.NetworkComplete,
                AniDBEventType.MemoryWarning => EventType.MemoryWarning,
                _ => throw new ArgumentException($"Unknown event type: {type}")
            };
        }

        private void ThrowIfDisposed()
        {
            if (_disposed)
                throw new ObjectDisposedException(nameof(AniDBClient));
        }

        #endregion

        #region IDisposable & IAsyncDisposable

        /// <summary>
        /// Releases all resources used by the AniDBClient
        /// </summary>
        public void Dispose()
        {
            if (_disposed) return;

            DisconnectEventSystem();
            
            _handle?.Dispose();
            _callbackLock?.Dispose();
            
            _disposed = true;
        }

        /// <summary>
        /// Asynchronously releases all resources used by the AniDBClient
        /// </summary>
        public async ValueTask DisposeAsync()
        {
            if (_disposed) return;

            await Task.Run(() => DisconnectEventSystem()).ConfigureAwait(false);
            
            _handle?.Dispose();
            _callbackLock?.Dispose();
            
            _disposed = true;
        }

        #endregion

        #region Helper Classes

        private class ProgressCallbackContext
        {
            private readonly Action<ProgressInfo> _userCallback;
            public readonly AniDBProgressCallback NativeCallback;

            public ProgressCallbackContext(Action<ProgressInfo> userCallback)
            {
                _userCallback = userCallback;
                NativeCallback = HandleProgress;
            }

            private void HandleProgress(float percentage, ulong bytesProcessed, ulong totalBytes, IntPtr userData)
            {
                try
                {
                    var info = new ProgressInfo
                    {
                        Percentage = percentage,
                        BytesProcessed = (long)bytesProcessed,
                        TotalBytes = (long)totalBytes
                    };
                    
                    _userCallback(info);
                }
                catch
                {
                    // Don't let user exceptions propagate to native code
                }
            }
        }

        private class RegisteredCallback
        {
            public Delegate UserCallback { get; set; } = null!;
            public GCHandle Handle { get; set; }
        }

        #endregion
    }
}