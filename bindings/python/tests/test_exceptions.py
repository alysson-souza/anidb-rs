"""
Tests for exception classes.
"""

import pytest

from anidb_client import (
    AniDBError,
    BusyError,
    CacheError,
    CancelledError,
    FileNotFoundError,
    InvalidHandleError,
    InvalidParameterError,
    InvalidUTF8Error,
    IOError,
    NetworkError,
    OutOfMemoryError,
    PermissionDeniedError,
    ProcessingError,
    TimeoutError,
    UnknownError,
    VersionMismatchError,
)
from anidb_client.native import Result


class TestAniDBError:
    """Test base AniDBError class."""
    
    def test_basic_error(self):
        """Test creating basic error."""
        error = AniDBError("Test error")
        assert str(error) == "Test error"
        assert error.error_code is None
        assert error.file_path is None
    
    def test_error_with_details(self):
        """Test error with additional details."""
        error = AniDBError(
            "Processing failed",
            error_code=Result.ERROR_PROCESSING,
            file_path="/tmp/test.mkv"
        )
        
        assert str(error) == "Processing failed"
        assert error.error_code == Result.ERROR_PROCESSING
        assert error.file_path == "/tmp/test.mkv"
    
    def test_from_result_mapping(self):
        """Test creating specific exceptions from result codes."""
        test_cases = [
            (Result.ERROR_INVALID_HANDLE, InvalidHandleError),
            (Result.ERROR_INVALID_PARAMETER, InvalidParameterError),
            (Result.ERROR_FILE_NOT_FOUND, FileNotFoundError),
            (Result.ERROR_PROCESSING, ProcessingError),
            (Result.ERROR_OUT_OF_MEMORY, OutOfMemoryError),
            (Result.ERROR_IO, IOError),
            (Result.ERROR_NETWORK, NetworkError),
            (Result.ERROR_CANCELLED, CancelledError),
            (Result.ERROR_INVALID_UTF8, InvalidUTF8Error),
            (Result.ERROR_VERSION_MISMATCH, VersionMismatchError),
            (Result.ERROR_TIMEOUT, TimeoutError),
            (Result.ERROR_PERMISSION_DENIED, PermissionDeniedError),
            (Result.ERROR_CACHE, CacheError),
            (Result.ERROR_BUSY, BusyError),
            (Result.ERROR_UNKNOWN, UnknownError),
        ]
        
        for result_code, expected_class in test_cases:
            error = AniDBError.from_result(result_code)
            assert isinstance(error, expected_class)
            assert error.error_code == result_code
    
    def test_from_result_with_message(self):
        """Test creating exception with custom message."""
        error = AniDBError.from_result(
            Result.ERROR_FILE_NOT_FOUND,
            message="Cannot find anime.mkv",
            file_path="/videos/anime.mkv"
        )
        
        assert isinstance(error, FileNotFoundError)
        assert str(error) == "Cannot find anime.mkv"
        assert error.file_path == "/videos/anime.mkv"
    
    def test_from_result_default_message(self):
        """Test default message when none provided."""
        error = AniDBError.from_result(Result.ERROR_NETWORK)
        
        assert isinstance(error, NetworkError)
        assert "error code 7" in str(error)


class TestSpecificExceptions:
    """Test specific exception types."""
    
    def test_file_not_found_error(self):
        """Test FileNotFoundError."""
        error = FileNotFoundError("anime.mkv not found", file_path="/tmp/anime.mkv")
        assert isinstance(error, AniDBError)
        assert error.file_path == "/tmp/anime.mkv"
    
    def test_network_error(self):
        """Test NetworkError."""
        error = NetworkError("Connection timeout")
        assert isinstance(error, AniDBError)
    
    def test_permission_denied_error(self):
        """Test PermissionDeniedError."""
        error = PermissionDeniedError("Access denied", file_path="/protected/file.mkv")
        assert isinstance(error, AniDBError)
        assert error.file_path == "/protected/file.mkv"
    
    def test_out_of_memory_error(self):
        """Test OutOfMemoryError."""
        error = OutOfMemoryError("Insufficient memory for processing")
        assert isinstance(error, AniDBError)
    
    def test_cancelled_error(self):
        """Test CancelledError."""
        error = CancelledError("Operation cancelled by user")
        assert isinstance(error, AniDBError)