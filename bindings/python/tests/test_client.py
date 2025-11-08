"""
Tests for the AniDB client.
"""

import asyncio
import tempfile
from pathlib import Path
from unittest.mock import Mock, patch

import pytest

from anidb_client import (
    AniDBClient,
    AniDBError,
    ClientConfig,
    FileNotFoundError,
    HashAlgorithm,
    ProcessOptions,
    ProcessingStatus,
)


@pytest.fixture
def temp_dir():
    """Create a temporary directory for tests."""
    with tempfile.TemporaryDirectory() as tmpdir:
        yield Path(tmpdir)


@pytest.fixture
def test_file(temp_dir):
    """Create a test file with known content."""
    test_path = temp_dir / "test.bin"
    # Create a 1MB test file
    test_data = b"x" * 1024 * 1024
    test_path.write_bytes(test_data)
    return test_path


@pytest.fixture
def client(temp_dir):
    """Create a test client."""
    config = ClientConfig(cache_dir=temp_dir / "cache")
    with AniDBClient(config) as client:
        yield client


class TestClientInitialization:
    """Test client initialization and configuration."""
    
    def test_default_initialization(self):
        """Test creating client with default config."""
        with AniDBClient() as client:
            assert client is not None
            assert not client._closed
    
    def test_custom_config(self, temp_dir):
        """Test creating client with custom config."""
        config = ClientConfig(
            cache_dir=temp_dir / "custom_cache",
            max_concurrent_files=8,
            chunk_size=131072,  # 128KB
            enable_debug_logging=True,
        )
        
        with AniDBClient(config) as client:
            assert client is not None
    
    def test_context_manager(self):
        """Test context manager functionality."""
        with AniDBClient() as client:
            assert not client._closed
        
        # Client should be closed after context
        assert client._closed
    
    def test_explicit_close(self):
        """Test explicit close method."""
        client = AniDBClient()
        assert not client._closed
        
        client.close()
        assert client._closed
        
        # Multiple closes should be safe
        client.close()
        assert client._closed
    
    def test_operations_after_close(self):
        """Test that operations fail after close."""
        client = AniDBClient()
        client.close()
        
        with pytest.raises(ValueError, match="Client is closed"):
            client.process_file("test.mkv")


class TestFileProcessing:
    """Test file processing functionality."""
    
    def test_process_single_file(self, client, test_file):
        """Test processing a single file."""
        result = client.process_file(test_file)
        
        assert result.file_path == test_file
        assert result.file_size == 1024 * 1024
        assert result.status == ProcessingStatus.COMPLETED
        assert HashAlgorithm.ED2K in result.hashes
        assert result.processing_time_ms > 0
    
    def test_process_multiple_algorithms(self, client, test_file):
        """Test processing with multiple hash algorithms."""
        options = ProcessOptions(
            algorithms=[HashAlgorithm.ED2K, HashAlgorithm.MD5, HashAlgorithm.SHA1]
        )
        
        result = client.process_file(test_file, options)
        
        assert len(result.hashes) == 3
        assert HashAlgorithm.ED2K in result.hashes
        assert HashAlgorithm.MD5 in result.hashes
        assert HashAlgorithm.SHA1 in result.hashes
    
    def test_process_nonexistent_file(self, client):
        """Test processing a file that doesn't exist."""
        with pytest.raises(FileNotFoundError):
            client.process_file("nonexistent.mkv")
    
    def test_progress_callback(self, client, test_file):
        """Test progress callback functionality."""
        progress_values = []
        
        def progress_callback(percentage, bytes_processed, total_bytes):
            progress_values.append((percentage, bytes_processed, total_bytes))
        
        options = ProcessOptions(
            enable_progress=True,
            progress_callback=progress_callback
        )
        
        client.process_file(test_file, options)
        
        # Should have received progress updates
        assert len(progress_values) > 0
        
        # Final progress should be 100%
        final_percentage = progress_values[-1][0]
        assert final_percentage >= 99.0
    
    @pytest.mark.asyncio
    async def test_process_file_async(self, client, test_file):
        """Test async file processing."""
        result = await client.process_file_async(test_file)
        
        assert result.file_path == test_file
        assert result.status == ProcessingStatus.COMPLETED
        assert HashAlgorithm.ED2K in result.hashes


