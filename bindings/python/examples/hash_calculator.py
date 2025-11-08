#!/usr/bin/env python3
"""
Standalone hash calculator using the AniDB Client library.
"""

import argparse
import sys
from pathlib import Path

from anidb_client import AniDBClient, HashAlgorithm


def calculate_file_hashes(file_path: Path, algorithms: list[HashAlgorithm]) -> dict[HashAlgorithm, str]:
    """Calculate multiple hashes for a file."""
    with AniDBClient() as client:
        hashes = {}
        
        for algo in algorithms:
            try:
                hash_value = client.calculate_hash(file_path, algo)
                hashes[algo] = hash_value
            except Exception as e:
                print(f"Error calculating {algo.name}: {e}", file=sys.stderr)
        
        return hashes


def calculate_string_hash(text: str, algorithm: HashAlgorithm) -> str:
    """Calculate hash for a text string."""
    with AniDBClient() as client:
        return client.calculate_hash_from_bytes(text.encode('utf-8'), algorithm)


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Calculate hashes using the AniDB Client library"
    )
    
    parser.add_argument(
        "input",
        help="File path or text string to hash"
    )
    
    parser.add_argument(
        "-a", "--algorithm",
        choices=["ed2k", "crc32", "md5", "sha1", "tth", "all"],
        default="ed2k",
        help="Hash algorithm to use (default: ed2k)"
    )
    
    parser.add_argument(
        "-s", "--string",
        action="store_true",
        help="Treat input as a string instead of file path"
    )
    
    parser.add_argument(
        "-c", "--compare",
        help="Compare calculated hash with this value"
    )
    
    args = parser.parse_args()
    
    # Determine which algorithms to use
    if args.algorithm == "all":
        algorithms = list(HashAlgorithm)
    else:
        algo_map = {
            "ed2k": HashAlgorithm.ED2K,
            "crc32": HashAlgorithm.CRC32,
            "md5": HashAlgorithm.MD5,
            "sha1": HashAlgorithm.SHA1,
            "tth": HashAlgorithm.TTH,
        }
        algorithms = [algo_map[args.algorithm]]
    
    try:
        if args.string:
            # Hash string
            if len(algorithms) > 1:
                print("String mode only supports single algorithm", file=sys.stderr)
                sys.exit(1)
            
            hash_value = calculate_string_hash(args.input, algorithms[0])
            print(f"{algorithms[0].name}: {hash_value}")
            
            if args.compare:
                if hash_value.lower() == args.compare.lower():
                    print("✅ Hash matches!")
                else:
                    print("❌ Hash does not match!")
                    sys.exit(1)
        else:
            # Hash file
            file_path = Path(args.input)
            
            if not file_path.exists():
                print(f"Error: File '{file_path}' does not exist", file=sys.stderr)
                sys.exit(1)
            
            print(f"File: {file_path}")
            print(f"Size: {file_path.stat().st_size:,} bytes")
            print()
            
            hashes = calculate_file_hashes(file_path, algorithms)
            
            for algo, hash_value in sorted(hashes.items(), key=lambda x: x[0].name):
                print(f"{algo.name:>6}: {hash_value}")
                
                if args.compare and len(algorithms) == 1:
                    if hash_value.lower() == args.compare.lower():
                        print("        ✅ Match!")
                    else:
                        print("        ❌ No match!")
                        sys.exit(1)
            
    except KeyboardInterrupt:
        print("\nCancelled by user", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()