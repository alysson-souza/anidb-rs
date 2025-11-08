"""
Native ctypes bindings for the AniDB Client Core Library.

This module provides low-level ctypes bindings to the C API.
Users should typically use the higher-level Client class instead.
"""

import ctypes
import ctypes.util
import platform
import sys
from ctypes import (
    CDLL, POINTER, Structure, Union, c_char_p, c_double, c_float, c_int,
    c_size_t, c_uint32, c_uint64, c_void_p, cast, c_uint8
)
from enum import IntEnum
from pathlib import Path
from typing import Optional

# Determine library name based on platform
if platform.system() == "Windows":
    LIB_NAME = "anidb_client_core.dll"
elif platform.system() == "Darwin":
    LIB_NAME = "libanidb_client_core.dylib"
else:
    LIB_NAME = "libanidb_client_core.so"

# Find library path
def _find_library() -> Path:
    """Find the AniDB client library."""
    # Check common locations
    search_paths = [
        # In package directory
        Path(__file__).parent / LIB_NAME,
        # System paths
        Path("/usr/local/lib") / LIB_NAME,
        Path("/usr/lib") / LIB_NAME,
        # Development paths
        Path(__file__).parent.parent.parent.parent.parent / "target" / "release" / LIB_NAME,
        Path(__file__).parent.parent.parent.parent.parent / "target" / "debug" / LIB_NAME,
    ]
    
    # Windows specific paths
    if platform.system() == "Windows":
        search_paths.extend([
            Path("C:/Program Files/AniDB Client") / LIB_NAME,
            Path("C:/Program Files (x86)/AniDB Client") / LIB_NAME,
        ])
    
    for path in search_paths:
        if path.exists():
            return path
    
    # Try loading from system path
    try:
        return ctypes.util.find_library("anidb_client_core") or LIB_NAME
    except:
        return LIB_NAME

# Load the library
try:
    lib_path = _find_library()
    lib = CDLL(str(lib_path))
except OSError as e:
    raise ImportError(f"Failed to load AniDB client library: {e}")

# Constants
ABI_VERSION = 1

# Enums
class Result(IntEnum):
    """Result codes for API operations."""
    SUCCESS = 0
    ERROR_INVALID_HANDLE = 1
    ERROR_INVALID_PARAMETER = 2
    ERROR_FILE_NOT_FOUND = 3
    ERROR_PROCESSING = 4
    ERROR_OUT_OF_MEMORY = 5
    ERROR_IO = 6
    ERROR_NETWORK = 7
    ERROR_CANCELLED = 8
    ERROR_INVALID_UTF8 = 9
    ERROR_VERSION_MISMATCH = 10
    ERROR_TIMEOUT = 11
    ERROR_PERMISSION_DENIED = 12
    ERROR_CACHE = 13
    ERROR_BUSY = 14
    ERROR_UNKNOWN = 99

class HashAlgorithm(IntEnum):
    """Hash algorithm identifiers."""
    ED2K = 1
    CRC32 = 2
    MD5 = 3
    SHA1 = 4
    TTH = 5

class Status(IntEnum):
    """Processing status codes."""
    PENDING = 0
    PROCESSING = 1
    COMPLETED = 2
    FAILED = 3
    CANCELLED = 4

class CallbackType(IntEnum):
    """Callback types that can be registered."""
    PROGRESS = 1
    ERROR = 2
    COMPLETION = 3
    EVENT = 4

class EventType(IntEnum):
    """Event types for the event callback system."""
    FILE_START = 1
    FILE_COMPLETE = 2
    HASH_START = 3
    HASH_COMPLETE = 4
    CACHE_HIT = 5
    CACHE_MISS = 6
    NETWORK_START = 7
    NETWORK_COMPLETE = 8
    MEMORY_WARNING = 9

