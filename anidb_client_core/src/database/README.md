# Database Module

This module provides SQLite-based data management for the AniDB client, implementing the Sprint 3 Data Management
features.

## Structure

- `mod.rs` - Main database interface with connection pooling
- `schema.rs` - Database schema definitions (version 1)
- `migrations.rs` - Migration system for schema updates
- `models.rs` - Data structures mapping to database tables
- `repositories/` - Repository pattern implementations for each entity

## Features

### Database Tables

1. **files** - Tracks file metadata and processing status
2. **hashes** - Stores calculated hash values (ED2K, CRC32, MD5, SHA1, TTH)
3. **anidb_results** - Caches AniDB identification results
4. **mylist_cache** - Stores MyList entries for offline access
5. **sync_queue** - Manages pending synchronization operations
6. **schema_version** - Tracks database migrations

### Key Capabilities

- **Connection Pooling**: 5-10 connections with SQLite WAL mode
- **Migration System**: Automatic schema updates with version tracking
- **Repository Pattern**: Clean data access layer for each entity
- **Transaction Support**: Atomic operations for data integrity
- **Performance**: Optimized indexes and prepared statement caching

### Repository Operations

Each repository provides:

- CRUD operations (Create, Read, Update, Delete)
- Specialized queries (find by status, find expired, etc.)
- Batch operations for efficiency
- Statistics and reporting

### Performance Characteristics

- Hash lookup: <1ms
- Batch insert: >1000 records/second
- Memory efficient: Reuses existing buffer pool
- Concurrent access: WAL mode enables reader/writer concurrency

## Usage Example

```rust
use anidb_client_core::database::{Database, FileRepository};
use anidb_client_core::database::models::File;

// Create database
let db = Database::new(Path::new("anidb.db")).await?;

// Get repository
let file_repo = FileRepository::new(db.pool().clone());

// Create file entry
let file_id = file_repo.create(&file).await?;

// Find pending files
let pending = file_repo.find_by_status(FileStatus::Pending, 100).await?;
```

## Migration from Existing Cache

The migration system automatically detects and migrates data from the existing `hash_cache` table to the new schema,
preserving all hash data and access statistics.

## Testing

Test coverage includes:

- Unit tests for each repository
- Migration testing including rollback scenarios
- Performance benchmarks
- Concurrent access testing

Run tests with:

```bash
cargo test -p anidb_client_core database
```