//! Database schema definitions
//!
//! This module contains all SQL schema definitions for the AniDB client database.

/// Current schema version
pub const CURRENT_SCHEMA_VERSION: i32 = 4;

/// Initial schema creation SQL
pub const SCHEMA_V1: &str = r#"
-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at INTEGER NOT NULL
);

-- File tracking
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    size INTEGER NOT NULL,
    modified_time INTEGER NOT NULL,
    inode INTEGER,
    status TEXT NOT NULL DEFAULT 'pending',
    last_checked INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Hash storage
CREATE TABLE IF NOT EXISTS hashes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    algorithm TEXT NOT NULL,
    hash TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE,
    UNIQUE(file_id, algorithm)
);

-- AniDB identification results
CREATE TABLE IF NOT EXISTS anidb_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    ed2k_hash TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    anime_id INTEGER,
    episode_id INTEGER,
    episode_number TEXT,
    anime_title TEXT,
    episode_title TEXT,
    group_name TEXT,
    group_short TEXT,
    version INTEGER DEFAULT 1,
    censored BOOLEAN DEFAULT FALSE,
    deprecated BOOLEAN DEFAULT FALSE,
    crc32_valid BOOLEAN,
    file_type TEXT,
    resolution TEXT,
    video_codec TEXT,
    audio_codec TEXT,
    source TEXT,
    quality TEXT,
    fetched_at INTEGER NOT NULL,
    expires_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE,
    UNIQUE(ed2k_hash, file_size)
);

-- MyList cache
CREATE TABLE IF NOT EXISTS mylist_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    mylist_id INTEGER NOT NULL,
    state INTEGER NOT NULL DEFAULT 1,
    filestate INTEGER NOT NULL DEFAULT 0,
    viewed BOOLEAN NOT NULL DEFAULT FALSE,
    viewdate INTEGER,
    storage TEXT,
    source TEXT,
    other TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE,
    UNIQUE(file_id)
);

-- Synchronization queue
CREATE TABLE IF NOT EXISTS sync_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    operation TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'pending',
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    error_message TEXT,
    scheduled_at INTEGER NOT NULL,
    last_attempt_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);
CREATE INDEX IF NOT EXISTS idx_files_status ON files(status);
CREATE INDEX IF NOT EXISTS idx_files_modified_time ON files(modified_time);

CREATE INDEX IF NOT EXISTS idx_hashes_file_id ON hashes(file_id);
CREATE INDEX IF NOT EXISTS idx_hashes_algorithm ON hashes(algorithm);

CREATE INDEX IF NOT EXISTS idx_anidb_results_file_id ON anidb_results(file_id);
CREATE INDEX IF NOT EXISTS idx_anidb_results_ed2k_hash ON anidb_results(ed2k_hash);
CREATE INDEX IF NOT EXISTS idx_anidb_results_anime_id ON anidb_results(anime_id);
CREATE INDEX IF NOT EXISTS idx_anidb_results_expires_at ON anidb_results(expires_at);

CREATE INDEX IF NOT EXISTS idx_mylist_cache_file_id ON mylist_cache(file_id);
CREATE INDEX IF NOT EXISTS idx_mylist_cache_mylist_id ON mylist_cache(mylist_id);

CREATE INDEX IF NOT EXISTS idx_sync_queue_file_id ON sync_queue(file_id);
CREATE INDEX IF NOT EXISTS idx_sync_queue_status ON sync_queue(status);
CREATE INDEX IF NOT EXISTS idx_sync_queue_scheduled_at ON sync_queue(scheduled_at);
CREATE INDEX IF NOT EXISTS idx_sync_queue_priority ON sync_queue(priority);
"#;

/// Migration from existing hash_cache table to new schema
pub const MIGRATION_FROM_HASH_CACHE: &str = r#"
-- Migrate existing hash_cache data to new schema
INSERT INTO files (path, size, modified_time, inode, status, last_checked, created_at, updated_at)
SELECT DISTINCT 
    file_path,
    file_size,
    file_modified_time,
    file_inode,
    'processed',
    accessed_at,
    created_at,
    accessed_at
FROM hash_cache
WHERE NOT EXISTS (SELECT 1 FROM files WHERE files.path = hash_cache.file_path);

