"""
Tests for type definitions.
"""

from pathlib import Path

import pytest

from anidb_client import (
    AnimeInfo,
    BatchOptions,
    BatchResult,
    ClientConfig,
    Event,
    EventType,
    FileResult,
    HashAlgorithm,
    HashResult,
    ProcessOptions,
    ProcessingStatus,
)


class TestHashAlgorithm:
    """Test HashAlgorithm enum."""
    
    def test_algorithm_values(self):
        """Test algorithm enum values."""
        assert HashAlgorithm.ED2K.value == 1
        assert HashAlgorithm.CRC32.value == 2
        assert HashAlgorithm.MD5.value == 3
        assert HashAlgorithm.SHA1.value == 4
        assert HashAlgorithm.TTH.value == 5
    
    def test_algorithm_names(self):
        """Test algorithm names."""
        assert HashAlgorithm.ED2K.name == "ED2K"
        assert HashAlgorithm.CRC32.name == "CRC32"
        assert HashAlgorithm.MD5.name == "MD5"
        assert HashAlgorithm.SHA1.name == "SHA1"
        assert HashAlgorithm.TTH.name == "TTH"
    
    def test_buffer_sizes(self):
        """Test buffer size property."""
        assert HashAlgorithm.ED2K.buffer_size == 33
        assert HashAlgorithm.CRC32.buffer_size == 9
        assert HashAlgorithm.MD5.buffer_size == 33
        assert HashAlgorithm.SHA1.buffer_size == 41
        assert HashAlgorithm.TTH.buffer_size == 40


class TestClientConfig:
    """Test ClientConfig dataclass."""
    
    def test_default_config(self):
        """Test default configuration values."""
        config = ClientConfig()
        
        assert config.cache_dir == Path(".anidb_cache")
        assert config.max_concurrent_files == 4
        assert config.chunk_size == 65536
        assert config.max_memory_usage == 0
        assert config.enable_debug_logging is False
        assert config.username is None
        assert config.password is None
    
    def test_custom_config(self):
        """Test custom configuration."""
        config = ClientConfig(
            cache_dir="/tmp/anidb",
            max_concurrent_files=8,
            chunk_size=131072,
            enable_debug_logging=True,
            username="testuser",
            password="testpass",
        )
        
        assert config.cache_dir == Path("/tmp/anidb")
        assert config.max_concurrent_files == 8
        assert config.chunk_size == 131072
        assert config.enable_debug_logging is True
        assert config.username == "testuser"
        assert config.password == "testpass"
    
    def test_path_conversion(self):
        """Test that cache_dir is converted to Path."""
        config = ClientConfig(cache_dir="/tmp/test")
        assert isinstance(config.cache_dir, Path)
    
    def test_validation(self):
        """Test configuration validation."""
        with pytest.raises(ValueError, match="max_concurrent_files must be at least 1"):
            ClientConfig(max_concurrent_files=0)
        
        with pytest.raises(ValueError, match="chunk_size must be at least 1024"):
            ClientConfig(chunk_size=512)


class TestProcessOptions:
    """Test ProcessOptions dataclass."""
    
    def test_default_options(self):
        """Test default processing options."""
        options = ProcessOptions()
        
        assert options.algorithms == [HashAlgorithm.ED2K]
        assert options.enable_progress is True
        assert options.verify_existing is False
        assert options.progress_callback is None
    
    def test_custom_options(self):
        """Test custom processing options."""
        def callback(p, b, t):
            pass
        
        options = ProcessOptions(
            algorithms=[HashAlgorithm.MD5, HashAlgorithm.SHA1],
            enable_progress=False,
            verify_existing=True,
            progress_callback=callback,
        )
        
        assert len(options.algorithms) == 2
        assert HashAlgorithm.MD5 in options.algorithms
        assert HashAlgorithm.SHA1 in options.algorithms
        assert options.enable_progress is False
        assert options.verify_existing is True
        assert options.progress_callback is callback
    
    def test_validation(self):
        """Test options validation."""
        with pytest.raises(ValueError, match="At least one hash algorithm"):
            ProcessOptions(algorithms=[])


