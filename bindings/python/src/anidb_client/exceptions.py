"""
Exception classes for the AniDB Client Python Library.
"""

from typing import Optional

from .native import Result


class AniDBError(Exception):
    """Base exception class for all AniDB client errors."""
    
    def __init__(self, message: str, error_code: Optional[Result] = None, file_path: Optional[str] = None):
        """
        Initialize an AniDB error.
        
        Args:
            message: Error message
            error_code: Native error code
            file_path: Path to file that caused the error (optional)
        """
        super().__init__(message)
        self.error_code = error_code
        self.file_path = file_path
    
    @classmethod
    def from_result(cls, result: Result, message: Optional[str] = None, file_path: Optional[str] = None) -> 'AniDBError':
        """
        Create an appropriate exception from a result code.
        
        Args:
            result: Native result code
            message: Optional custom message
            file_path: Optional file path
            
        Returns:
            Appropriate exception instance
        """
        error_map = {
            Result.ERROR_INVALID_HANDLE: InvalidHandleError,
            Result.ERROR_INVALID_PARAMETER: InvalidParameterError,
            Result.ERROR_FILE_NOT_FOUND: FileNotFoundError,
            Result.ERROR_PROCESSING: ProcessingError,
            Result.ERROR_OUT_OF_MEMORY: OutOfMemoryError,
            Result.ERROR_IO: IOError,
            Result.ERROR_NETWORK: NetworkError,
            Result.ERROR_CANCELLED: CancelledError,
            Result.ERROR_INVALID_UTF8: InvalidUTF8Error,
            Result.ERROR_VERSION_MISMATCH: VersionMismatchError,
            Result.ERROR_TIMEOUT: TimeoutError,
            Result.ERROR_PERMISSION_DENIED: PermissionDeniedError,
            Result.ERROR_CACHE: CacheError,
            Result.ERROR_BUSY: BusyError,
            Result.ERROR_UNKNOWN: UnknownError,
        }
        
        exception_class = error_map.get(result, AniDBError)
        if message is None:
            message = f"AniDB operation failed with error code {result}"
        
        return exception_class(message, result, file_path)


class InvalidHandleError(AniDBError):
    """Raised when an invalid handle is provided."""
    pass


class InvalidParameterError(AniDBError):
    """Raised when an invalid parameter is provided."""
    pass


class FileNotFoundError(AniDBError):
    """Raised when a file is not found."""
    pass


class ProcessingError(AniDBError):
    """Raised when an error occurs during processing."""
    pass


class OutOfMemoryError(AniDBError):
    """Raised when the system runs out of memory."""
    pass


class IOError(AniDBError):
    """Raised when an I/O error occurs."""
    pass


class NetworkError(AniDBError):
    """Raised when a network error occurs."""
    pass


class CancelledError(AniDBError):
    """Raised when an operation is cancelled."""
    pass


class InvalidUTF8Error(AniDBError):
    """Raised when invalid UTF-8 is encountered."""
    pass


class VersionMismatchError(AniDBError):
    """Raised when there's a version mismatch."""
    pass


class TimeoutError(AniDBError):
    """Raised when an operation times out."""
    pass


class PermissionDeniedError(AniDBError):
    """Raised when permission is denied."""
    pass


class CacheError(AniDBError):
    """Raised when a cache error occurs."""
    pass


class BusyError(AniDBError):
    """Raised when a resource is busy."""
    pass


class UnknownError(AniDBError):
    """Raised when an unknown error occurs."""
    pass