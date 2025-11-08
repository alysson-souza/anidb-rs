using System;
using System.Runtime.InteropServices;
using AniDBClient.Native;

namespace AniDBClient
{
    /// <summary>
    /// Base exception for AniDB client errors
    /// </summary>
    public class AniDBException : Exception
    {
        /// <summary>
        /// Gets the error code
        /// </summary>
        public AniDBErrorCode ErrorCode { get; }

        /// <summary>
        /// Initializes a new instance of the AniDBException class
        /// </summary>
        public AniDBException(string message) : base(message)
        {
            ErrorCode = AniDBErrorCode.Unknown;
        }

        /// <summary>
        /// Initializes a new instance of the AniDBException class
        /// </summary>
        public AniDBException(string message, Exception innerException) 
            : base(message, innerException)
        {
            ErrorCode = AniDBErrorCode.Unknown;
        }

        /// <summary>
        /// Initializes a new instance of the AniDBException class
        /// </summary>
        public AniDBException(AniDBErrorCode errorCode, string message) : base(message)
        {
            ErrorCode = errorCode;
        }

        /// <summary>
        /// Initializes a new instance of the AniDBException class
        /// </summary>
        public AniDBException(AniDBErrorCode errorCode, string message, Exception innerException) 
            : base(message, innerException)
        {
            ErrorCode = errorCode;
        }

        internal static AniDBException FromResult(AniDBResult result, string? additionalInfo = null)
        {
            var errorCode = ConvertErrorCode(result);
            var message = GetErrorMessage(result);
            
            if (!string.IsNullOrEmpty(additionalInfo))
            {
                message = $"{message}: {additionalInfo}";
            }

            return errorCode switch
            {
                AniDBErrorCode.InvalidHandle => new InvalidHandleException(message),
                AniDBErrorCode.InvalidParameter => new ArgumentException(message),
                AniDBErrorCode.FileNotFound => new FileNotFoundException(message),
                AniDBErrorCode.OutOfMemory => new OutOfMemoryException(message),
                AniDBErrorCode.IO => new IOException(message),
                AniDBErrorCode.Network => new NetworkException(message),
                AniDBErrorCode.Cancelled => new OperationCancelledException(message),
                AniDBErrorCode.Timeout => new TimeoutException(message),
                AniDBErrorCode.PermissionDenied => new UnauthorizedAccessException(message),
                AniDBErrorCode.Cache => new CacheException(message),
                AniDBErrorCode.Busy => new ResourceBusyException(message),
                AniDBErrorCode.VersionMismatch => new VersionMismatchException(message),
                _ => new AniDBException(errorCode, message)
            };
        }

        private static AniDBErrorCode ConvertErrorCode(AniDBResult result)
        {
            return result switch
            {
                AniDBResult.ErrorInvalidHandle => AniDBErrorCode.InvalidHandle,
                AniDBResult.ErrorInvalidParameter => AniDBErrorCode.InvalidParameter,
                AniDBResult.ErrorFileNotFound => AniDBErrorCode.FileNotFound,
                AniDBResult.ErrorProcessing => AniDBErrorCode.Processing,
                AniDBResult.ErrorOutOfMemory => AniDBErrorCode.OutOfMemory,
                AniDBResult.ErrorIo => AniDBErrorCode.IO,
                AniDBResult.ErrorNetwork => AniDBErrorCode.Network,
                AniDBResult.ErrorCancelled => AniDBErrorCode.Cancelled,
                AniDBResult.ErrorInvalidUtf8 => AniDBErrorCode.InvalidUtf8,
                AniDBResult.ErrorVersionMismatch => AniDBErrorCode.VersionMismatch,
                AniDBResult.ErrorTimeout => AniDBErrorCode.Timeout,
                AniDBResult.ErrorPermissionDenied => AniDBErrorCode.PermissionDenied,
                AniDBResult.ErrorCache => AniDBErrorCode.Cache,
                AniDBResult.ErrorBusy => AniDBErrorCode.Busy,
                _ => AniDBErrorCode.Unknown
            };
        }

        private static string GetErrorMessage(AniDBResult result)
        {
            var errorPtr = NativeMethods.anidb_error_string(result);
            if (errorPtr != IntPtr.Zero)
            {
                return Marshal.PtrToStringAnsi(errorPtr) ?? "Unknown error";
            }
            return "Unknown error";
        }
    }

    /// <summary>
    /// Error codes for AniDB operations
    /// </summary>
    public enum AniDBErrorCode
    {
        /// <summary>Unknown error</summary>
        Unknown,
        /// <summary>Invalid handle</summary>
        InvalidHandle,
        /// <summary>Invalid parameter</summary>
        InvalidParameter,
        /// <summary>File not found</summary>
        FileNotFound,
        /// <summary>Processing error</summary>
        Processing,
        /// <summary>Out of memory</summary>
        OutOfMemory,
        /// <summary>I/O error</summary>
        IO,
        /// <summary>Network error</summary>
        Network,
        /// <summary>Operation cancelled</summary>
        Cancelled,
        /// <summary>Invalid UTF-8</summary>
        InvalidUtf8,
        /// <summary>Version mismatch</summary>
        VersionMismatch,
        /// <summary>Operation timeout</summary>
        Timeout,
        /// <summary>Permission denied</summary>
        PermissionDenied,
        /// <summary>Cache error</summary>
        Cache,
        /// <summary>Resource busy</summary>
        Busy
    }