class TestBatchOptions:
    """Test BatchOptions dataclass."""
    
    def test_default_options(self):
        """Test default batch options."""
        options = BatchOptions()
        
        assert options.algorithms == [HashAlgorithm.ED2K]
        assert options.max_concurrent == 4
        assert options.continue_on_error is True
        assert options.skip_existing is False
        assert options.progress_callback is None
        assert options.completion_callback is None
    
    def test_validation(self):
        """Test batch options validation."""
        with pytest.raises(ValueError, match="At least one hash algorithm"):
            BatchOptions(algorithms=[])
        
        with pytest.raises(ValueError, match="max_concurrent must be at least 1"):
            BatchOptions(max_concurrent=0)


class TestFileResult:
    """Test FileResult dataclass."""
    
    def test_file_result(self):
        """Test file result properties."""
        result = FileResult(
            file_path=Path("/tmp/test.mkv"),
            file_size=1024 * 1024,
            status=ProcessingStatus.COMPLETED,
            hashes={
                HashAlgorithm.ED2K: "abcd1234",
                HashAlgorithm.MD5: "efgh5678",
            },
            processing_time_ms=1500,
            error_message=None,
        )
        
        assert result.file_path == Path("/tmp/test.mkv")
        assert result.file_size == 1024 * 1024
        assert result.status == ProcessingStatus.COMPLETED
        assert len(result.hashes) == 2
        assert result.processing_time_seconds == 1.5
        assert result.is_successful is True
        assert result.get_hash(HashAlgorithm.ED2K) == "abcd1234"
        assert result.get_hash(HashAlgorithm.SHA1) is None
    
    def test_failed_result(self):
        """Test failed file result."""
        result = FileResult(
            file_path=Path("/tmp/missing.mkv"),
            file_size=0,
            status=ProcessingStatus.FAILED,
            hashes={},
            processing_time_ms=100,
            error_message="File not found",
        )
        
        assert result.is_successful is False
        assert result.error_message == "File not found"


class TestAnimeInfo:
    """Test AnimeInfo dataclass."""
    
    def test_anime_info(self):
        """Test anime info properties."""
        info = AnimeInfo(
            anime_id=12345,
            episode_id=67890,
            title="Test Anime",
            episode_number=1,
            confidence=0.95,
            source="0",
        )
        
        assert info.anime_id == 12345
        assert info.episode_id == 67890
        assert info.title == "Test Anime"
        assert info.episode_number == 1
        assert info.confidence == 0.95
        assert info.source_type == "AniDB"


class TestBatchResult:
    """Test BatchResult dataclass."""
    
    def test_batch_result(self):
        """Test batch result properties."""
        results = [
            FileResult(
                file_path=Path(f"/tmp/file{i}.mkv"),
                file_size=1024,
                status=ProcessingStatus.COMPLETED,
                hashes={HashAlgorithm.ED2K: f"hash{i}"},
                processing_time_ms=100,
            )
            for i in range(3)
        ]
        
        batch_result = BatchResult(
            total_files=3,
            successful_files=3,
            failed_files=0,
            results=results,
            total_time_ms=500,
        )
        
        assert batch_result.total_files == 3
        assert batch_result.successful_files == 3
        assert batch_result.failed_files == 0
        assert len(batch_result.results) == 3
        assert batch_result.total_time_seconds == 0.5
        assert batch_result.success_rate == 100.0
    
    def test_partial_success(self):
        """Test batch result with partial success."""
        batch_result = BatchResult(
            total_files=10,
            successful_files=7,
            failed_files=3,
            results=[],
            total_time_ms=2000,
        )
        
        assert batch_result.success_rate == 70.0


class TestEvent:
    """Test Event dataclass."""
    
    def test_event(self):
        """Test event properties."""
        event = Event(
            type=EventType.FILE_START,
            timestamp=1234567890,
            data={
                "file_path": "/tmp/test.mkv",
                "file_size": 1024,
            },
            context="Processing started",
        )
        
        assert event.type == EventType.FILE_START
        assert event.timestamp == 1234567890
        assert event.data["file_path"] == "/tmp/test.mkv"
        assert event.data["file_size"] == 1024
        assert event.context == "Processing started"
        assert event.event_name == "FILE_START"