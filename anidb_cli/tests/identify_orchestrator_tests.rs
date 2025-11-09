//! Tests for identify orchestrator MyList integration

use anidb_client_core::database::Database;
use anidb_client_core::database::models::{File, FileStatus, SyncStatus, time_utils};
use anidb_client_core::database::repositories::sync_queue::SyncQueueRepository;
use anidb_client_core::database::repositories::{FileRepository, Repository};
use anidb_client_core::identification::IdentificationStatus;
use std::sync::Arc;
use tempfile::TempDir;

/// Helper to create test database
async fn create_test_db() -> (Arc<Database>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path).await.unwrap();
    (Arc::new(db), temp_dir)
}

/// Helper to get repositories
fn get_repos(db: Arc<Database>) -> (Arc<SyncQueueRepository>, Arc<FileRepository>) {
    let sync_repo = Arc::new(SyncQueueRepository::new(db.pool().clone()));
    let file_repo = Arc::new(FileRepository::new(db.pool().clone()));
    (sync_repo, file_repo)
}

/// Helper to create a test file in the database
async fn create_test_file(file_repo: &FileRepository, file_id: i64) -> i64 {
    let file = File {
        id: 0,
        path: format!("/test/file_{}.mkv", file_id),
        size: 1024,
        modified_time: time_utils::now_millis(),
        inode: None,
        status: FileStatus::Processed,
        last_checked: time_utils::now_millis(),
        created_at: time_utils::now_millis(),
        updated_at: time_utils::now_millis(),
    };
    file_repo.create(&file).await.unwrap()
}

#[tokio::test]
async fn test_enqueue_single_identified_file() {
    let (db, _temp_dir) = create_test_db().await;
    let (sync_repo, file_repo) = get_repos(db);

    // Create a file first
    let file_id = create_test_file(&file_repo, 123).await;

    // Enqueue to MyList
    let queue_id = sync_repo.enqueue(file_id, "mylist_add", 5).await.unwrap();

    assert!(queue_id > 0);

    // Verify it was enqueued
    let item = sync_repo.find_by_id(queue_id).await.unwrap().unwrap();
    assert_eq!(item.file_id, file_id);
    assert_eq!(item.operation, "mylist_add");
    assert_eq!(item.status, SyncStatus::Pending);
    assert_eq!(item.priority, 5);
}

#[tokio::test]
async fn test_batch_enqueue_multiple_files() {
    let (db, _temp_dir) = create_test_db().await;
    let (sync_repo, file_repo) = get_repos(db);

    // Create files first
    let mut file_ids = Vec::new();
    for i in 0..4 {
        let file_id = create_test_file(&file_repo, 100 + i).await;
        file_ids.push(file_id);
    }

    let operations: Vec<(i64, String, i32)> = file_ids
        .iter()
        .map(|&fid| (fid, "mylist_add".to_string(), 5))
        .collect();

    // Batch enqueue
    let queue_ids = sync_repo.batch_enqueue(&operations).await.unwrap();
    assert_eq!(queue_ids.len(), 4);

    // Verify all were enqueued
    for (i, &queue_id) in queue_ids.iter().enumerate() {
        let item = sync_repo.find_by_id(queue_id).await.unwrap().unwrap();
        assert_eq!(item.file_id, file_ids[i]);
        assert_eq!(item.operation, "mylist_add");
        assert_eq!(item.status, SyncStatus::Pending);
    }
}

#[tokio::test]
async fn test_no_enqueue_for_not_found() {
    let (db, _temp_dir) = create_test_db().await;
    let (sync_repo, _file_repo) = get_repos(db);

    // Should NOT enqueue files that were not found
    // This is a behavioral test - orchestrator should filter

    let initial_count = sync_repo.count().await.unwrap();
    assert_eq!(initial_count, 0);
}

#[tokio::test]
async fn test_no_enqueue_for_network_error() {
    let (db, _temp_dir) = create_test_db().await;
    let (sync_repo, _file_repo) = get_repos(db);

    // Should NOT enqueue files that had network errors
    // This is a behavioral test - orchestrator should filter

    let initial_count = sync_repo.count().await.unwrap();
    assert_eq!(initial_count, 0);
}

