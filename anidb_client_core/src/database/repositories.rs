//! Repository pattern implementation for database access
//!
//! This module provides repository interfaces for each entity type,
//! handling all database operations with proper error handling and transactions.

pub mod anidb_result;
pub mod file;
pub mod hash;
pub mod mylist;
pub mod sync_queue;

use crate::Result;
use async_trait::async_trait;
use sqlx::SqlitePool;

// Re-export repository implementations
pub use anidb_result::{AniDBResultRepository, AnimeStats};
pub use file::FileRepository;
pub use hash::HashRepository;
pub use mylist::MyListRepository;
pub use sync_queue::{QueueStats, SyncQueueRepository};

/// Base repository trait for common operations
#[async_trait]
pub trait Repository<T> {
    /// Create a new entity
    async fn create(&self, entity: &T) -> Result<i64>;

    /// Find an entity by ID
    async fn find_by_id(&self, id: i64) -> Result<Option<T>>;

    /// Update an existing entity
    async fn update(&self, entity: &T) -> Result<()>;

    /// Delete an entity by ID
    async fn delete(&self, id: i64) -> Result<()>;

    /// Count all entities
    async fn count(&self) -> Result<i64>;
}

/// Transaction support for repositories
pub struct Transaction<'a> {
    tx: sqlx::Transaction<'a, sqlx::Sqlite>,
}

impl<'a> Transaction<'a> {
    /// Create a new transaction
    pub async fn new(pool: &SqlitePool) -> Result<Transaction<'_>> {
        let tx = pool.begin().await?;
        Ok(Transaction { tx })
    }

    /// Commit the transaction
    pub async fn commit(self) -> Result<()> {
        self.tx.commit().await?;
        Ok(())
    }

    /// Rollback the transaction
    pub async fn rollback(self) -> Result<()> {
        self.tx.rollback().await?;
        Ok(())
    }

    /// Get a reference to the transaction for queries
    pub fn transaction_mut(&mut self) -> &mut sqlx::Transaction<'a, sqlx::Sqlite> {
        &mut self.tx
    }
}
