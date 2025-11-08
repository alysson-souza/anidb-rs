"""
Command-line interface for the AniDB Client.
"""

import argparse
import sys
from pathlib import Path

from . import AniDBClient, HashAlgorithm, __version__


def main():
    """Main CLI entry point."""
    parser = argparse.ArgumentParser(
        description="AniDB Client - File hashing and anime identification",
        prog="python -m anidb_client"
    )
    
    parser.add_argument(
        "--version",
        action="version",
        version=f"%(prog)s {__version__} (Library: {AniDBClient.get_version()})"
    )
    
    subparsers = parser.add_subparsers(dest="command", help="Available commands")
    
    # Hash command
    hash_parser = subparsers.add_parser("hash", help="Calculate file hashes")
    hash_parser.add_argument("file", type=Path, help="File to hash")
    hash_parser.add_argument(
        "-a", "--algorithm",
        choices=["ed2k", "crc32", "md5", "sha1", "tth", "all"],
        default="ed2k",
        help="Hash algorithm (default: ed2k)"
    )
    
    # Info command
    info_parser = subparsers.add_parser("info", help="Show library information")
    
    args = parser.parse_args()
    
    if args.command == "hash":
        # Hash file
        if not args.file.exists():
            print(f"Error: File '{args.file}' not found", file=sys.stderr)
            sys.exit(1)
        
        algorithms = []
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
            with AniDBClient() as client:
                for algo in algorithms:
                    hash_value = client.calculate_hash(args.file, algo)
                    print(f"{algo.name}: {hash_value}")
        except Exception as e:
            print(f"Error: {e}", file=sys.stderr)
            sys.exit(1)
    
    elif args.command == "info":
        # Show info
        print(f"AniDB Client Python v{__version__}")
        print(f"Core Library v{AniDBClient.get_version()}")
        print(f"ABI Version: {AniDBClient.get_abi_version()}")
    
    else:
        # No command specified
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()