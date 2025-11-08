//! Ring buffer manager for lock-free parallel hashing
//!
//! This module implements a ring buffer architecture that allows multiple hash
//! algorithms to process data at different speeds without blocking each other.
//! A single reader fills the ring, and each algorithm can lag behind or race
//! ahead independently.

// Avoid tracked buffer allocation to prevent global memory counter inflation during tests
use crate::{Error, Result, error::InternalError};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use tokio::sync::Notify;

/// A tracked buffer that releases memory on drop
struct TrackedBuffer {
    data: Vec<u8>,
}

impl Drop for TrackedBuffer {
    fn drop(&mut self) {
        // Plain Vec<u8> will be dropped by Rust; no global memory tracking here
        let _ = std::mem::take(&mut self.data);
    }
}

/// Size of the ring buffer (number of slots)
/// Chosen to balance memory usage vs allowing algorithms to diverge
/// With ED2K's 9.5MB chunks, we need to keep this small to fit in 500MB
const RING_SIZE: usize = 32;

/// A single buffer slot in the ring
pub(super) struct BufferSlot {
    /// The actual data buffer (wrapped in Arc for sharing)
    data: tokio::sync::RwLock<Option<Arc<TrackedBuffer>>>,
    /// Number of bytes valid in this buffer
    size: AtomicUsize,
    /// Sequence number for this chunk
    sequence: AtomicU64,
    /// Reference count - number of algorithms still using this buffer
    ref_count: AtomicUsize,
    /// Whether this slot contains the last chunk
    is_last: AtomicBool,
}

impl BufferSlot {
    fn new() -> Self {
        Self {
            data: tokio::sync::RwLock::new(None),
            size: AtomicUsize::new(0),
            sequence: AtomicU64::new(0),
            ref_count: AtomicUsize::new(0),
            is_last: AtomicBool::new(false),
        }
    }

    /// Reset the slot for reuse
    #[allow(dead_code)]
    async fn reset(&self) {
        *self.data.write().await = None;
        self.size.store(0, Ordering::Relaxed);
        self.sequence.store(0, Ordering::Relaxed);
        self.ref_count.store(0, Ordering::Relaxed);
        self.is_last.store(false, Ordering::Relaxed);
    }
}

/// Ring buffer manager for parallel hashing
pub struct BufferRing {
    /// The ring of buffer slots
    slots: Vec<Arc<BufferSlot>>,
    /// Current write position (for the reader)
    write_pos: AtomicUsize,
    /// Number of active readers (algorithms)
    num_readers: usize,
    /// Notification for when slots become available
    slot_available: Arc<Notify>,
    /// Notification for when new data is available for reading
    data_available: Arc<Notify>,
    /// Buffer size for each slot
    buffer_size: usize,
    /// Total number of chunks written
    total_chunks: AtomicU64,
    /// Whether writing is complete
    write_complete: AtomicBool,
}

impl BufferRing {
    /// Create a new buffer ring
    pub fn new(buffer_size: usize, num_readers: usize) -> Result<Self> {
        // Calculate memory usage
        let memory_per_slot = buffer_size;
        let total_memory = memory_per_slot * RING_SIZE;

        // Check if we'd exceed memory limit
        let memory_limit = crate::buffer::get_memory_limit();
        if total_memory > memory_limit {
            return Err(Error::Internal(InternalError::memory_limit_exceeded(
                memory_limit,
                total_memory,
            )));
        }

        let mut slots = Vec::with_capacity(RING_SIZE);
        for _ in 0..RING_SIZE {
            slots.push(Arc::new(BufferSlot::new()));
        }

        Ok(Self {
            slots,
            write_pos: AtomicUsize::new(0),
            num_readers,
            slot_available: Arc::new(Notify::new()),
            data_available: Arc::new(Notify::new()),
            buffer_size,
            total_chunks: AtomicU64::new(0),
            write_complete: AtomicBool::new(false),
        })
    }