# Structures
class Config(Structure):
    """Client configuration structure."""
    _fields_ = [
        ("cache_dir", c_char_p),
        ("max_concurrent_files", c_size_t),
        ("chunk_size", c_size_t),
        ("max_memory_usage", c_size_t),
        ("enable_debug_logging", c_int),
        ("username", c_char_p),
        ("password", c_char_p),
    ]

class ProcessOptions(Structure):
    """File processing options."""
    _fields_ = [
        ("algorithms", POINTER(c_int)),
        ("algorithm_count", c_size_t),
        ("enable_progress", c_int),
        ("verify_existing", c_int),
        ("progress_callback", c_void_p),
        ("user_data", c_void_p),
    ]

class HashResult(Structure):
    """Hash result structure."""
    _fields_ = [
        ("algorithm", c_int),
        ("hash_value", c_char_p),
        ("hash_length", c_size_t),
    ]

class FileResult(Structure):
    """File processing result."""
    _fields_ = [
        ("file_path", c_char_p),
        ("file_size", c_uint64),
        ("status", c_int),
        ("hashes", POINTER(HashResult)),
        ("hash_count", c_size_t),
        ("processing_time_ms", c_uint64),
        ("error_message", c_char_p),
    ]

class AnimeInfo(Structure):
    """Anime identification information."""
    _fields_ = [
        ("anime_id", c_uint64),
        ("episode_id", c_uint64),
        ("title", c_char_p),
        ("episode_number", c_uint32),
        ("confidence", c_double),
        ("source", c_int),
    ]

class BatchOptions(Structure):
    """Batch processing options."""
    _fields_ = [
        ("algorithms", POINTER(c_int)),
        ("algorithm_count", c_size_t),
        ("max_concurrent", c_size_t),
        ("continue_on_error", c_int),
        ("skip_existing", c_int),
        ("progress_callback", c_void_p),
        ("completion_callback", c_void_p),
        ("user_data", c_void_p),
    ]

class BatchResult(Structure):
    """Batch processing result."""
    _fields_ = [
        ("total_files", c_size_t),
        ("successful_files", c_size_t),
        ("failed_files", c_size_t),
        ("results", POINTER(FileResult)),
        ("total_time_ms", c_uint64),
    ]

# Event data structures
class FileEventData(Structure):
    """Data for file events."""
    _fields_ = [
        ("file_path", c_char_p),
        ("file_size", c_uint64),
    ]

class HashEventData(Structure):
    """Data for hash events."""
    _fields_ = [
        ("algorithm", c_int),
        ("hash_value", c_char_p),
    ]

class CacheEventData(Structure):
    """Data for cache events."""
    _fields_ = [
        ("file_path", c_char_p),
        ("algorithm", c_int),
    ]

class NetworkEventData(Structure):
    """Data for network events."""
    _fields_ = [
        ("endpoint", c_char_p),
        ("status_code", c_int),
    ]

class MemoryEventData(Structure):
    """Data for memory events."""
    _fields_ = [
        ("current_usage", c_uint64),
        ("max_usage", c_uint64),
    ]

class EventData(Union):
    """Event data union for different event types."""
    _fields_ = [
        ("file", FileEventData),
        ("hash", HashEventData),
        ("cache", CacheEventData),
        ("network", NetworkEventData),
        ("memory", MemoryEventData),
    ]

class Event(Structure):
    """Event structure for event callbacks."""
    _fields_ = [
        ("type", c_int),
        ("timestamp", c_uint64),
        ("data", EventData),
        ("context", c_char_p),
    ]

# Callback function types
ProgressCallback = ctypes.CFUNCTYPE(None, c_float, c_uint64, c_uint64, c_void_p)
ErrorCallback = ctypes.CFUNCTYPE(None, c_int, c_char_p, c_char_p, c_void_p)
CompletionCallback = ctypes.CFUNCTYPE(None, c_int, c_void_p)
EventCallback = ctypes.CFUNCTYPE(None, POINTER(Event), c_void_p)

