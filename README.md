# anidb-rs

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Rust 1.91+](https://img.shields.io/badge/rust-1.91%2B-orange.svg)
![Status: alpha](https://img.shields.io/badge/status-alpha-ff9800.svg)

> Streaming file hashing, AniDB identification, and MyList sync for anime collections.

**Key capabilities:** ED2K/CRC32/MD5/SHA1/TTH hashing with constant memory (< 500 MB for any file size), smart caching, AniDB protocol integration with 0.5 req/s throttling, and multiple output formats.

**Requirements:** Rust 1.91+, AniDB account (for identification/sync)

## Quick Start

```bash
git clone git@github.com:alysson-souza/anidb-rs.git
cd anidb-rs
cargo install --path anidb_cli

# Hash files
anidb hash ~/Anime --recursive --algorithm ed2k

# Identify files
anidb auth login
anidb identify ./episodes --recursive

# Sync MyList
anidb sync all --dry-run
```

## Commands

| Command       | Purpose                             |
| ------------- | ----------------------------------- |
| `hash`        | Calculate file hashes with caching  |
| `identify`    | Query AniDB for file metadata       |
| `sync`        | Manage AniDB MyList queue           |
| `auth`        | Store/retrieve credentials securely |
| `config`      | View/modify settings                |
| `completions` | Generate shell completions          |

**Common flags:** `--recursive`, `--format text/json/csv`, `--include/--exclude`, `--no-cache`, `--debug`, `--verbose`

## Configuration

Configuration is loaded in order: defaults → config file → environment variables → CLI flags.

**Config file locations:**

- Linux: `~/.config/anidb/config.toml` (respects `XDG_CONFIG_HOME`)
- macOS: `~/Library/Application Support/anidb/config.toml`
- Windows: `%APPDATA%\anidb\config.toml`

**Example config.toml:**

```toml
[client]
max_concurrent_files = 4
chunk_size = 65536           # 64 KB
max_memory_usage = 524288000 # 500 MB
client_name = "yourclient"   # Required for AniDB API authentication
client_version = "1"         # Client version for AniDB API

[network]
timeout_seconds = 30
retry_count = 3

[output]
default_format = "text"
color_enabled = true
progress_enabled = true
```

**Environment variables:** Use `ANIDB_SECTION__KEY` pattern:

- `ANIDB_CLIENT__MAX_CONCURRENT_FILES=8` - Concurrent file processing
- `ANIDB_CLIENT__CHUNK_SIZE=65536` - I/O chunk size in bytes
- `ANIDB_CLIENT__MAX_MEMORY_USAGE=524288000` - Memory limit in bytes
- `ANIDB_CLIENT__CLIENT_NAME=yourclient` - AniDB API client name
- `ANIDB_CLIENT__CLIENT_VERSION=1` - AniDB API client version
- `ANIDB_NETWORK__TIMEOUT_SECONDS=60` - Network timeout
- `ANIDB_NETWORK__RETRY_COUNT=5` - Retry attempts
- `ANIDB_OUTPUT__DEFAULT_FORMAT=json` - Output format (text/json/csv)
- `ANIDB_OUTPUT__PROGRESS_ENABLED=false` - Toggle progress bars

## Storage & Caching

**Hash cache:** `{data_dir}/anidb/cache` (file-based, disable with `--no-cache`)  
**Identification cache:** `{data_dir}/anidb/anidb.db` (SQLite)  
**Credentials:** `{config_dir}/anidb-client/credentials/store.enc` (AES-256-GCM encrypted)

Where:

- Linux: `data_dir` = `~/.local/share`, `config_dir` = `~/.config`
- macOS: Both use `~/Library/Application Support`
- Windows: `data_dir` = `%LOCALAPPDATA%`, `config_dir` = `%APPDATA%`

## Development

```bash
# Run checks
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets

# Install pre-commit hooks
cargo install prek
prek install
```
