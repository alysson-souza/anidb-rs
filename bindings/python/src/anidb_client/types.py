"""
Type definitions for the AniDB Client Python Library.
"""

from dataclasses import dataclass, field
from enum import IntEnum
from pathlib import Path
from typing import Callable, Dict, List, Optional, Union

from .native import (
    CallbackType as NativeCallbackType,
    EventType as NativeEventType,
    HashAlgorithm as NativeHashAlgorithm,
    Status as NativeStatus,
)


class HashAlgorithm(IntEnum):
    """Hash algorithm identifiers."""
    ED2K = NativeHashAlgorithm.ED2K
    CRC32 = NativeHashAlgorithm.CRC32
    MD5 = NativeHashAlgorithm.MD5
    SHA1 = NativeHashAlgorithm.SHA1
    TTH = NativeHashAlgorithm.TTH
    
    @property
    def name(self) -> str:
        """Get the algorithm name."""
        return self._name_
    
    @property
    def buffer_size(self) -> int:
        """Get the required buffer size for this algorithm."""
        sizes = {
            HashAlgorithm.ED2K: 33,  # 32 hex chars + null
            HashAlgorithm.CRC32: 9,  # 8 hex chars + null
            HashAlgorithm.MD5: 33,   # 32 hex chars + null
            HashAlgorithm.SHA1: 41,  # 40 hex chars + null
            HashAlgorithm.TTH: 40,   # 39 base32 chars + null
        }
        return sizes[self]


class ProcessingStatus(IntEnum):
    """Processing status codes."""
    PENDING = NativeStatus.PENDING
    PROCESSING = NativeStatus.PROCESSING
    COMPLETED = NativeStatus.COMPLETED
    FAILED = NativeStatus.FAILED
    CANCELLED = NativeStatus.CANCELLED


class CallbackType(IntEnum):
    """Callback types that can be registered."""
    PROGRESS = NativeCallbackType.PROGRESS
    ERROR = NativeCallbackType.ERROR
    COMPLETION = NativeCallbackType.COMPLETION
    EVENT = NativeCallbackType.EVENT


class EventType(IntEnum):
    """Event types for the event callback system."""
    FILE_START = NativeEventType.FILE_START
    FILE_COMPLETE = NativeEventType.FILE_COMPLETE
    HASH_START = NativeEventType.HASH_START
    HASH_COMPLETE = NativeEventType.HASH_COMPLETE
    CACHE_HIT = NativeEventType.CACHE_HIT
    CACHE_MISS = NativeEventType.CACHE_MISS
    NETWORK_START = NativeEventType.NETWORK_START
    NETWORK_COMPLETE = NativeEventType.NETWORK_COMPLETE
    MEMORY_WARNING = NativeEventType.MEMORY_WARNING


@dataclass
class ClientConfig:
    """Client configuration options."""
    cache_dir: Path = field(default_factory=lambda: Path(".anidb_cache"))
    max_concurrent_files: int = 4
    chunk_size: int = 65536  # 64KB
    max_memory_usage: int = 0  # 0 for default
    enable_debug_logging: bool = False
    username: Optional[str] = None
    password: Optional[str] = None
    
    def __post_init__(self):
        """Validate configuration."""
        if not isinstance(self.cache_dir, Path):
            self.cache_dir = Path(self.cache_dir)
        
        if self.max_concurrent_files < 1:
            raise ValueError("max_concurrent_files must be at least 1")
        
        if self.chunk_size < 1024:
            raise ValueError("chunk_size must be at least 1024 bytes")


@dataclass
class ProcessOptions:
    """File processing options."""
    algorithms: List[HashAlgorithm] = field(default_factory=lambda: [HashAlgorithm.ED2K])
    enable_progress: bool = True
    verify_existing: bool = False
    progress_callback: Optional[Callable[[float, int, int], None]] = None
    
    def __post_init__(self):
        """Validate options."""
        if not self.algorithms:
            raise ValueError("At least one hash algorithm must be specified")
        
        # Ensure all algorithms are valid
        for algo in self.algorithms:
            if not isinstance(algo, HashAlgorithm):
                raise TypeError(f"Invalid algorithm type: {type(algo)}")


@dataclass
class BatchOptions:
    """Batch processing options."""
    algorithms: List[HashAlgorithm] = field(default_factory=lambda: [HashAlgorithm.ED2K])
    max_concurrent: int = 4
    continue_on_error: bool = True
    skip_existing: bool = False
    progress_callback: Optional[Callable[[float, int, int], None]] = None
    completion_callback: Optional[Callable[[bool], None]] = None
    
    def __post_init__(self):
        """Validate options."""
        if not self.algorithms:
            raise ValueError("At least one hash algorithm must be specified")
        
        if self.max_concurrent < 1:
            raise ValueError("max_concurrent must be at least 1")


@dataclass
class HashResult:
    """Hash calculation result."""
    algorithm: HashAlgorithm
    hash_value: str
    
    def __str__(self) -> str:
        """String representation."""
        return f"{self.algorithm.name}: {self.hash_value}"


@dataclass
class FileResult:
    """File processing result."""
    file_path: Path
    file_size: int
    status: ProcessingStatus
    hashes: Dict[HashAlgorithm, str]
    processing_time_ms: int
    error_message: Optional[str] = None
    
    @property
    def processing_time_seconds(self) -> float:
        """Get processing time in seconds."""
        return self.processing_time_ms / 1000.0
    
    @property
    def is_successful(self) -> bool:
        """Check if processing was successful."""
        return self.status == ProcessingStatus.COMPLETED
    
    def get_hash(self, algorithm: HashAlgorithm) -> Optional[str]:
        """Get hash value for a specific algorithm."""
        return self.hashes.get(algorithm)


@dataclass
class AnimeInfo:
    """Anime identification information."""
    anime_id: int
    episode_id: int
    title: str
    episode_number: int
    confidence: float
    source: str  # "AniDB", "Cache", or "Filename"
    
    @property
    def source_type(self) -> str:
        """Get human-readable source type."""
        sources = {0: "AniDB", 1: "Cache", 2: "Filename"}
        return sources.get(int(self.source), "Unknown")


@dataclass
class BatchResult:
    """Batch processing result."""
    total_files: int
    successful_files: int
    failed_files: int
    results: List[FileResult]
    total_time_ms: int
    
    @property
    def total_time_seconds(self) -> float:
        """Get total time in seconds."""
        return self.total_time_ms / 1000.0
    
    @property
    def success_rate(self) -> float:
        """Get success rate as a percentage."""
        if self.total_files == 0:
            return 0.0
        return (self.successful_files / self.total_files) * 100.0


@dataclass
class Event:
    """Event information."""
    type: EventType
    timestamp: int  # milliseconds since epoch
    data: Dict[str, Union[str, int, float]]
    context: Optional[str] = None
    
    @property
    def event_name(self) -> str:
        """Get human-readable event name."""
        return self.type.name