# Function signatures
# Library initialization
lib.anidb_init.argtypes = [c_uint32]
lib.anidb_init.restype = c_int

lib.anidb_cleanup.argtypes = []
lib.anidb_cleanup.restype = None

lib.anidb_get_version.argtypes = []
lib.anidb_get_version.restype = c_char_p

lib.anidb_get_abi_version.argtypes = []
lib.anidb_get_abi_version.restype = c_uint32

# Client management
lib.anidb_client_create.argtypes = [POINTER(c_void_p)]
lib.anidb_client_create.restype = c_int

lib.anidb_client_create_with_config.argtypes = [POINTER(Config), POINTER(c_void_p)]
lib.anidb_client_create_with_config.restype = c_int

lib.anidb_client_destroy.argtypes = [c_void_p]
lib.anidb_client_destroy.restype = c_int

lib.anidb_client_get_last_error.argtypes = [c_void_p, c_char_p, c_size_t]
lib.anidb_client_get_last_error.restype = c_int

# File processing
lib.anidb_process_file.argtypes = [c_void_p, c_char_p, POINTER(ProcessOptions), POINTER(POINTER(FileResult))]
lib.anidb_process_file.restype = c_int

# Hash calculation
lib.anidb_calculate_hash.argtypes = [c_char_p, c_int, c_char_p, c_size_t]
lib.anidb_calculate_hash.restype = c_int

lib.anidb_calculate_hash_buffer.argtypes = [POINTER(c_uint8), c_size_t, c_int, c_char_p, c_size_t]
lib.anidb_calculate_hash_buffer.restype = c_int

# Cache management
lib.anidb_cache_clear.argtypes = [c_void_p]
lib.anidb_cache_clear.restype = c_int

lib.anidb_cache_get_stats.argtypes = [c_void_p, POINTER(c_size_t), POINTER(c_uint64)]
lib.anidb_cache_get_stats.restype = c_int

lib.anidb_cache_check_file.argtypes = [c_void_p, c_char_p, c_int, POINTER(c_int)]
lib.anidb_cache_check_file.restype = c_int

# Anime identification
lib.anidb_identify_file.argtypes = [c_void_p, c_char_p, c_uint64, POINTER(POINTER(AnimeInfo))]
lib.anidb_identify_file.restype = c_int

# Memory management
lib.anidb_free_string.argtypes = [c_char_p]
lib.anidb_free_string.restype = None

lib.anidb_free_file_result.argtypes = [POINTER(FileResult)]
lib.anidb_free_file_result.restype = None

lib.anidb_free_batch_result.argtypes = [POINTER(BatchResult)]
lib.anidb_free_batch_result.restype = None

lib.anidb_free_anime_info.argtypes = [POINTER(AnimeInfo)]
lib.anidb_free_anime_info.restype = None

# Callback management
lib.anidb_register_callback.argtypes = [c_void_p, c_int, c_void_p, c_void_p]
lib.anidb_register_callback.restype = c_uint64

lib.anidb_unregister_callback.argtypes = [c_void_p, c_uint64]
lib.anidb_unregister_callback.restype = c_int

lib.anidb_event_connect.argtypes = [c_void_p, EventCallback, c_void_p]
lib.anidb_event_connect.restype = c_int

lib.anidb_event_disconnect.argtypes = [c_void_p]
lib.anidb_event_disconnect.restype = c_int

lib.anidb_event_poll.argtypes = [c_void_p, POINTER(Event), c_size_t, POINTER(c_size_t)]
lib.anidb_event_poll.restype = c_int

# Utility functions
lib.anidb_error_string.argtypes = [c_int]
lib.anidb_error_string.restype = c_char_p

lib.anidb_hash_algorithm_name.argtypes = [c_int]
lib.anidb_hash_algorithm_name.restype = c_char_p

lib.anidb_hash_buffer_size.argtypes = [c_int]
lib.anidb_hash_buffer_size.restype = c_size_t