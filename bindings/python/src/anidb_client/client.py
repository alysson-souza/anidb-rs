"""
High-level Python client for the AniDB Client Core Library.
"""

import asyncio
import ctypes
import threading
from contextlib import contextmanager
from pathlib import Path
from typing import (
    Any, Callable, Dict, List, Optional, Union, Iterator, Tuple
)

from . import native
from .exceptions import AniDBError
from .types import (
    AnimeInfo, BatchOptions, BatchResult, CallbackType, ClientConfig,
    Event, EventType, FileResult, HashAlgorithm, ProcessOptions,
    ProcessingStatus,
)


class AniDBClient:
    """
    AniDB Client for file hashing and anime identification.
    
    This client provides a pythonic interface to the AniDB Client Core Library,
    supporting both synchronous and asynchronous operations with context manager
    support for automatic cleanup.
    
    Example:
        >>> with AniDBClient() as client:
        ...     result = client.process_file("anime.mkv")
        ...     print(f"ED2K: {result.get_hash(HashAlgorithm.ED2K)}")
    """
    
    def __init__(self, config: Optional[ClientConfig] = None):
        """
        Initialize the AniDB client.
        
        Args:
            config: Client configuration (uses defaults if None)
            
        Raises:
            AniDBError: If initialization fails
        """
        self._handle: Optional[ctypes.c_void_p] = None
        self._callbacks: Dict[int, Tuple[Any, Any]] = {}
        self._event_thread: Optional[threading.Thread] = None
        self._event_callback: Optional[Callable] = None
        self._lock = threading.Lock()
        self._closed = False
        
        # Initialize the library
        result = native.lib.anidb_init(native.ABI_VERSION)
        if result != native.Result.SUCCESS:
            raise AniDBError.from_result(result, "Failed to initialize AniDB library")
        
        # Create client handle
        handle = ctypes.c_void_p()
        
        if config is None:
            result = native.lib.anidb_client_create(ctypes.byref(handle))
        else:
            # Convert Python config to native
            native_config = native.Config()
            native_config.cache_dir = str(config.cache_dir).encode('utf-8')
            native_config.max_concurrent_files = config.max_concurrent_files
            native_config.chunk_size = config.chunk_size
            native_config.max_memory_usage = config.max_memory_usage
            native_config.enable_debug_logging = int(config.enable_debug_logging)
            native_config.username = config.username.encode('utf-8') if config.username else None
            native_config.password = config.password.encode('utf-8') if config.password else None
            
            result = native.lib.anidb_client_create_with_config(
                ctypes.byref(native_config),
                ctypes.byref(handle)
            )
        
        if result != native.Result.SUCCESS:
            raise AniDBError.from_result(result, "Failed to create AniDB client")
        
        self._handle = handle
    
    def __enter__(self) -> 'AniDBClient':
        """Context manager entry."""
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        """Context manager exit."""
        self.close()
    
    def close(self) -> None:
        """
        Close the client and release resources.
        
        This is automatically called when using the client as a context manager.
        """
        with self._lock:
            if self._closed:
                return
            
            self._closed = True
            
            # Disconnect event system
            if self._event_callback is not None:
                self._disconnect_events()
            
            # Destroy handle
            if self._handle is not None:
                native.lib.anidb_client_destroy(self._handle)
                self._handle = None
    
    def _check_closed(self) -> None:
        """Check if the client is closed."""
        if self._closed:
            raise ValueError("Client is closed")
    
    def _get_last_error(self) -> str:
        """Get the last error message from the library."""
        self._check_closed()
        
        buffer_size = 1024
        buffer = ctypes.create_string_buffer(buffer_size)
        
        result = native.lib.anidb_client_get_last_error(
            self._handle,
            buffer,
            buffer_size
        )
        
        if result == native.Result.SUCCESS:
            return buffer.value.decode('utf-8', errors='replace')
        else:
            return "Unknown error"
    
    def process_file(
        self,
        file_path: Union[str, Path],
        options: Optional[ProcessOptions] = None
    ) -> FileResult:
        """
        Process a single file synchronously.
        
        Args:
            file_path: Path to the file to process
            options: Processing options (uses defaults if None)
            
        Returns:
            FileResult: Processing result
            
        Raises:
            AniDBError: If processing fails
        """
        self._check_closed()
        
        if options is None:
            options = ProcessOptions()
        
        # Convert path to string
        file_path_str = str(Path(file_path).absolute())
        
        # Convert options to native
        algorithms = (ctypes.c_int * len(options.algorithms))()
        for i, algo in enumerate(options.algorithms):
            algorithms[i] = algo.value
        
        # Handle progress callback
        progress_ref = None
        if options.progress_callback:
            def progress_wrapper(percentage, bytes_processed, total_bytes, user_data):
                options.progress_callback(percentage, bytes_processed, total_bytes)
            
            progress_ref = native.ProgressCallback(progress_wrapper)
        
        native_options = native.ProcessOptions()
        native_options.algorithms = ctypes.cast(algorithms, ctypes.POINTER(ctypes.c_int))
        native_options.algorithm_count = len(options.algorithms)
        native_options.enable_progress = int(options.enable_progress)
        native_options.verify_existing = int(options.verify_existing)
        native_options.progress_callback = ctypes.cast(progress_ref, ctypes.c_void_p) if progress_ref else None
        native_options.user_data = None
        
        # Process file
        result_ptr = ctypes.POINTER(native.FileResult)()
        
        result = native.lib.anidb_process_file(
            self._handle,
            file_path_str.encode('utf-8'),
            ctypes.byref(native_options),
            ctypes.byref(result_ptr)
        )
        
        if result != native.Result.SUCCESS:
            error_msg = self._get_last_error()
            raise AniDBError.from_result(result, error_msg, file_path_str)
        
        try:
            # Convert native result to Python
            native_result = result_ptr.contents
            
            # Extract hashes
            hashes = {}
            if native_result.hash_count > 0 and native_result.hashes:
                for i in range(native_result.hash_count):
                    hash_result = native_result.hashes[i]
                    algo = HashAlgorithm(hash_result.algorithm)
                    hash_value = hash_result.hash_value.decode('utf-8')
                    hashes[algo] = hash_value
            
            # Create Python result
            return FileResult(
                file_path=Path(native_result.file_path.decode('utf-8')),
                file_size=native_result.file_size,
                status=ProcessingStatus(native_result.status),
                hashes=hashes,
                processing_time_ms=native_result.processing_time_ms,
                error_message=native_result.error_message.decode('utf-8') if native_result.error_message else None
            )
        finally:
            # Free native result
            native.lib.anidb_free_file_result(result_ptr)
    
    async def process_file_async(
        self,
        file_path: Union[str, Path],
        options: Optional[ProcessOptions] = None
    ) -> FileResult:
        """
        Process a single file asynchronously.
        
        Args:
            file_path: Path to the file to process
            options: Processing options (uses defaults if None)
            
        Returns:
            FileResult: Processing result
            
        Raises:
            AniDBError: If processing fails
        """
        # Run synchronous method in thread pool
        loop = asyncio.get_event_loop()
        return await loop.run_in_executor(
            None,
            self.process_file,
            file_path,
            options
        )
    
    def process_batch(
        self,
        file_paths: List[Union[str, Path]],
        options: Optional[BatchOptions] = None
    ) -> BatchResult:
        """
        Process multiple files in batch.
        
        Args:
            file_paths: List of file paths to process
            options: Batch processing options (uses defaults if None)
            
        Returns:
            BatchResult: Batch processing result
            
        Raises:
            AniDBError: If processing fails
        """
        self._check_closed()
        
        if not file_paths:
            return BatchResult(
                total_files=0,
                successful_files=0,
                failed_files=0,
                results=[],
                total_time_ms=0
            )
        
        if options is None:
            options = BatchOptions()
        
        # Process files individually (batch API not fully implemented in FFI)
        results = []
        successful = 0
        failed = 0
        start_time = asyncio.get_event_loop().time()
        
        for file_path in file_paths:
            try:
                result = self.process_file(
                    file_path,
                    ProcessOptions(
                        algorithms=options.algorithms,
                        enable_progress=True,
                        verify_existing=False,
                        progress_callback=options.progress_callback
                    )
                )
                results.append(result)
                if result.is_successful:
                    successful += 1
                else:
                    failed += 1
            except AniDBError as e:
                if not options.continue_on_error:
                    raise
                
                # Create failed result
                results.append(FileResult(
                    file_path=Path(file_path),
                    file_size=0,
                    status=ProcessingStatus.FAILED,
                    hashes={},
                    processing_time_ms=0,
                    error_message=str(e)
                ))
                failed += 1
        
        total_time_ms = int((asyncio.get_event_loop().time() - start_time) * 1000)
        
        batch_result = BatchResult(
            total_files=len(file_paths),
            successful_files=successful,
            failed_files=failed,
            results=results,
            total_time_ms=total_time_ms
        )
        
        if options.completion_callback:
            options.completion_callback(batch_result.success_rate == 100.0)
        
        return batch_result
    
    async def process_batch_async(
        self,
        file_paths: List[Union[str, Path]],
        options: Optional[BatchOptions] = None
    ) -> BatchResult:
        """
        Process multiple files in batch asynchronously.
        
        Args:
            file_paths: List of file paths to process
            options: Batch processing options (uses defaults if None)
            
        Returns:
            BatchResult: Batch processing result
            
        Raises:
            AniDBError: If processing fails
        """
        # Run synchronous method in thread pool
        loop = asyncio.get_event_loop()
        return await loop.run_in_executor(
            None,
            self.process_batch,
            file_paths,
            options
        )
    
    def calculate_hash(
        self,
        file_path: Union[str, Path],
        algorithm: HashAlgorithm = HashAlgorithm.ED2K
    ) -> str:
        """
        Calculate a single hash for a file.
        
        Args:
            file_path: Path to the file
            algorithm: Hash algorithm to use
            
        Returns:
            str: Hash value as hexadecimal string
            
        Raises:
            AniDBError: If calculation fails
        """
        self._check_closed()
        
        file_path_str = str(Path(file_path).absolute())
        buffer_size = algorithm.buffer_size
        buffer = ctypes.create_string_buffer(buffer_size)
        
        result = native.lib.anidb_calculate_hash(
            file_path_str.encode('utf-8'),
            algorithm.value,
            buffer,
            buffer_size
        )
        
        if result != native.Result.SUCCESS:
            raise AniDBError.from_result(result, f"Failed to calculate {algorithm.name} hash", file_path_str)
        
        return buffer.value.decode('utf-8')
    
    def calculate_hash_from_bytes(
        self,
        data: bytes,
        algorithm: HashAlgorithm = HashAlgorithm.ED2K
    ) -> str:
        """
        Calculate hash for a byte buffer.
        
        Args:
            data: Data to hash
            algorithm: Hash algorithm to use
            
        Returns:
            str: Hash value as hexadecimal string
            
        Raises:
            AniDBError: If calculation fails
        """
        self._check_closed()
        
        if not data:
            raise ValueError("Data cannot be empty")
        
        buffer_size = algorithm.buffer_size
        buffer = ctypes.create_string_buffer(buffer_size)
        
        # Create ctypes buffer from bytes
        data_buffer = (ctypes.c_uint8 * len(data)).from_buffer_copy(data)
        
        result = native.lib.anidb_calculate_hash_buffer(
            data_buffer,
            len(data),
            algorithm.value,
            buffer,
            buffer_size
        )
        
        if result != native.Result.SUCCESS:
            raise AniDBError.from_result(result, f"Failed to calculate {algorithm.name} hash from buffer")
        
        return buffer.value.decode('utf-8')
    
    def identify_file(
        self,
        ed2k_hash: str,
        file_size: int
    ) -> Optional[AnimeInfo]:
        """
        Identify an anime file by its ED2K hash and size.
        
        Args:
            ed2k_hash: ED2K hash of the file
            file_size: File size in bytes
            
        Returns:
            AnimeInfo: Anime identification info, or None if not found
            
        Raises:
            AniDBError: If identification fails
        """
        self._check_closed()
        
        info_ptr = ctypes.POINTER(native.AnimeInfo)()
        
        result = native.lib.anidb_identify_file(
            self._handle,
            ed2k_hash.encode('utf-8'),
            file_size,
            ctypes.byref(info_ptr)
        )
        
        if result == native.Result.ERROR_NETWORK:
            # Network error or file not found
            return None
        elif result != native.Result.SUCCESS:
            raise AniDBError.from_result(result, "Failed to identify file")
        
        try:
            if not info_ptr:
                return None
            
            native_info = info_ptr.contents
            
            return AnimeInfo(
                anime_id=native_info.anime_id,
                episode_id=native_info.episode_id,
                title=native_info.title.decode('utf-8'),
                episode_number=native_info.episode_number,
                confidence=native_info.confidence,
                source=str(native_info.source)
            )
        finally:
            if info_ptr:
                native.lib.anidb_free_anime_info(info_ptr)
    
    def clear_cache(self) -> None:
        """
        Clear the hash cache.
        
        Raises:
            AniDBError: If clearing fails
        """
        self._check_closed()
        
        result = native.lib.anidb_cache_clear(self._handle)
        if result != native.Result.SUCCESS:
            raise AniDBError.from_result(result, "Failed to clear cache")
    
    def get_cache_stats(self) -> Tuple[int, int]:
        """
        Get cache statistics.
        
        Returns:
            Tuple[int, int]: (total_entries, cache_size_bytes)
            
        Raises:
            AniDBError: If getting stats fails
        """
        self._check_closed()
        
        total_entries = ctypes.c_size_t()
        cache_size = ctypes.c_uint64()
        
        result = native.lib.anidb_cache_get_stats(
            self._handle,
            ctypes.byref(total_entries),
            ctypes.byref(cache_size)
        )
        
        if result != native.Result.SUCCESS:
            raise AniDBError.from_result(result, "Failed to get cache stats")
        
        return (total_entries.value, cache_size.value)
    
    def is_file_cached(
        self,
        file_path: Union[str, Path],
        algorithm: HashAlgorithm = HashAlgorithm.ED2K
    ) -> bool:
        """
        Check if a file's hash is in the cache.
        
        Args:
            file_path: Path to the file
            algorithm: Hash algorithm to check
            
        Returns:
            bool: True if cached, False otherwise
            
        Raises:
            AniDBError: If checking fails
        """
        self._check_closed()
        
        file_path_str = str(Path(file_path).absolute())
        is_cached = ctypes.c_int()
        
        result = native.lib.anidb_cache_check_file(
            self._handle,
            file_path_str.encode('utf-8'),
            algorithm.value,
            ctypes.byref(is_cached)
        )
        
        if result != native.Result.SUCCESS:
            raise AniDBError.from_result(result, "Failed to check cache", file_path_str)
        
        return bool(is_cached.value)
    
    def register_callback(
        self,
        callback_type: CallbackType,
        callback: Callable[..., None]
    ) -> int:
        """
        Register a callback function.
        
        Args:
            callback_type: Type of callback to register
            callback: Callback function
            
        Returns:
            int: Callback ID for unregistration
            
        Raises:
            AniDBError: If registration fails
        """
        self._check_closed()
        
        # Create appropriate wrapper based on callback type
        if callback_type == CallbackType.PROGRESS:
            def wrapper(percentage, bytes_processed, total_bytes, user_data):
                callback(float(percentage), int(bytes_processed), int(total_bytes))
            
            native_callback = native.ProgressCallback(wrapper)
        
        elif callback_type == CallbackType.ERROR:
            def wrapper(error_code, error_msg, file_path, user_data):
                error_msg_str = error_msg.decode('utf-8') if error_msg else ""
                file_path_str = file_path.decode('utf-8') if file_path else None
                callback(error_code, error_msg_str, file_path_str)
            
            native_callback = native.ErrorCallback(wrapper)
        
        elif callback_type == CallbackType.COMPLETION:
            def wrapper(result_code, user_data):
                callback(result_code == native.Result.SUCCESS)
            
            native_callback = native.CompletionCallback(wrapper)
        
        else:
            raise ValueError(f"Unsupported callback type: {callback_type}")
        
        # Register with library
        callback_id = native.lib.anidb_register_callback(
            self._handle,
            callback_type.value,
            ctypes.cast(native_callback, ctypes.c_void_p),
            None
        )
        
        if callback_id == 0:
            raise AniDBError("Failed to register callback")
        
        # Store reference to prevent garbage collection
        self._callbacks[callback_id] = (native_callback, callback)
        
        return callback_id
    
    def unregister_callback(self, callback_id: int) -> None:
        """
        Unregister a callback.
        
        Args:
            callback_id: ID returned by register_callback
            
        Raises:
            AniDBError: If unregistration fails
        """
        self._check_closed()
        
        result = native.lib.anidb_unregister_callback(self._handle, callback_id)
        
        if result != native.Result.SUCCESS:
            raise AniDBError.from_result(result, "Failed to unregister callback")
        
        # Remove reference
        self._callbacks.pop(callback_id, None)
    
    def connect_events(self, callback: Callable[[Event], None]) -> None:
        """
        Connect to the event system for receiving events.
        
        Args:
            callback: Function to call for each event
            
        Raises:
            AniDBError: If connection fails
        """
        self._check_closed()
        
        if self._event_callback is not None:
            raise ValueError("Event system already connected")
        
        def event_wrapper(event_ptr, user_data):
            if not event_ptr:
                return
            
            try:
                native_event = event_ptr.contents
                
                # Extract event data based on type
                event_type = EventType(native_event.type)
                data = {}
                
                if event_type in (EventType.FILE_START, EventType.FILE_COMPLETE):
                    data['file_path'] = native_event.data.file.file_path.decode('utf-8') if native_event.data.file.file_path else ""
                    data['file_size'] = native_event.data.file.file_size
                
                elif event_type in (EventType.HASH_START, EventType.HASH_COMPLETE):
                    data['algorithm'] = HashAlgorithm(native_event.data.hash.algorithm).name
                    data['hash_value'] = native_event.data.hash.hash_value.decode('utf-8') if native_event.data.hash.hash_value else ""
                
                elif event_type in (EventType.CACHE_HIT, EventType.CACHE_MISS):
                    data['file_path'] = native_event.data.cache.file_path.decode('utf-8') if native_event.data.cache.file_path else ""
                    data['algorithm'] = HashAlgorithm(native_event.data.cache.algorithm).name
                
                elif event_type in (EventType.NETWORK_START, EventType.NETWORK_COMPLETE):
                    data['endpoint'] = native_event.data.network.endpoint.decode('utf-8') if native_event.data.network.endpoint else ""
                    data['status_code'] = native_event.data.network.status_code
                
                elif event_type == EventType.MEMORY_WARNING:
                    data['current_usage'] = native_event.data.memory.current_usage
                    data['max_usage'] = native_event.data.memory.max_usage
                
                # Create Python event
                event = Event(
                    type=event_type,
                    timestamp=native_event.timestamp,
                    data=data,
                    context=native_event.context.decode('utf-8') if native_event.context else None
                )
                
                # Call user callback
                callback(event)
            except Exception as e:
                # Swallow exceptions to prevent crashes
                pass
        
        # Create native callback
        native_callback = native.EventCallback(event_wrapper)
        
        # Connect to event system
        result = native.lib.anidb_event_connect(
            self._handle,
            native_callback,
            None
        )
        
        if result != native.Result.SUCCESS:
            raise AniDBError.from_result(result, "Failed to connect event system")
        
        self._event_callback = (native_callback, callback)
    
    def _disconnect_events(self) -> None:
        """Disconnect from the event system."""
        if self._event_callback is None:
            return
        
        native.lib.anidb_event_disconnect(self._handle)
        self._event_callback = None
    
    def disconnect_events(self) -> None:
        """
        Disconnect from the event system.
        
        Raises:
            AniDBError: If disconnection fails
        """
        self._check_closed()
        self._disconnect_events()
    
    def poll_events(self, max_events: int = 100) -> List[Event]:
        """
        Poll for events without using callbacks.
        
        Args:
            max_events: Maximum number of events to retrieve
            
        Returns:
            List[Event]: List of events
            
        Raises:
            AniDBError: If polling fails
        """
        self._check_closed()
        
        events = (native.Event * max_events)()
        event_count = ctypes.c_size_t()
        
        result = native.lib.anidb_event_poll(
            self._handle,
            events,
            max_events,
            ctypes.byref(event_count)
        )
        
        if result != native.Result.SUCCESS:
            raise AniDBError.from_result(result, "Failed to poll events")
        
        # Convert to Python events
        python_events = []
        for i in range(event_count.value):
            native_event = events[i]
            
            # Extract event data based on type
            event_type = EventType(native_event.type)
            data = {}
            
            if event_type in (EventType.FILE_START, EventType.FILE_COMPLETE):
                data['file_path'] = native_event.data.file.file_path.decode('utf-8') if native_event.data.file.file_path else ""
                data['file_size'] = native_event.data.file.file_size
            
            elif event_type in (EventType.HASH_START, EventType.HASH_COMPLETE):
                data['algorithm'] = HashAlgorithm(native_event.data.hash.algorithm).name
                data['hash_value'] = native_event.data.hash.hash_value.decode('utf-8') if native_event.data.hash.hash_value else ""
            
            elif event_type in (EventType.CACHE_HIT, EventType.CACHE_MISS):
                data['file_path'] = native_event.data.cache.file_path.decode('utf-8') if native_event.data.cache.file_path else ""
                data['algorithm'] = HashAlgorithm(native_event.data.cache.algorithm).name
            
            elif event_type in (EventType.NETWORK_START, EventType.NETWORK_COMPLETE):
                data['endpoint'] = native_event.data.network.endpoint.decode('utf-8') if native_event.data.network.endpoint else ""
                data['status_code'] = native_event.data.network.status_code
            
            elif event_type == EventType.MEMORY_WARNING:
                data['current_usage'] = native_event.data.memory.current_usage
                data['max_usage'] = native_event.data.memory.max_usage
            
            # Create Python event
            event = Event(
                type=event_type,
                timestamp=native_event.timestamp,
                data=data,
                context=native_event.context.decode('utf-8') if native_event.context else None
            )
            
            python_events.append(event)
        
        return python_events
    
    @staticmethod
    def get_version() -> str:
        """
        Get the library version string.
        
        Returns:
            str: Version string
        """
        version = native.lib.anidb_get_version()
        return version.decode('utf-8') if version else "Unknown"
    
    @staticmethod
    def get_abi_version() -> int:
        """
        Get the library ABI version.
        
        Returns:
            int: ABI version number
        """
        return native.lib.anidb_get_abi_version()
    
    @staticmethod
    @contextmanager
    def create(**kwargs) -> Iterator['AniDBClient']:
        """
        Create a client using a context manager.
        
        This is a convenience method for creating a client with custom config.
        
        Args:
            **kwargs: Arguments passed to ClientConfig
            
        Yields:
            AniDBClient: Configured client instance
            
        Example:
            >>> with AniDBClient.create(cache_dir="/tmp/anidb") as client:
            ...     result = client.process_file("anime.mkv")
        """
        config = ClientConfig(**kwargs) if kwargs else None
        client = AniDBClient(config)
        try:
            yield client
        finally:
            client.close()