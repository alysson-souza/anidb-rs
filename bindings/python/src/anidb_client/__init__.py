"""
AniDB Client Python Library

A Python wrapper for the AniDB Client Core Library providing efficient
file hashing and anime identification capabilities.
"""

from .client import AniDBClient
from .exceptions import (
    AniDBError,
    InvalidHandleError,
    InvalidParameterError,
    FileNotFoundError,
    ProcessingError,
    OutOfMemoryError,
    IOError,
    NetworkError,
    CancelledError,
    InvalidUTF8Error,
    VersionMismatchError,
    TimeoutError,
    PermissionDeniedError,
    CacheError,
    BusyError,
    UnknownError,
)
from .types import (
    HashAlgorithm,
    ProcessingStatus,
    CallbackType,
    EventType,
    ClientConfig,
    ProcessOptions,
    BatchOptions,
    HashResult,
    FileResult,
    AnimeInfo,
    BatchResult,
    Event,
)

__version__ = "0.1.0a1"
__all__ = [
    # Main client
    "AniDBClient",
    # Exceptions
    "AniDBError",
    "InvalidHandleError",
    "InvalidParameterError",
    "FileNotFoundError",
    "ProcessingError",
    "OutOfMemoryError",
    "IOError",
    "NetworkError",
    "CancelledError",
    "InvalidUTF8Error",
    "VersionMismatchError",
    "TimeoutError",
    "PermissionDeniedError",
    "CacheError",
    "BusyError",
    "UnknownError",
    # Types
    "HashAlgorithm",
    "ProcessingStatus",
    "CallbackType",
    "EventType",
    "ClientConfig",
    "ProcessOptions",
    "BatchOptions",
    "HashResult",
    "FileResult",
    "AnimeInfo",
    "BatchResult",
    "Event",
]