using System;
using System.Runtime.InteropServices;

namespace AniDBClient.Native
{
    /// <summary>
    /// Result codes for API operations
    /// </summary>
    internal enum AniDBResult
    {
        Success = 0,
        ErrorInvalidHandle = 1,
        ErrorInvalidParameter = 2,
        ErrorFileNotFound = 3,
        ErrorProcessing = 4,
        ErrorOutOfMemory = 5,
        ErrorIo = 6,
        ErrorNetwork = 7,
        ErrorCancelled = 8,
        ErrorInvalidUtf8 = 9,
        ErrorVersionMismatch = 10,
        ErrorTimeout = 11,
        ErrorPermissionDenied = 12,
        ErrorCache = 13,
        ErrorBusy = 14,
        ErrorUnknown = 99
    }

    /// <summary>
    /// Hash algorithm identifiers
    /// </summary>
    internal enum AniDBHashAlgorithm
    {
        ED2K = 1,
        CRC32 = 2,
        MD5 = 3,
        SHA1 = 4,
        TTH = 5
    }

    /// <summary>
    /// Processing status codes
    /// </summary>
    internal enum AniDBStatus
    {
        Pending = 0,
        Processing = 1,
        Completed = 2,
        Failed = 3,
        Cancelled = 4
    }

    /// <summary>
    /// Callback types
    /// </summary>
    internal enum AniDBCallbackType
    {
        Progress = 1,
        Error = 2,
        Completion = 3,
        Event = 4
    }

    /// <summary>
    /// Event types
    /// </summary>
    internal enum AniDBEventType
    {
        FileStart = 1,
        FileComplete = 2,
        HashStart = 3,
        HashComplete = 4,
        CacheHit = 5,
        CacheMiss = 6,
        NetworkStart = 7,
        NetworkComplete = 8,
        MemoryWarning = 9
    }

    /// <summary>
    /// Client configuration structure
    /// </summary>
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Ansi)]
    internal struct AniDBConfig
    {
        public IntPtr CacheDir;
        public UIntPtr MaxConcurrentFiles;
        public UIntPtr ChunkSize;
        public UIntPtr MaxMemoryUsage;
        public int EnableDebugLogging;
        public IntPtr Username;
        public IntPtr Password;
    }

    /// <summary>
    /// File processing options
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    internal struct AniDBProcessOptions
    {
        public IntPtr Algorithms;
        public UIntPtr AlgorithmCount;
        public int EnableProgress;
        public int VerifyExisting;
        public IntPtr ProgressCallback;
        public IntPtr UserData;
    }

    /// <summary>
    /// Hash result structure
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    internal struct AniDBHashResult
    {
        public AniDBHashAlgorithm Algorithm;
        public IntPtr HashValue;
        public UIntPtr HashLength;
    }

    /// <summary>
    /// File processing result
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    internal struct AniDBFileResult
    {
        public IntPtr FilePath;
        public ulong FileSize;
        public AniDBStatus Status;
        public IntPtr Hashes;
        public UIntPtr HashCount;
        public ulong ProcessingTimeMs;
        public IntPtr ErrorMessage;
    }

    /// <summary>
    /// Anime identification information
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    internal struct AniDBAnimeInfo
    {
        public ulong AnimeId;
        public ulong EpisodeId;
        public IntPtr Title;
        public uint EpisodeNumber;
        public double Confidence;
        public int Source;
    }

    /// <summary>
    /// Batch processing options
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    internal struct AniDBBatchOptions
    {
        public IntPtr Algorithms;
        public UIntPtr AlgorithmCount;
        public UIntPtr MaxConcurrent;
        public int ContinueOnError;
        public int SkipExisting;
        public IntPtr ProgressCallback;
        public IntPtr CompletionCallback;
        public IntPtr UserData;
    }

    /// <summary>
    /// Batch processing result
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    internal struct AniDBBatchResult
    {
        public UIntPtr TotalFiles;
        public UIntPtr SuccessfulFiles;
        public UIntPtr FailedFiles;
        public IntPtr Results;
        public ulong TotalTimeMs;
    }

    /// <summary>
    /// Event data union
    /// </summary>
    [StructLayout(LayoutKind.Explicit)]
    internal struct AniDBEventData
    {
        [FieldOffset(0)]
        public FileEventData File;

        [FieldOffset(0)]
        public HashEventData Hash;

        [FieldOffset(0)]
        public CacheEventData Cache;

        [FieldOffset(0)]
        public NetworkEventData Network;

        [FieldOffset(0)]
        public MemoryEventData Memory;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct FileEventData
    {
        public IntPtr FilePath;
        public ulong FileSize;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct HashEventData
    {
        public AniDBHashAlgorithm Algorithm;
        public IntPtr HashValue;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct CacheEventData
    {
        public IntPtr FilePath;
        public AniDBHashAlgorithm Algorithm;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct NetworkEventData
    {
        public IntPtr Endpoint;
        public int StatusCode;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct MemoryEventData
    {
        public ulong CurrentUsage;
        public ulong MaxUsage;
    }

    /// <summary>
    /// Event structure
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    internal struct AniDBEvent
    {
        public AniDBEventType Type;
        public ulong Timestamp;
        public AniDBEventData Data;
        public IntPtr Context;
    }

    #region Delegates

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    internal delegate void AniDBProgressCallback(
        float percentage,
        ulong bytesProcessed,
        ulong totalBytes,
        IntPtr userData);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    internal delegate void AniDBErrorCallback(
        AniDBResult errorCode,
        IntPtr errorMessage,
        IntPtr filePath,
        IntPtr userData);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    internal delegate void AniDBCompletionCallback(
        AniDBResult result,
        IntPtr userData);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    internal delegate void AniDBEventCallback(
        ref AniDBEvent evt,
        IntPtr userData);

    #endregion
}