#![cfg(feature = "database")]

//! Database integration tests
//!

//! This module tests database operations including connection pooling,
//! transactions, migrations, and repository operations.

use anidb_client_core::database::Database;
use serial_test::serial;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::task::JoinSet;

mod database_tests {
    use super::*;

    async fn create_test_database() -> (Database, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(&db_path).await.unwrap();
        (db, temp_dir)
    }

    #[tokio::test]
    async fn test_database_creation_and_migrations() {
        let (db, _temp_dir) = create_test_database().await;

        // Verify database is accessible
        let stats = db.stats().await.unwrap();
        assert_eq!(stats.file_count, 0);
        assert_eq!(stats.hash_count, 0);
        assert_eq!(stats.anidb_result_count, 0);
        assert_eq!(stats.mylist_count, 0);
        assert_eq!(stats.sync_queue_count, 0);

        // Verify tables exist by attempting queries
        let pool = db.pool();
        sqlx::query("SELECT COUNT(*) FROM files")
            .fetch_one(pool)
            .await
            .expect("files table should exist");

        sqlx::query("SELECT COUNT(*) FROM hashes")
            .fetch_one(pool)
            .await
            .expect("hashes table should exist");

        sqlx::query("SELECT COUNT(*) FROM anidb_results")
            .fetch_one(pool)
            .await
            .expect("anidb_results table should exist");

        sqlx::query("SELECT COUNT(*) FROM mylist_cache")
            .fetch_one(pool)
            .await
            .expect("mylist_cache table should exist");

        sqlx::query("SELECT COUNT(*) FROM sync_queue")
            .fetch_one(pool)
            .await
            .expect("sync_queue table should exist");
    }

    #[tokio::test]
    async fn test_connection_pool_limits() {
        let (db, _temp_dir) = create_test_database().await;
        let pool = Arc::new(db.pool().clone());

        // Spawn more tasks than max connections (10)
        let mut tasks = JoinSet::new();
        for i in 0..20 {
            let pool_clone = pool.clone();
            tasks.spawn(async move {
                // Hold connection for a bit
                let mut conn = pool_clone.acquire().await.unwrap();
                sqlx::query("SELECT ?")
                    .bind(i)
                    .execute(&mut *conn)
                    .await
                    .unwrap();
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                i
            });
        }

        // All tasks should complete without deadlock
        let mut results = Vec::new();
        while let Some(result) = tasks.join_next().await {
            results.push(result.unwrap());
        }
        assert_eq!(results.len(), 20);
    }

