#!/usr/bin/env python3
"""
Basic usage example for the AniDB Client Python library.
"""

import sys
from pathlib import Path

from anidb_client import AniDBClient, HashAlgorithm, ProcessOptions


def main():
    """Demonstrate basic file processing."""
    if len(sys.argv) < 2:
        print("Usage: python basic_usage.py <file_path>")
        sys.exit(1)
    
    file_path = Path(sys.argv[1])
    
    if not file_path.exists():
        print(f"Error: File '{file_path}' does not exist")
        sys.exit(1)
    
    # Create client with context manager for automatic cleanup
    with AniDBClient() as client:
        print(f"Processing file: {file_path}")
        print(f"Library version: {client.get_version()}")
        print("-" * 50)
        
        # Process file with default options (ED2K hash)
        try:
            result = client.process_file(file_path)
            
            print(f"File: {result.file_path.name}")
            print(f"Size: {result.file_size:,} bytes")
            print(f"Status: {result.status.name}")
            print(f"Processing time: {result.processing_time_seconds:.2f} seconds")
            print("\nHashes:")
            
            for algorithm, hash_value in result.hashes.items():
                print(f"  {algorithm.name}: {hash_value}")
            
        except Exception as e:
            print(f"Error processing file: {e}")
            sys.exit(1)


if __name__ == "__main__":
    main()