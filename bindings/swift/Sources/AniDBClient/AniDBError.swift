import Foundation
import CAniDB

/// Errors that can occur when using the AniDB client
public enum AniDBError: LocalizedError, Sendable {
    case invalidHandle
    case invalidParameter
    case fileNotFound(path: String)
    case processingError(message: String)
    case outOfMemory
    case ioError(message: String)
    case networkError(message: String)
    case cancelled
    case invalidUTF8
    case versionMismatch
    case timeout
    case permissionDenied(path: String)
    case cacheError(message: String)
    case busy
    case unknown(code: Int)
    case clientDestroyed
    
    init(result: anidb_result_t) {
        switch result {
        case ANIDB_ERROR_INVALID_HANDLE:
            self = .invalidHandle
        case ANIDB_ERROR_INVALID_PARAMETER:
            self = .invalidParameter
        case ANIDB_ERROR_FILE_NOT_FOUND:
            self = .fileNotFound(path: "")
        case ANIDB_ERROR_PROCESSING:
            self = .processingError(message: "Processing failed")
        case ANIDB_ERROR_OUT_OF_MEMORY:
            self = .outOfMemory
        case ANIDB_ERROR_IO:
            self = .ioError(message: "I/O operation failed")
        case ANIDB_ERROR_NETWORK:
            self = .networkError(message: "Network operation failed")
        case ANIDB_ERROR_CANCELLED:
            self = .cancelled
        case ANIDB_ERROR_INVALID_UTF8:
            self = .invalidUTF8
        case ANIDB_ERROR_VERSION_MISMATCH:
            self = .versionMismatch
        case ANIDB_ERROR_TIMEOUT:
            self = .timeout
        case ANIDB_ERROR_PERMISSION_DENIED:
            self = .permissionDenied(path: "")
        case ANIDB_ERROR_CACHE:
            self = .cacheError(message: "Cache operation failed")
        case ANIDB_ERROR_BUSY:
            self = .busy
        default:
            self = .unknown(code: Int(result.rawValue))
        }
    }
    
    public var errorDescription: String? {
        switch self {
        case .invalidHandle:
            return "Invalid client handle"
        case .invalidParameter:
            return "Invalid parameter provided"
        case .fileNotFound(let path):
            return path.isEmpty ? "File not found" : "File not found: \(path)"
        case .processingError(let message):
            return "Processing error: \(message)"
        case .outOfMemory:
            return "Out of memory"
        case .ioError(let message):
            return "I/O error: \(message)"
        case .networkError(let message):
            return "Network error: \(message)"
        case .cancelled:
            return "Operation cancelled"
        case .invalidUTF8:
            return "Invalid UTF-8 string"
        case .versionMismatch:
            return "Library version mismatch"
        case .timeout:
            return "Operation timed out"
        case .permissionDenied(let path):
            return path.isEmpty ? "Permission denied" : "Permission denied: \(path)"
        case .cacheError(let message):
            return "Cache error: \(message)"
        case .busy:
            return "Resource busy"
        case .unknown(let code):
            return "Unknown error (code: \(code))"
        case .clientDestroyed:
            return "Client has been destroyed"
        }
    }
    
    public var recoverySuggestion: String? {
        switch self {
        case .invalidHandle, .clientDestroyed:
            return "Create a new client instance"
        case .invalidParameter:
            return "Check the parameters passed to the function"
        case .fileNotFound:
            return "Verify the file path exists and is accessible"
        case .processingError:
            return "Check the file format and try again"
        case .outOfMemory:
            return "Close other applications or process smaller files"
        case .ioError:
            return "Check disk space and file permissions"
        case .networkError:
            return "Check your internet connection and try again"
        case .cancelled:
            return "The operation was cancelled by user request"
        case .invalidUTF8:
            return "Ensure all strings are valid UTF-8"
        case .versionMismatch:
            return "Update the library to a compatible version"
        case .timeout:
            return "Try again or increase the timeout duration"
        case .permissionDenied:
            return "Check file permissions and try running with appropriate privileges"
        case .cacheError:
            return "Try clearing the cache or check disk space"
        case .busy:
            return "Wait for the current operation to complete"
        case .unknown:
            return "An unexpected error occurred"
        }
    }
}