    /// Get a slot for writing (blocks until available)
    pub async fn get_write_slot(&self) -> Result<(Arc<BufferSlot>, Vec<u8>)> {
        let notify = self.slot_available.clone();

        loop {
            let pos = self.write_pos.load(Ordering::Acquire);
            let slot = &self.slots[pos % RING_SIZE];

            // Check if this slot is free (ref_count == 0)
            if slot.ref_count.load(Ordering::Acquire) == 0 {
                // Allocate a new buffer (untracked to avoid interfering with global memory counters)
                // The old one (if any) will be dropped when TrackedBuffer is dropped
                let buffer = vec![0u8; self.buffer_size];

                // Initialize for new write
                slot.ref_count.store(self.num_readers, Ordering::Release);

                return Ok((slot.clone(), buffer));
            }

            // Slot is still in use, wait for notification
            notify.notified().await;
        }
    }

    /// Commit a written buffer
    pub async fn commit_write(
        &self,
        slot: Arc<BufferSlot>,
        buffer: Vec<u8>,
        size: usize,
        is_last: bool,
    ) {
        // Store the data wrapped in Arc for sharing
        let tracked = TrackedBuffer { data: buffer };
        *slot.data.write().await = Some(Arc::new(tracked));
        slot.size.store(size, Ordering::Release);
        slot.sequence
            .store(self.total_chunks.load(Ordering::Acquire), Ordering::Release);
        slot.is_last.store(is_last, Ordering::Release);

        // Update counters
        self.total_chunks.fetch_add(1, Ordering::AcqRel);
        self.write_pos.fetch_add(1, Ordering::AcqRel);

        if is_last {
            self.write_complete.store(true, Ordering::Release);
        }

        // Notify all waiting readers that new data is available
        self.data_available.notify_waiters();
    }

    /// Check if writing is complete
    pub fn is_write_complete(&self) -> bool {
        self.write_complete.load(Ordering::Acquire)
    }

    /// Create a cursor for an algorithm to read from the ring
    pub fn create_cursor(self: &Arc<Self>) -> RingCursor {
        RingCursor {
            ring: self.clone(),
            read_pos: AtomicUsize::new(0),
            chunks_read: AtomicU64::new(0),
            data_notifier: self.data_available.clone(),
        }
    }

    /// Release a buffer after an algorithm is done with it
    #[allow(dead_code)]
    pub fn release(&self, slot: &Arc<BufferSlot>) {
        let old_count = slot.ref_count.fetch_sub(1, Ordering::AcqRel);
        if old_count == 1 {
            // This was the last reference, notify that slot is available
            self.slot_available.notify_one();
        }
    }

    /// Get the notification handle for slot availability
    pub fn get_slot_notifier(&self) -> Arc<Notify> {
        self.slot_available.clone()
    }
}

impl Drop for BufferRing {
    fn drop(&mut self) {
        // TrackedBuffer will automatically release memory when dropped
        // No manual cleanup needed
    }
}

impl BufferRing {
    /// Write next chunk from a file to the ring buffer
    pub async fn write_next<R: tokio::io::AsyncRead + Unpin>(
        &self,
        reader: &mut R,
    ) -> Result<Option<usize>> {
        use tokio::io::AsyncReadExt;

        // Get a write slot
        let (slot, mut buffer) = self.get_write_slot().await?;

        // Read from the file
        let n = reader.read(&mut buffer).await?;

        if n == 0 {
            // End of file - we need to release the slot we acquired
            // Since we're not writing anything, decrement the ref count
            slot.ref_count.store(0, Ordering::Release);
            self.slot_available.notify_one();
            return Ok(None);
        }

        // Commit the write
        self.commit_write(slot, buffer, n, false).await;
        Ok(Some(n))
    }

    /// Mark writing as complete
    pub fn mark_complete(&self) {
        self.write_complete.store(true, Ordering::Release);
        // Notify all waiting readers that writing is complete
        self.data_available.notify_waiters();
    }

    /// Create a reader for consuming from the ring
    pub fn create_reader(self: &Arc<Self>) -> RingReader {
        RingReader {
            cursor: self.create_cursor(),
        }
    }
}

/// Reader interface for consuming from the ring buffer
pub struct RingReader {
    cursor: RingCursor,
}