    #[tokio::test]
    #[serial]
    async fn test_concurrent_read_write() {
        let (db, _temp_dir) = create_test_database().await;
        let pool = Arc::new(db.pool().clone());

        // Setup test data
        sqlx::query("INSERT INTO files (path, size, modified_time, status, last_checked, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind("/test/file.txt")
            .bind(1024i64)
            .bind(0i64)
            .bind("pending")
            .bind(0i64)
            .bind(0i64)
            .bind(0i64)
            .execute(pool.as_ref())
            .await
            .unwrap();

        let mut tasks = JoinSet::new();

        // Readers
        for _ in 0..10 {
            let pool_clone = pool.clone();
            tasks.spawn(async move {
                for _ in 0..100 {
                    sqlx::query("SELECT * FROM files WHERE path = ?")
                        .bind("/test/file.txt")
                        .fetch_optional(pool_clone.as_ref())
                        .await
                        .unwrap();
                }
            });
        }

        // Writers
        for i in 0..5 {
            let pool_clone = pool.clone();
            tasks.spawn(async move {
                for j in 0..20 {
                    let path = format!("/test/file_{i}_{j}.txt");
                    sqlx::query("INSERT INTO files (path, size, modified_time, status, last_checked, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
                        .bind(&path)
                        .bind(1024i64)
                        .bind(0i64)
                        .bind("pending")
                        .bind(0i64)
                        .bind(0i64)
                        .bind(0i64)
                        .execute(pool_clone.as_ref())
                        .await
                        .unwrap();
                }
            });
        }

        // All operations should complete
        while let Some(result) = tasks.join_next().await {
            result.unwrap();
        }

        // Verify final count
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files")
            .fetch_one(pool.as_ref())
            .await
            .unwrap();
        assert_eq!(count, 101); // 1 initial + 100 from writers
    }

    #[tokio::test]
    async fn test_transaction_isolation() {
        let (db, _temp_dir) = create_test_database().await;
        let pool = db.pool();

        // Start a transaction
        let mut tx1 = pool.begin().await.unwrap();

        // Insert in transaction
        sqlx::query("INSERT INTO files (path, size, modified_time, status, last_checked, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind("/tx/file.txt")
            .bind(1024i64)
            .bind(0i64)
            .bind("pending")
            .bind(0i64)
            .bind(0i64)
            .bind(0i64)
            .execute(&mut *tx1)
            .await
            .unwrap();

        // Verify not visible outside transaction
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE path = ?")
            .bind("/tx/file.txt")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(count, 0);

        // Commit
        tx1.commit().await.unwrap();

        // Now visible
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE path = ?")
            .bind("/tx/file.txt")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_transaction_rollback() {
        let (db, _temp_dir) = create_test_database().await;
        let pool = db.pool();

        // Start a transaction
        let mut tx = pool.begin().await.unwrap();

        // Insert multiple records
        for i in 0..5 {
            let path = format!("/rollback/file_{i}.txt");
            sqlx::query("INSERT INTO files (path, size, modified_time, status, last_checked, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
                .bind(&path)
                .bind(1024i64)
                .bind(0i64)
                .bind("pending")
                .bind(0i64)
                .bind(0i64)
                .bind(0i64)
                .execute(&mut *tx)
                .await
                .unwrap();
        }

        // Rollback transaction
        tx.rollback().await.unwrap();

        // Verify nothing was persisted
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE path LIKE '/rollback/%'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    #[serial]
    async fn test_connection_pool_recovery() {
        let (db, _temp_dir) = create_test_database().await;
        let pool = Arc::new(db.pool().clone());

        // Acquire all connections
        let mut connections = Vec::new();
        for _ in 0..10 {
            connections.push(pool.acquire().await.unwrap());
        }

        // Try to acquire one more (should wait)
        let pool_clone = pool.clone();
        let acquire_task = tokio::spawn(async move {
            let start = tokio::time::Instant::now();
            let _conn = pool_clone.acquire().await.unwrap();
            start.elapsed()
        });

        // Wait a bit then release connections
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        connections.clear(); // Drop all connections

        // The waiting task should now complete
        let elapsed = acquire_task.await.unwrap();
        assert!(elapsed.as_millis() >= 100);
        assert!(elapsed.as_millis() < 1000); // Should not timeout
    }

    #[tokio::test]
    async fn test_database_stats_accuracy() {
        let (db, _temp_dir) = create_test_database().await;
        let pool = db.pool();

        // Insert test data
        for i in 0..5 {
            sqlx::query("INSERT INTO files (path, size, modified_time, status, last_checked, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
                .bind(format!("/stats/file_{i}.txt"))
                .bind(1024i64)
                .bind(0i64)
                .bind("pending")
                .bind(0i64)
                .bind(0i64)
                .bind(0i64)
                .execute(pool)
                .await
                .unwrap();
        }

        // Insert hashes
        for i in 1..=3 {
            sqlx::query("INSERT INTO hashes (file_id, algorithm, hash, duration_ms, created_at) VALUES (?, ?, ?, ?, ?)")
                .bind(i)
                .bind("ed2k")
                .bind(format!("hash_{i}"))
                .bind(100i64)
                .bind(0i64)
                .execute(pool)
                .await
                .unwrap();
        }

        // Verify stats
        let stats = db.stats().await.unwrap();
        assert_eq!(stats.file_count, 5);
        assert_eq!(stats.hash_count, 3);
    }
}

mod repository_tests {
    use super::*;
    use anidb_client_core::database::{
        File, FileRepository, FileStatus, Hash, HashRepository, Repository, models::time_utils,
    };
    use std::path::Path;

    async fn setup_test_env() -> (Database, FileRepository, HashRepository, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(&db_path).await.unwrap();

        let file_repo = FileRepository::new(db.pool().clone());
        let hash_repo = HashRepository::new(db.pool().clone());

        (db, file_repo, hash_repo, temp_dir)
    }

    fn create_test_file(path: &str) -> File {
        File {
            id: 0,
            path: path.to_string(),
            size: 1024,
            modified_time: time_utils::now_millis(),
            inode: None,
            status: FileStatus::Pending,
            last_checked: time_utils::now_millis(),
            created_at: time_utils::now_millis(),
            updated_at: time_utils::now_millis(),
        }
    }

    mod file_repository_tests {
        use super::*;

        #[tokio::test]
        async fn test_find_by_path_performance() {
            let (_db, file_repo, _hash_repo, _temp_dir) = setup_test_env().await;

            // Insert test file
            let file = create_test_file("/test/perf/lookup.txt");
            file_repo.create(&file).await.unwrap();

            // Warm up with more iterations to ensure SQLite caches are hot
            for _ in 0..50 {
                file_repo
                    .find_by_path(Path::new("/test/perf/lookup.txt"))
                    .await
                    .unwrap();
            }

            // Give the system a moment to stabilize after warmup
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

            // Measure lookup time
            let mut durations = Vec::new();
            for _ in 0..100 {
                let start = std::time::Instant::now();
                let result = file_repo
                    .find_by_path(Path::new("/test/perf/lookup.txt"))
                    .await
                    .unwrap();
                let duration = start.elapsed();

                assert!(result.is_some());
                durations.push(duration);
            }

            // Calculate p99
            durations.sort();
            let p99_index = (durations.len() as f64 * 0.99) as usize;
            let p99_duration = durations[p99_index];

            // Performance requirement: 5ms for P99 (more realistic for cross-platform testing)
            // This accounts for:
            // - Different SQLite implementations across platforms
            // - Variable system load during CI/CD
            // - File system cache state variations
            // - Debug vs Release build differences
            let max_p99_micros = if cfg!(debug_assertions) { 10000 } else { 5000 };

            assert!(
                p99_duration.as_micros() < max_p99_micros,
                "P99 lookup time {p99_duration:?} exceeds limit of {}ms",
                max_p99_micros / 1000
            );

            // Also verify median performance is reasonable
            let median_duration = durations[durations.len() / 2];
            let max_median_micros = if cfg!(debug_assertions) { 5000 } else { 2000 };

            assert!(
                median_duration.as_micros() < max_median_micros,
                "Median lookup time {median_duration:?} exceeds limit of {}ms",
                max_median_micros / 1000
            );
        }

        #[tokio::test]
        async fn test_update_metadata() {
            let (_db, file_repo, _hash_repo, _temp_dir) = setup_test_env().await;

            // Create file
            let file = create_test_file("/test/metadata.txt");
            let id = file_repo.create(&file).await.unwrap();

            // Update metadata
            let new_size = 2048;
            let new_modified = time_utils::now_millis() + 1000;
            let new_inode = Some(12345);

            file_repo
                .update_metadata(id, new_size, new_modified, new_inode)
                .await
                .unwrap();

            // Verify update
            let updated = file_repo.find_by_id(id).await.unwrap().unwrap();
            assert_eq!(updated.size, new_size);
            assert_eq!(updated.modified_time, new_modified);
            assert_eq!(updated.inode, new_inode);
            assert!(updated.last_checked >= file.last_checked);
        }

        #[tokio::test]
        async fn test_mark_deleted() {
            let (_db, file_repo, _hash_repo, _temp_dir) = setup_test_env().await;

            // Create multiple files
            let paths = vec!["/test/del1.txt", "/test/del2.txt", "/test/keep.txt"];

            for path in &paths {
                let file = create_test_file(path);
                file_repo.create(&file).await.unwrap();
            }

            // Mark some as deleted
            let to_delete = vec![paths[0].to_string(), paths[1].to_string()];
            let affected = file_repo.mark_deleted(&to_delete).await.unwrap();
            assert_eq!(affected, 2);

            // Verify status
            for (i, path) in paths.iter().enumerate() {
                let file = file_repo
                    .find_by_path(Path::new(path))
                    .await
                    .unwrap()
                    .unwrap();

                if i < 2 {
                    assert_eq!(file.status, FileStatus::Deleted);
                } else {
                    assert_eq!(file.status, FileStatus::Pending);
                }
            }
        }

        #[tokio::test]
        async fn test_get_files_to_check() {
            let (_db, file_repo, _hash_repo, _temp_dir) = setup_test_env().await;

            let now = time_utils::now_millis();

            // Create files with different last_checked times
            for i in 0..5 {
                let mut file = create_test_file(&format!("/test/check_{i}.txt"));
                file.last_checked = now - (i as i64 * 60000); // i minutes ago
                file_repo.create(&file).await.unwrap();
            }

            // Create a deleted file (should be excluded)
            let mut deleted = create_test_file("/test/deleted.txt");
            deleted.status = FileStatus::Deleted;
            deleted.last_checked = now - 300000; // 5 minutes ago
            file_repo.create(&deleted).await.unwrap();

            // Get files older than 2 minutes
            let files = file_repo.get_files_to_check(10, 120000).await.unwrap();

            // Should get 3 files (2, 3, 4 minutes old)
            assert_eq!(files.len(), 3);

            // Should be ordered by last_checked ASC (oldest first)
            assert!(files[0].last_checked < files[1].last_checked);
            assert!(files[1].last_checked < files[2].last_checked);

            // Should not include deleted file
            assert!(!files.iter().any(|f| f.path.contains("deleted")));
        }
    }

    mod hash_repository_tests {
        use super::*;
        use anidb_client_core::hashing::HashAlgorithm;

        #[tokio::test]
        async fn test_hash_crud() {
            let (_db, file_repo, hash_repo, _temp_dir) = setup_test_env().await;

            // Create a file first
            let file = create_test_file("/test/hash_crud.txt");
            let file_id = file_repo.create(&file).await.unwrap();

            // Create hash
            let hash = Hash {
                id: 0,
                file_id,
                algorithm: HashAlgorithm::ED2K.to_string(),
                hash: "abcdef123456".to_string(),
                duration_ms: 150,
                created_at: time_utils::now_millis(),
            };

            let hash_id = hash_repo.create(&hash).await.unwrap();
            assert!(hash_id > 0);

            // Find by ID
            let found = hash_repo.find_by_id(hash_id).await.unwrap().unwrap();
            assert_eq!(found.file_id, file_id);
            assert_eq!(found.hash, "abcdef123456");

            // Find by file ID
            let hashes = hash_repo.find_by_file_id(file_id).await.unwrap();
            assert_eq!(hashes.len(), 1);
            assert_eq!(hashes[0].algorithm, HashAlgorithm::ED2K.to_string());

            // Delete
            hash_repo.delete(hash_id).await.unwrap();
            assert!(hash_repo.find_by_id(hash_id).await.unwrap().is_none());
        }

        #[tokio::test]
        async fn test_multiple_algorithms_per_file() {
            let (_db, file_repo, hash_repo, _temp_dir) = setup_test_env().await;

            // Create a file
            let file = create_test_file("/test/multi_hash.txt");
            let file_id = file_repo.create(&file).await.unwrap();

            // Create multiple hashes for the same file
            let algorithms = vec![
                (HashAlgorithm::ED2K, "ed2k_hash", 150),
                (HashAlgorithm::CRC32, "crc32_hash", 50),
            ];

            for (algo, hash_str, duration) in algorithms {
                let hash = Hash {
                    id: 0,
                    file_id,
                    algorithm: algo.to_string(),
                    hash: hash_str.to_string(),
                    duration_ms: duration,
                    created_at: time_utils::now_millis(),
                };
                hash_repo.create(&hash).await.unwrap();
            }

            // Find all hashes for the file
            let hashes = hash_repo.find_by_file_id(file_id).await.unwrap();
            assert_eq!(hashes.len(), 2);

            // Verify all algorithms present
            let found_algos: Vec<_> = hashes.iter().map(|h| h.algorithm.clone()).collect();
            assert!(found_algos.contains(&HashAlgorithm::ED2K.to_string()));
            assert!(found_algos.contains(&HashAlgorithm::CRC32.to_string()));
        }
    }

    mod transaction_tests {
        use super::*;

        #[tokio::test]
        async fn test_transaction_commit_multiple_operations() {
            let (db, file_repo, hash_repo, _temp_dir) = setup_test_env().await;

            let mut tx = db.pool().begin().await.unwrap();

            // Insert file in transaction
            let file_result = sqlx::query(
                r#"
                INSERT INTO files (path, size, modified_time, status, last_checked, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                "#
            )
            .bind("/tx/test.txt")
            .bind(1024i64)
            .bind(0i64)
            .bind("pending")
            .bind(0i64)
            .bind(0i64)
            .bind(0i64)
            .execute(&mut *tx)
            .await
            .unwrap();

            let file_id = file_result.last_insert_rowid();

            // Insert hash in same transaction
            sqlx::query(
                r#"
                INSERT INTO hashes (file_id, algorithm, hash, duration_ms, created_at)
                VALUES (?, ?, ?, ?, ?)
                "#,
            )
            .bind(file_id)
            .bind("ed2k")
            .bind("test_hash")
            .bind(100i64)
            .bind(0i64)
            .execute(&mut *tx)
            .await
            .unwrap();

            // Commit transaction
            tx.commit().await.unwrap();

            // Verify both operations persisted
            let file = file_repo
                .find_by_path(Path::new("/tx/test.txt"))
                .await
                .unwrap()
                .unwrap();

            let hashes = hash_repo.find_by_file_id(file.id).await.unwrap();
            assert_eq!(hashes.len(), 1);
        }

        #[tokio::test]
        async fn test_transaction_rollback_on_error() {
            let (db, file_repo, _hash_repo, _temp_dir) = setup_test_env().await;

            // Insert a file first
            let file = File {
                id: 0,
                path: "/unique/path.txt".to_string(),
                size: 1024,
                modified_time: time_utils::now_millis(),
                inode: None,
                status: FileStatus::Pending,
                last_checked: time_utils::now_millis(),
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            };
            file_repo.create(&file).await.unwrap();

            // Try transaction that will fail
            let mut tx = db.pool().begin().await.unwrap();

            // This should succeed
            sqlx::query(
                r#"
                INSERT INTO files (path, size, modified_time, status, last_checked, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                "#
            )
            .bind("/another/file.txt")
            .bind(2048i64)
            .bind(0i64)
            .bind("pending")
            .bind(0i64)
            .bind(0i64)
            .bind(0i64)
            .execute(&mut *tx)
            .await
            .unwrap();

            // This should fail (duplicate path)
            let result = sqlx::query(
                r#"
                INSERT INTO files (path, size, modified_time, status, last_checked, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                "#
            )
            .bind("/unique/path.txt") // Duplicate!
            .bind(2048i64)
            .bind(0i64)
            .bind("pending")
            .bind(0i64)
            .bind(0i64)
            .bind(0i64)
            .execute(&mut *tx)
            .await;

            assert!(result.is_err());

            // Rollback
            tx.rollback().await.unwrap();

            // Verify first insert was rolled back
            let count = file_repo.count().await.unwrap();
            assert_eq!(count, 1); // Only the original file
        }
    }
}

mod batch_operation_performance_tests {
    use super::*;
    use anidb_client_core::database::{
        File, FileRepository, FileStatus, Hash, HashRepository, models::time_utils,
    };
    use anidb_client_core::hashing::HashAlgorithm;

    async fn setup_perf_test_env() -> (Database, FileRepository, HashRepository, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("perf_test.db");
        let db = Database::new(&db_path).await.unwrap();

        let file_repo = FileRepository::new(db.pool().clone());
        let hash_repo = HashRepository::new(db.pool().clone());

        (db, file_repo, hash_repo, temp_dir)
    }

    #[tokio::test]
    async fn test_batch_insert_performance() {
        let (_db, file_repo, _hash_repo, _temp_dir) = setup_perf_test_env().await;

        // Generate 1000 test files
        let mut files = Vec::new();
        for i in 0..1000 {
            files.push(File {
                id: 0,
                path: format!("/test/perf/file_{i}.txt"),
                size: 1024 * i,
                modified_time: time_utils::now_millis(),
                inode: Some(i),
                status: FileStatus::Pending,
                last_checked: time_utils::now_millis(),
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            });
        }

        // Measure batch insert time
        let start = std::time::Instant::now();
        let ids = file_repo.batch_insert(&files).await.unwrap();
        let duration = start.elapsed();

        assert_eq!(ids.len(), 1000);

        // Should be faster than 1 second for 1000 records
        assert!(
            duration.as_secs() < 1,
            "Batch insert of 1000 records took {duration:?}, expected < 1s"
        );

        // Calculate throughput
        let throughput = 1000.0 / duration.as_secs_f64();
        println!("Batch insert throughput: {throughput:.0} records/second");

        // Should exceed 1000 records/second
        assert!(
            throughput > 1000.0,
            "Throughput {throughput:.0} records/s is below target of 1000 records/s"
        );
    }

    #[tokio::test]
    async fn test_lookup_performance_with_data() {
        let (_db, file_repo, hash_repo, _temp_dir) = setup_perf_test_env().await;

        // Insert 10,000 files for realistic performance testing
        for batch in 0..10 {
            let mut files = Vec::new();
            for i in 0..1000 {
                let idx = batch * 1000 + i;
                files.push(File {
                    id: 0,
                    path: format!("/test/lookup/file_{idx}.txt"),
                    size: 1024 * idx,
                    modified_time: time_utils::now_millis() - idx * 1000,
                    inode: Some(idx),
                    status: if idx % 3 == 0 {
                        FileStatus::Processed
                    } else {
                        FileStatus::Pending
                    },
                    last_checked: time_utils::now_millis() - idx * 2000,
                    created_at: time_utils::now_millis() - idx * 3000,
                    updated_at: time_utils::now_millis() - idx * 1000,
                });
            }
            file_repo.batch_insert(&files).await.unwrap();
        }

        // Add hashes for half the files
        let mut hashes = Vec::new();
        for i in 0..5000 {
            hashes.push(Hash {
                id: 0,
                file_id: (i + 1) as i64,
                algorithm: HashAlgorithm::ED2K.to_string(),
                hash: format!("{i:032x}"),
                duration_ms: 100 + (i % 200) as i64,
                created_at: time_utils::now_millis(),
            });

            // Batch insert every 1000 hashes
            if hashes.len() == 1000 {
                hash_repo.batch_insert(&hashes).await.unwrap();
                hashes.clear();
            }
        }

        // Test lookup performance
        let test_path = "/test/lookup/file_5000.txt";

        // Warm up
        for _ in 0..10 {
            file_repo.find_by_path(Path::new(test_path)).await.unwrap();
        }

        // Measure lookup times
        let mut durations = Vec::new();
        for _ in 0..100 {
            let start = std::time::Instant::now();
            let result = file_repo.find_by_path(Path::new(test_path)).await.unwrap();
            let duration = start.elapsed();

            assert!(result.is_some());
            durations.push(duration);
        }

        // Calculate statistics
        durations.sort();
        let p50 = durations[50];
        let p95 = durations[95];
        let p99 = durations[99];

        println!("Lookup performance - p50: {p50:?}, p95: {p95:?}, p99: {p99:?}");

        // All percentiles should be under 1ms
        assert!(p50.as_micros() < 1000, "p50 too slow: {p50:?}");
        assert!(p95.as_micros() < 1000, "p95 too slow: {p95:?}");
        assert!(p99.as_micros() < 1000, "p99 too slow: {p99:?}");
    }

    #[tokio::test]
    async fn test_find_files_without_hashes_performance() {
        let (_db, file_repo, hash_repo, _temp_dir) = setup_perf_test_env().await;

        // Insert files with mixed hash status
        let mut files = Vec::new();
        for i in 0..5000 {
            files.push(File {
                id: 0,
                path: format!("/test/mixed/file_{i}.txt"),
                size: if i % 100 == 0 { 0 } else { 1024 }, // Some with size 0
                modified_time: time_utils::now_millis(),
                inode: Some(i),
                status: match i % 10 {
                    0 => FileStatus::Deleted,
                    1 => FileStatus::Error,
                    _ => FileStatus::Pending,
                },
                last_checked: time_utils::now_millis(),
                created_at: time_utils::now_millis() - i * 1000,
                updated_at: time_utils::now_millis(),
            });
        }

        let file_ids = file_repo.batch_insert(&files).await.unwrap();

        // Add hashes for 60% of valid files
        let mut hashes = Vec::new();
        for (i, &file_id) in file_ids.iter().enumerate() {
            if i % 10 >= 2 && i % 10 < 8 && i % 100 != 0 {
                // Skip deleted, error, and size=0
                hashes.push(Hash {
                    id: 0,
                    file_id,
                    algorithm: HashAlgorithm::ED2K.to_string(),
                    hash: format!("{i:032x}"),
                    duration_ms: 100,
                    created_at: time_utils::now_millis(),
                });
            }
        }
        hash_repo.batch_insert(&hashes).await.unwrap();

        // Test query performance
        let start = std::time::Instant::now();
        let files_without_hashes = file_repo.find_files_without_hashes(1000).await.unwrap();
        let duration = start.elapsed();

        println!("find_files_without_hashes for 1000 files took: {duration:?}");

        // Should complete quickly even with complex join
        assert!(
            duration.as_millis() < 100,
            "Query took {duration:?}, expected < 100ms"
        );

        // Verify results are correct
        assert!(!files_without_hashes.is_empty());
        for file in &files_without_hashes {
            assert_ne!(file.status, FileStatus::Deleted);
            assert_ne!(file.status, FileStatus::Error);
            assert!(file.size > 0);
        }
    }
}

mod stress_tests {
    use super::*;
    use anidb_client_core::database::{
        File, FileRepository, FileStatus, Repository, models::time_utils,
    };
    use std::sync::Arc;
    use tokio::task::JoinSet;

    #[tokio::test]
    async fn test_concurrent_batch_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("stress.db");
        let db = Database::new(&db_path).await.unwrap();
        let pool = Arc::new(db.pool().clone());

        // Spawn multiple tasks doing batch operations concurrently
        let mut tasks = JoinSet::new();

        // Writers
        for task_id in 0..5 {
            let pool_clone = pool.clone();
            tasks.spawn(async move {
                let file_repo = FileRepository::new((*pool_clone).clone());

                for batch in 0..10 {
                    let mut files = Vec::new();
                    for i in 0..100 {
                        let idx = task_id * 1000 + batch * 100 + i;
                        files.push(File {
                            id: 0,
                            path: format!("/stress/task_{task_id}/file_{idx}.txt"),
                            size: 1024,
                            modified_time: time_utils::now_millis(),
                            inode: None,
                            status: FileStatus::Pending,
                            last_checked: time_utils::now_millis(),
                            created_at: time_utils::now_millis(),
                            updated_at: time_utils::now_millis(),
                        });
                    }

                    file_repo.batch_insert(&files).await.unwrap();
                }
            });
        }

        // Readers
        for reader_id in 0..3 {
            let pool_clone = pool.clone();
            tasks.spawn(async move {
                let file_repo = FileRepository::new((*pool_clone).clone());

                for _ in 0..50 {
                    // Random reads
                    let path =
                        format!("/stress/task_{}/file_{}.txt", reader_id % 5, reader_id * 10);
                    file_repo.find_by_path(Path::new(&path)).await.ok();

                    // Status queries
                    file_repo
                        .find_by_status(FileStatus::Pending, 10)
                        .await
                        .unwrap();

                    // Files without hashes
                    file_repo.find_files_without_hashes(50).await.unwrap();

                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
            });
        }

        // Wait for all tasks to complete
        while let Some(result) = tasks.join_next().await {
            result.unwrap();
        }

        // Verify final state
        let file_repo = FileRepository::new((*pool).clone());
        let total_count = file_repo.count().await.unwrap();
        assert_eq!(total_count, 5000); // 5 writers * 10 batches * 100 files
    }
}
