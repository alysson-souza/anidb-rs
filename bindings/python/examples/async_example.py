#!/usr/bin/env python3
"""
Async example for the AniDB Client Python library.
"""

import asyncio
import sys
from pathlib import Path

from anidb_client import AniDBClient, BatchOptions, HashAlgorithm


async def process_files_async(file_paths: list[Path]):
    """Process multiple files asynchronously."""
    # Create client
    with AniDBClient() as client:
        print(f"Processing {len(file_paths)} files asynchronously...")
        print("-" * 50)
        
        # Process files concurrently
        tasks = []
        for file_path in file_paths:
            task = client.process_file_async(file_path)
            tasks.append(task)
        
        # Wait for all tasks to complete
        results = await asyncio.gather(*tasks, return_exceptions=True)
        
        # Display results
        for file_path, result in zip(file_paths, results):
            print(f"\n📄 {file_path.name}")
            
            if isinstance(result, Exception):
                print(f"   ❌ Error: {result}")
            else:
                print(f"   ✅ Status: {result.status.name}")
                print(f"   📏 Size: {result.file_size:,} bytes")
                print(f"   ⏱️  Time: {result.processing_time_seconds:.2f}s")
                
                if result.hashes:
                    ed2k = result.get_hash(HashAlgorithm.ED2K)
                    if ed2k:
                        print(f"   🔑 ED2K: {ed2k}")


async def batch_processing_example(file_paths: list[Path]):
    """Demonstrate batch processing."""
    with AniDBClient() as client:
        print(f"\n\nBatch processing {len(file_paths)} files...")
        print("-" * 50)
        
        # Configure batch options
        options = BatchOptions(
            algorithms=[HashAlgorithm.ED2K, HashAlgorithm.CRC32],
            max_concurrent=3,
            continue_on_error=True,
            skip_existing=False,
        )
        
        # Process batch
        batch_result = await client.process_batch_async(file_paths, options)
        
        # Summary
        print(f"\n📊 Batch Summary:")
        print(f"   Total files: {batch_result.total_files}")
        print(f"   Successful: {batch_result.successful_files}")
        print(f"   Failed: {batch_result.failed_files}")
        print(f"   Success rate: {batch_result.success_rate:.1f}%")
        print(f"   Total time: {batch_result.total_time_seconds:.2f}s")
        
        # Show failed files if any
        if batch_result.failed_files > 0:
            print("\n❌ Failed files:")
            for result in batch_result.results:
                if not result.is_successful:
                    print(f"   - {result.file_path.name}: {result.error_message}")


async def main():
    """Main async entry point."""
    if len(sys.argv) < 2:
        print("Usage: python async_example.py <file_path> [file_path2 ...]")
        sys.exit(1)
    
    file_paths = []
    for arg in sys.argv[1:]:
        path = Path(arg)
        if path.is_file():
            file_paths.append(path)
        elif path.is_dir():
            # Add all video files in directory
            for ext in ["*.mkv", "*.mp4", "*.avi"]:
                file_paths.extend(path.glob(ext))
    
    if not file_paths:
        print("No valid files found")
        sys.exit(1)
    
    print(f"Found {len(file_paths)} file(s) to process")
    
    # Run async processing
    await process_files_async(file_paths[:5])  # Limit to 5 for demo
    
    # Run batch processing
    if len(file_paths) > 1:
        await batch_processing_example(file_paths)


if __name__ == "__main__":
    asyncio.run(main())