impl RingReader {
    /// Read the next chunk from the ring
    pub async fn read_next(&mut self) -> Option<RingChunk> {
        self.cursor.read().await.map(|data| RingChunk { data })
    }
}

/// A chunk read from the ring buffer
pub struct RingChunk {
    data: ChunkData,
}

impl RingChunk {
    /// Get the data slice
    pub fn data(&self) -> &[u8] {
        self.data.data()
    }

    /// Mark this chunk as consumed
    pub fn mark_consumed(self) {
        // Dropping self will trigger ChunkData's Drop impl
        // which decrements the ref count
    }
}

impl std::ops::Deref for RingChunk {
    type Target = ChunkData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

/// Cursor for reading from the ring buffer
pub struct RingCursor {
    ring: Arc<BufferRing>,
    read_pos: AtomicUsize,
    chunks_read: AtomicU64,
    data_notifier: Arc<Notify>,
}

/// Data returned when reading from the ring
pub struct ChunkData {
    /// The tracked buffer (shared reference)
    tracked_buffer: Arc<TrackedBuffer>,
    /// Number of valid bytes
    pub size: usize,
    /// Sequence number
    #[allow(dead_code)]
    pub sequence: u64,
    /// Whether this is the last chunk
    #[allow(dead_code)]
    pub is_last: bool,
    /// The slot this came from (for release)
    slot: Arc<BufferSlot>,
    /// Notifier for when slots become available
    slot_notifier: Arc<Notify>,
}

impl ChunkData {
    /// Get a reference to the data
    pub fn data(&self) -> &[u8] {
        &self.tracked_buffer.data[..self.size]
    }
}

impl RingCursor {
    /// Try to read the next chunk without blocking
    pub async fn try_read(&self) -> Option<ChunkData> {
        let chunks_read = self.chunks_read.load(Ordering::Acquire);
        let total_written = self.ring.total_chunks.load(Ordering::Acquire);

        // Check if we've caught up to the writer
        if chunks_read >= total_written {
            // If writing is complete and we've read everything, we're done
            if self.ring.is_write_complete() {
                return None;
            }
            // Otherwise, no data available yet
            return None;
        }

        // Calculate which slot to read from
        let pos = self.read_pos.load(Ordering::Acquire);
        let slot = &self.ring.slots[pos % RING_SIZE];

        // Check if this slot has the sequence we need
        let slot_seq = slot.sequence.load(Ordering::Acquire);

        if slot_seq != chunks_read {
            // Slot doesn't have our data yet (writer hasn't caught up after wrapping)
            return None;
        }

        // Read the data
        let data_guard = slot.data.read().await;
        if let Some(buffer_arc) = data_guard.as_ref() {
            // Share the Arc reference - no new allocation
            let chunk = ChunkData {
                tracked_buffer: buffer_arc.clone(), // Just clone the Arc, not the data
                size: slot.size.load(Ordering::Acquire),
                sequence: slot_seq,
                is_last: slot.is_last.load(Ordering::Acquire),
                slot: slot.clone(),
                slot_notifier: self.ring.get_slot_notifier(),
            };

            // Update our position
            self.read_pos.fetch_add(1, Ordering::AcqRel);
            self.chunks_read.fetch_add(1, Ordering::AcqRel);

            Some(chunk)
        } else {
            None
        }
    }