-- Migrate hash data
INSERT INTO hashes (file_id, algorithm, hash, duration_ms, created_at)
SELECT 
    f.id,
    hc.algorithm,
    hc.hash,
    hc.hash_duration_ms,
    hc.created_at
FROM hash_cache hc
INNER JOIN files f ON f.path = hc.file_path
WHERE NOT EXISTS (
    SELECT 1 FROM hashes h 
    WHERE h.file_id = f.id AND h.algorithm = hc.algorithm
);
"#;

/// Schema v2: Performance profile persistence
pub const SCHEMA_V2: &str = r#"
-- Performance profiles for different storage types
CREATE TABLE IF NOT EXISTS performance_profiles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    storage_type TEXT NOT NULL,
    file_path_pattern TEXT NOT NULL,
    avg_throughput_mbps REAL NOT NULL,
    avg_latency_ms REAL NOT NULL,
    optimal_buffer_size INTEGER NOT NULL,
    sample_count INTEGER NOT NULL,
    last_updated INTEGER NOT NULL,
    UNIQUE(storage_type, file_path_pattern)
);

-- Learned adjustments from adaptive algorithm
CREATE TABLE IF NOT EXISTS learned_adjustments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    storage_type TEXT NOT NULL,
    file_size_category TEXT NOT NULL,
    initial_buffer_size INTEGER NOT NULL,
    final_buffer_size INTEGER NOT NULL,
    performance_improvement REAL NOT NULL,
    chunk_count INTEGER NOT NULL,
    total_bytes_processed INTEGER NOT NULL,
    processing_duration_ms INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

-- Storage-specific performance metrics
CREATE TABLE IF NOT EXISTS storage_performance_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    storage_type TEXT NOT NULL,
    throughput_bps REAL NOT NULL,
    latency_ms REAL NOT NULL,
    memory_efficiency REAL NOT NULL,
    buffer_utilization REAL NOT NULL,
    sample_count INTEGER NOT NULL,
    recorded_at INTEGER NOT NULL
);

-- Indexes for performance profiles
CREATE INDEX IF NOT EXISTS idx_performance_profiles_storage ON performance_profiles(storage_type);
CREATE INDEX IF NOT EXISTS idx_performance_profiles_updated ON performance_profiles(last_updated);

CREATE INDEX IF NOT EXISTS idx_learned_adjustments_storage ON learned_adjustments(storage_type);
CREATE INDEX IF NOT EXISTS idx_learned_adjustments_category ON learned_adjustments(file_size_category);
CREATE INDEX IF NOT EXISTS idx_learned_adjustments_created ON learned_adjustments(created_at);

CREATE INDEX IF NOT EXISTS idx_storage_metrics_storage ON storage_performance_metrics(storage_type);
CREATE INDEX IF NOT EXISTS idx_storage_metrics_recorded ON storage_performance_metrics(recorded_at);
"#;

/// Schema v3: Add mylist_lid to anidb_results for MyList tracking
pub const SCHEMA_V3: &str = r#"
-- Add mylist_lid column to track MyList membership (if it doesn't exist)
-- SQLite doesn't have IF NOT EXISTS for ALTER TABLE, so we handle this in the migration code

-- For now, we'll attempt the ALTER and let the migration handler deal with duplicates
ALTER TABLE anidb_results ADD COLUMN mylist_lid INTEGER;

-- Create index for mylist_lid lookups
CREATE INDEX IF NOT EXISTS idx_anidb_results_mylist_lid ON anidb_results(mylist_lid);
"#;

/// Schema v4: Add deduplication and UNIQUE constraint for sync queue
/// This migration deduplicates existing pending operations and adds a partial UNIQUE index
/// to prevent duplicate pending operations for the same file_id + operation_type
pub const SCHEMA_V4: &str = r#"
-- Step 1: Deduplicate existing sync_queue entries
-- Keep only the oldest entry for each (file_id, operation, status='pending') combination
DELETE FROM sync_queue
WHERE id NOT IN (
    SELECT MIN(id)
    FROM sync_queue
    WHERE status = 'pending'
    GROUP BY file_id, operation
);

-- Step 2: Create a partial unique index to prevent future duplicates
-- Only enforce uniqueness for pending operations (completed/failed can have duplicates for history)
CREATE UNIQUE INDEX IF NOT EXISTS idx_sync_queue_unique_pending 
ON sync_queue(file_id, operation) 
WHERE status = 'pending';
"#;