class TestBatchProcessing:
    """Test batch processing functionality."""
    
    def test_process_batch(self, client, temp_dir):
        """Test processing multiple files."""
        # Create test files
        files = []
        for i in range(3):
            file_path = temp_dir / f"test{i}.bin"
            file_path.write_bytes(b"test data" * 100)
            files.append(file_path)
        
        result = client.process_batch(files)
        
        assert result.total_files == 3
        assert result.successful_files == 3
        assert result.failed_files == 0
        assert len(result.results) == 3
        assert result.success_rate == 100.0
    
    def test_batch_with_errors(self, client, temp_dir):
        """Test batch processing with some failures."""
        files = [
            temp_dir / "exists.bin",
            temp_dir / "missing.bin",
        ]
        
        # Create only the first file
        files[0].write_bytes(b"test data")
        
        result = client.process_batch(files)
        
        assert result.total_files == 2
        assert result.successful_files == 1
        assert result.failed_files == 1
    
    @pytest.mark.asyncio
    async def test_process_batch_async(self, client, temp_dir):
        """Test async batch processing."""
        files = []
        for i in range(2):
            file_path = temp_dir / f"async{i}.bin"
            file_path.write_bytes(b"async test" * 50)
            files.append(file_path)
        
        result = await client.process_batch_async(files)
        
        assert result.total_files == 2
        assert result.successful_files == 2


class TestHashCalculation:
    """Test hash calculation functionality."""
    
    def test_calculate_hash_from_file(self, client, test_file):
        """Test calculating hash from file."""
        ed2k_hash = client.calculate_hash(test_file, HashAlgorithm.ED2K)
        
        assert ed2k_hash is not None
        assert len(ed2k_hash) == 32  # ED2K hash is 32 hex chars
        assert all(c in "0123456789abcdef" for c in ed2k_hash.lower())
    
    def test_calculate_hash_from_bytes(self, client):
        """Test calculating hash from byte buffer."""
        test_data = b"Hello, AniDB!" * 1000
        
        md5_hash = client.calculate_hash_from_bytes(test_data, HashAlgorithm.MD5)
        
        assert md5_hash is not None
        assert len(md5_hash) == 32  # MD5 hash is 32 hex chars
    
    def test_calculate_hash_empty_bytes(self, client):
        """Test that empty bytes raise an error."""
        with pytest.raises(ValueError, match="Data cannot be empty"):
            client.calculate_hash_from_bytes(b"", HashAlgorithm.MD5)


class TestCacheManagement:
    """Test cache management functionality."""
    
    @pytest.mark.skip(reason="Cache operations not fully implemented in FFI")
    def test_cache_operations(self, client, test_file):
        """Test cache operations."""
        # Process a file to populate cache
        client.process_file(test_file)
        
        # Check if file is cached
        is_cached = client.is_file_cached(test_file)
        assert is_cached
        
        # Get cache stats
        entries, size = client.get_cache_stats()
        assert entries > 0
        assert size > 0
        
        # Clear cache
        client.clear_cache()
        
        # Verify cache is empty
        entries, size = client.get_cache_stats()
        assert entries == 0


class TestCallbacks:
    """Test callback registration and management."""
    
    def test_register_progress_callback(self, client):
        """Test registering a progress callback."""
        called = False
        
        def progress_callback(percentage, bytes_processed, total_bytes):
            nonlocal called
            called = True
        
        callback_id = client.register_callback(
            CallbackType.PROGRESS,
            progress_callback
        )
        
        assert callback_id > 0
        
        # Unregister
        client.unregister_callback(callback_id)
    
    def test_register_error_callback(self, client):
        """Test registering an error callback."""
        def error_callback(error_code, error_msg, file_path):
            pass
        
        callback_id = client.register_callback(
            CallbackType.ERROR,
            error_callback
        )
        
        assert callback_id > 0
        client.unregister_callback(callback_id)


class TestEventSystem:
    """Test event system functionality."""
    
    def test_connect_events(self, client):
        """Test connecting to event system."""
        events = []
        
        def event_callback(event):
            events.append(event)
        
        client.connect_events(event_callback)
        
        # Disconnect
        client.disconnect_events()
    
    def test_poll_events(self, client):
        """Test polling for events."""
        events = client.poll_events(max_events=10)
        
        # Should return empty list if no events
        assert isinstance(events, list)


class TestUtilities:
    """Test utility functions."""
    
    def test_get_version(self):
        """Test getting library version."""
        version = AniDBClient.get_version()
        assert version is not None
        assert isinstance(version, str)
    
    def test_get_abi_version(self):
        """Test getting ABI version."""
        abi_version = AniDBClient.get_abi_version()
        assert abi_version > 0
    
    def test_create_factory(self, temp_dir):
        """Test create factory method."""
        with AniDBClient.create(cache_dir=temp_dir / "factory") as client:
            assert client is not None
            assert not client._closed