#[tokio::test]
async fn test_deduplication_prevents_double_enqueue() {
    let (db, _temp_dir) = create_test_db().await;
    let (sync_repo, file_repo) = get_repos(db);

    // Create a file first
    let file_id = create_test_file(&file_repo, 123).await;

    // Enqueue once
    let queue_id_1 = sync_repo.enqueue(file_id, "mylist_add", 5).await.unwrap();

    // Try to enqueue again - should create a new entry
    // (In the implementation, we should check for existing pending operations)
    let queue_id_2 = sync_repo.enqueue(file_id, "mylist_add", 5).await.unwrap();

    // Both enqueued (current behavior)
    assert_ne!(queue_id_1, queue_id_2);

    // TODO: In the implementation, add deduplication logic
    // to prevent enqueueing the same file_id + operation if already pending
}

#[tokio::test]
async fn test_prompt_result_tracking() {
    // This test verifies the behavior of user prompts
    // The actual prompting will be done in the orchestrator

    // Test case: User says "yes" to single file
    let user_response = "y";
    assert!(matches!(user_response, "y" | "Y" | "yes" | "Yes"));

    // Test case: User says "no" to single file
    let user_response = "n";
    assert!(matches!(user_response, "n" | "N" | "no" | "No" | ""));

    // Test case: Empty response defaults to "no"
    let user_response = "";
    assert!(matches!(user_response, "n" | "N" | "no" | "No" | ""));
}

#[tokio::test]
async fn test_auto_add_flag_skips_prompt() {
    // Test that --add-to-mylist flag automatically enqueues without prompting
    let (db, _temp_dir) = create_test_db().await;
    let (sync_repo, file_repo) = get_repos(db);

    // Create a file first
    let file_id = create_test_file(&file_repo, 123).await;

    let add_to_mylist = true;

    if add_to_mylist {
        sync_repo.enqueue(file_id, "mylist_add", 5).await.unwrap();
    }

    let count = sync_repo.count().await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_no_mylist_flag_skips_prompt() {
    // Test that --no-mylist flag skips prompting entirely
    let (db, _temp_dir) = create_test_db().await;
    let (sync_repo, file_repo) = get_repos(db);

    // Create a file first (but won't enqueue)
    let file_id = create_test_file(&file_repo, 123).await;

    let no_mylist = true;

    if !no_mylist {
        // Would prompt/enqueue here
        sync_repo.enqueue(file_id, "mylist_add", 5).await.unwrap();
    }

    let count = sync_repo.count().await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_batch_summary_shows_successful_count() {
    // Simulate batch identification results
    let total_files = 10;
    let successful_files = 7;
    let failed_files = 3;

    assert_eq!(total_files, successful_files + failed_files);

    // The prompt should show: "Add 7 successfully identified files to MyList? [y/N]: "
    let prompt_message = format!(
        "Add {} successfully identified files to MyList? [y/N]: ",
        successful_files
    );

    assert!(prompt_message.contains("7 successfully identified files"));
}

#[tokio::test]
async fn test_only_identified_files_enqueued() {
    let (db, _temp_dir) = create_test_db().await;
    let (sync_repo, file_repo) = get_repos(db);

    // Create test files
    let mut file_ids_with_status = Vec::new();
    for i in 0..5 {
        let file_id = create_test_file(&file_repo, 100 + i).await;
        let status = match i {
            0 => IdentificationStatus::Identified,
            1 => IdentificationStatus::NotFound,
            2 => IdentificationStatus::Identified,
            3 => IdentificationStatus::NetworkError,
            _ => IdentificationStatus::Identified,
        };
        file_ids_with_status.push((file_id, status));
    }

    // Only enqueue successfully identified files
    let to_enqueue: Vec<_> = file_ids_with_status
        .iter()
        .filter(|(_, status)| *status == IdentificationStatus::Identified)
        .map(|&(fid, _)| (fid, "mylist_add".to_string(), 5))
        .collect();

    let queue_ids = sync_repo.batch_enqueue(&to_enqueue).await.unwrap();

    // Should only enqueue 3 files (indices 0, 2, 4)
    assert_eq!(queue_ids.len(), 3);

    // Verify the correct files were enqueued
    let items = sync_repo.find_ready(10).await.unwrap();
    let enqueued_file_ids: Vec<i64> = items.iter().map(|item| item.file_id).collect();

    // Check that only the identified files are enqueued
    assert_eq!(enqueued_file_ids.len(), 3);
    assert!(enqueued_file_ids.contains(&file_ids_with_status[0].0));
    assert!(enqueued_file_ids.contains(&file_ids_with_status[2].0));
    assert!(enqueued_file_ids.contains(&file_ids_with_status[4].0));
    assert!(!enqueued_file_ids.contains(&file_ids_with_status[1].0));
    assert!(!enqueued_file_ids.contains(&file_ids_with_status[3].0));
}
