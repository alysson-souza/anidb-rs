#!/usr/bin/env python3
"""
Advanced usage example for the AniDB Client Python library.
"""

import sys
from pathlib import Path

from anidb_client import (
    AniDBClient,
    ClientConfig,
    HashAlgorithm,
    ProcessOptions,
    EventType,
)


def progress_callback(percentage: float, bytes_processed: int, total_bytes: int):
    """Display progress bar."""
    bar_width = 40
    filled = int(bar_width * percentage / 100)
    bar = "█" * filled + "░" * (bar_width - filled)
    
    print(f"\r[{bar}] {percentage:.1f}% ({bytes_processed:,}/{total_bytes:,} bytes)", end="", flush=True)


def event_callback(event):
    """Handle events from the library."""
    if event.type == EventType.FILE_START:
        print(f"\n🎬 Started processing: {event.data.get('file_path', 'Unknown')}")
    elif event.type == EventType.FILE_COMPLETE:
        print(f"\n✅ Completed: {event.data.get('file_path', 'Unknown')}")
    elif event.type == EventType.HASH_START:
        print(f"\n🔐 Calculating {event.data.get('algorithm', 'Unknown')} hash...")
    elif event.type == EventType.CACHE_HIT:
        print(f"\n💾 Cache hit for {event.data.get('algorithm', 'Unknown')}")
    elif event.type == EventType.MEMORY_WARNING:
        current = event.data.get('current_usage', 0) / (1024 * 1024)
        max_usage = event.data.get('max_usage', 0) / (1024 * 1024)
        print(f"\n⚠️  Memory warning: {current:.1f}MB / {max_usage:.1f}MB")


def main():
    """Demonstrate advanced features."""
    if len(sys.argv) < 2:
        print("Usage: python advanced_usage.py <file_path> [file_path2 ...]")
        sys.exit(1)
    
    file_paths = [Path(arg) for arg in sys.argv[1:]]
    
    # Validate all files exist
    for file_path in file_paths:
        if not file_path.exists():
            print(f"Error: File '{file_path}' does not exist")
            sys.exit(1)
    
    # Custom configuration
    config = ClientConfig(
        cache_dir=Path.home() / ".anidb_cache",
        max_concurrent_files=4,
        chunk_size=128 * 1024,  # 128KB chunks
        enable_debug_logging=False,
    )
    
    # Create client with custom config
    with AniDBClient(config) as client:
        print(f"AniDB Client v{client.get_version()}")
        print(f"Processing {len(file_paths)} file(s)")
        print("=" * 60)
        
        # Connect event system
        client.connect_events(event_callback)
        
        # Process each file with multiple algorithms and progress
        for file_path in file_paths:
            print(f"\n📁 File: {file_path.name}")
            print(f"   Size: {file_path.stat().st_size:,} bytes")
            
            # Check if already cached
            if client.is_file_cached(file_path, HashAlgorithm.ED2K):
                print("   ℹ️  ED2K hash already cached")
            
            # Process with multiple algorithms
            options = ProcessOptions(
                algorithms=[
                    HashAlgorithm.ED2K,
                    HashAlgorithm.CRC32,
                    HashAlgorithm.MD5,
                    HashAlgorithm.SHA1,
                ],
                enable_progress=True,
                progress_callback=progress_callback,
            )
            
            try:
                result = client.process_file(file_path, options)
                print()  # New line after progress bar
                
                if result.is_successful:
                    print("\n   📊 Results:")
                    for algo, hash_value in sorted(result.hashes.items(), key=lambda x: x[0].name):
                        print(f"      {algo.name:>6}: {hash_value}")
                    
                    print(f"\n   ⏱️  Time: {result.processing_time_seconds:.2f}s")
                    
                    # Try to identify the file if we have ED2K hash
                    if HashAlgorithm.ED2K in result.hashes:
                        print("\n   🔍 Attempting anime identification...")
                        anime_info = client.identify_file(
                            result.hashes[HashAlgorithm.ED2K],
                            result.file_size
                        )
                        
                        if anime_info:
                            print(f"      Title: {anime_info.title}")
                            print(f"      Episode: {anime_info.episode_number}")
                            print(f"      Confidence: {anime_info.confidence:.0%}")
                            print(f"      Source: {anime_info.source_type}")
                        else:
                            print("      Not found in AniDB")
                else:
                    print(f"\n   ❌ Failed: {result.error_message}")
                    
            except Exception as e:
                print(f"\n   ❌ Error: {e}")
        
        # Show cache statistics at the end
        print("\n" + "=" * 60)
        print("📊 Cache Statistics:")
        try:
            entries, size_bytes = client.get_cache_stats()
            size_mb = size_bytes / (1024 * 1024)
            print(f"   Entries: {entries:,}")
            print(f"   Size: {size_mb:.2f} MB")
        except:
            print("   Cache statistics not available")


if __name__ == "__main__":
    main()