    /// <summary>
    /// Exception thrown when an invalid handle is used
    /// </summary>
    public class InvalidHandleException : AniDBException
    {
        /// <summary>
        /// Initializes a new instance of the InvalidHandleException class
        /// </summary>
        public InvalidHandleException(string message) 
            : base(AniDBErrorCode.InvalidHandle, message) { }
    }

    /// <summary>
    /// Exception thrown when a file is not found
    /// </summary>
    public class FileNotFoundException : AniDBException
    {
        /// <summary>
        /// Gets the file path that was not found
        /// </summary>
        public string? FilePath { get; }

        /// <summary>
        /// Initializes a new instance of the FileNotFoundException class
        /// </summary>
        public FileNotFoundException(string message, string? filePath = null) 
            : base(AniDBErrorCode.FileNotFound, message)
        {
            FilePath = filePath;
        }
    }

    /// <summary>
    /// Exception thrown for I/O errors
    /// </summary>
    public class IOException : AniDBException
    {
        /// <summary>
        /// Initializes a new instance of the IOException class
        /// </summary>
        public IOException(string message) 
            : base(AniDBErrorCode.IO, message) { }

        /// <summary>
        /// Initializes a new instance of the IOException class
        /// </summary>
        public IOException(string message, Exception innerException) 
            : base(AniDBErrorCode.IO, message, innerException) { }
    }

    /// <summary>
    /// Exception thrown for network errors
    /// </summary>
    public class NetworkException : AniDBException
    {
        /// <summary>
        /// Initializes a new instance of the NetworkException class
        /// </summary>
        public NetworkException(string message) 
            : base(AniDBErrorCode.Network, message) { }

        /// <summary>
        /// Initializes a new instance of the NetworkException class
        /// </summary>
        public NetworkException(string message, Exception innerException) 
            : base(AniDBErrorCode.Network, message, innerException) { }
    }

    /// <summary>
    /// Exception thrown when an operation is cancelled
    /// </summary>
    public class OperationCancelledException : AniDBException
    {
        /// <summary>
        /// Initializes a new instance of the OperationCancelledException class
        /// </summary>
        public OperationCancelledException(string message) 
            : base(AniDBErrorCode.Cancelled, message) { }
    }

    /// <summary>
    /// Exception thrown when an operation times out
    /// </summary>
    public class TimeoutException : AniDBException
    {
        /// <summary>
        /// Initializes a new instance of the TimeoutException class
        /// </summary>
        public TimeoutException(string message) 
            : base(AniDBErrorCode.Timeout, message) { }
    }

    /// <summary>
    /// Exception thrown for permission errors
    /// </summary>
    public class UnauthorizedAccessException : AniDBException
    {
        /// <summary>
        /// Initializes a new instance of the UnauthorizedAccessException class
        /// </summary>
        public UnauthorizedAccessException(string message) 
            : base(AniDBErrorCode.PermissionDenied, message) { }
    }

    /// <summary>
    /// Exception thrown for cache errors
    /// </summary>
    public class CacheException : AniDBException
    {
        /// <summary>
        /// Initializes a new instance of the CacheException class
        /// </summary>
        public CacheException(string message) 
            : base(AniDBErrorCode.Cache, message) { }

        /// <summary>
        /// Initializes a new instance of the CacheException class
        /// </summary>
        public CacheException(string message, Exception innerException) 
            : base(AniDBErrorCode.Cache, message, innerException) { }
    }

    /// <summary>
    /// Exception thrown when a resource is busy
    /// </summary>
    public class ResourceBusyException : AniDBException
    {
        /// <summary>
        /// Initializes a new instance of the ResourceBusyException class
        /// </summary>
        public ResourceBusyException(string message) 
            : base(AniDBErrorCode.Busy, message) { }
    }

    /// <summary>
    /// Exception thrown for version mismatches
    /// </summary>
    public class VersionMismatchException : AniDBException
    {
        /// <summary>
        /// Gets the expected version
        /// </summary>
        public string? ExpectedVersion { get; }

        /// <summary>
        /// Gets the actual version
        /// </summary>
        public string? ActualVersion { get; }

        /// <summary>
        /// Initializes a new instance of the VersionMismatchException class
        /// </summary>
        public VersionMismatchException(string message, string? expectedVersion = null, string? actualVersion = null) 
            : base(AniDBErrorCode.VersionMismatch, message)
        {
            ExpectedVersion = expectedVersion;
            ActualVersion = actualVersion;
        }
    }

    /// <summary>
    /// Exception thrown when memory limit is exceeded
    /// </summary>
    public class OutOfMemoryException : AniDBException
    {
        /// <summary>
        /// Initializes a new instance of the OutOfMemoryException class
        /// </summary>
        public OutOfMemoryException(string message) 
            : base(AniDBErrorCode.OutOfMemory, message) { }
    }
}