    /// Read the next chunk, waiting if necessary
    pub async fn read(&self) -> Option<ChunkData> {
        loop {
            // Try to read first
            if let Some(chunk) = self.try_read().await {
                return Some(chunk);
            }

            // Check if we're done - do this check AFTER trying to read
            // to avoid race conditions where data is available but write_complete is set
            let chunks_read = self.chunks_read.load(Ordering::Acquire);
            let total_written = self.ring.total_chunks.load(Ordering::Acquire);

            // If write is complete AND we've read all chunks, we're done
            if self.ring.is_write_complete() && chunks_read >= total_written {
                // Double-check by trying to read one more time to handle race conditions
                if let Some(chunk) = self.try_read().await {
                    return Some(chunk);
                }
                return None;
            }

            // Wait for notification with a timeout to prevent deadlocks
            // This ensures we periodically re-check conditions even if notifications are missed
            tokio::select! {
                _ = self.data_notifier.notified() => {
                    // Got notification, loop back to try reading
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {
                    // Timeout - loop back to check conditions again
                }
            }
        }
    }
}

impl Drop for ChunkData {
    fn drop(&mut self) {
        // Release our reference to this slot by decrementing ref count
        let old_count = self.slot.ref_count.fetch_sub(1, Ordering::AcqRel);
        if old_count == 1 {
            // This was the last reference, notify that slot is available
            self.slot_notifier.notify_one();
        }
        // TrackedBuffer will handle memory release automatically when dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::LazyLock;
    use tokio::sync::Mutex as AsyncMutex;

    // Mutex to ensure tests run sequentially for memory tracking
    static TEST_MUTEX: LazyLock<AsyncMutex<()>> = LazyLock::new(|| AsyncMutex::new(()));

    #[tokio::test]
    async fn test_buffer_ring_basic() {
        let _guard = TEST_MUTEX.lock().await;
        crate::buffer::reset_memory_tracking();

        let ring = Arc::new(BufferRing::new(1024, 2).unwrap());

        // Write a chunk
        let (slot, mut buffer) = ring.get_write_slot().await.unwrap();
        buffer[0] = 42;
        ring.commit_write(slot, buffer, 1024, false).await;

        // Read from two cursors
        let cursor1 = ring.create_cursor();
        let cursor2 = ring.create_cursor();

        let chunk1 = cursor1.read().await.unwrap();
        assert_eq!(chunk1.data()[0], 42);
        assert_eq!(chunk1.sequence, 0);
        assert!(!chunk1.is_last);

        let chunk2 = cursor2.read().await.unwrap();
        assert_eq!(chunk2.data()[0], 42);
        assert_eq!(chunk2.sequence, 0);

        // Chunks should be dropped, releasing the slot
        drop(chunk1);
        drop(chunk2);
    }

    #[tokio::test]
    async fn test_ring_wraparound() {
        let _guard = TEST_MUTEX.lock().await;
        crate::buffer::reset_memory_tracking();

        let ring = Arc::new(BufferRing::new(1024, 1).unwrap());
        let cursor = ring.create_cursor();

        // Fill the ring and wrap around
        for i in 0..(RING_SIZE + 10) {
            let (slot, mut buffer) = ring.get_write_slot().await.unwrap();
            buffer[0] = i as u8;
            ring.commit_write(slot, buffer, 1024, false).await;

            // Read to keep up (otherwise we'd block)
            let chunk = cursor.read().await.unwrap();
            assert_eq!(chunk.data()[0], i as u8);
            assert_eq!(chunk.sequence, i as u64);
        }
    }

    #[tokio::test]
    async fn test_multiple_readers_different_speeds() {
        let _guard = TEST_MUTEX.lock().await;
        crate::buffer::reset_memory_tracking();

        let ring = Arc::new(BufferRing::new(1024, 2).unwrap());
        let fast_cursor = ring.create_cursor();
        let slow_cursor = ring.create_cursor();

        // Write several chunks
        for i in 0..10 {
            let (slot, mut buffer) = ring.get_write_slot().await.unwrap();
            buffer[0] = i;
            ring.commit_write(slot, buffer, 1024, i == 9).await;
        }

        // Fast reader consumes all
        let mut fast_chunks = vec![];
        while let Some(chunk) = fast_cursor.read().await {
            fast_chunks.push(chunk);
        }
        assert_eq!(fast_chunks.len(), 10);

        // Slow reader can still read (data is retained by ref counting)
        let mut slow_chunks = vec![];
        while let Some(chunk) = slow_cursor.read().await {
            slow_chunks.push(chunk);
        }
        assert_eq!(slow_chunks.len(), 10);

        // Verify data integrity
        for i in 0..10 {
            assert_eq!(fast_chunks[i].data()[0], i as u8);
            assert_eq!(slow_chunks[i].data()[0], i as u8);
        }
    }
}
