//! Combinators for composing pipeline stages
//!
//! This module provides utilities for combining and chaining stages
//! in flexible ways to create complex processing pipelines.

use super::ProcessingStage;
use crate::Result;
use async_trait::async_trait;
use std::fmt::Debug;

/// A stage that conditionally applies another stage based on a predicate
pub struct ConditionalStage<P>
where
    P: Fn(&[u8]) -> bool + Send + Sync,
{
    inner: Box<dyn ProcessingStage>,
    predicate: P,
    name: String,
}

impl<P> Debug for ConditionalStage<P>
where
    P: Fn(&[u8]) -> bool + Send + Sync,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConditionalStage")
            .field("name", &self.name)
            .finish()
    }
}

impl<P> ConditionalStage<P>
where
    P: Fn(&[u8]) -> bool + Send + Sync,
{
    /// Create a new conditional stage
    pub fn new(inner: Box<dyn ProcessingStage>, predicate: P) -> Self {
        let name = format!("Conditional[{}]", inner.name());
        Self {
            inner,
            predicate,
            name,
        }
    }
}

#[async_trait]
impl<P> ProcessingStage for ConditionalStage<P>
where
    P: Fn(&[u8]) -> bool + Send + Sync,
{
    async fn process(&mut self, chunk: &[u8]) -> Result<()> {
        if (self.predicate)(chunk) {
            self.inner.process(chunk).await
        } else {
            Ok(())
        }
    }

    async fn initialize(&mut self, total_size: u64) -> Result<()> {
        self.inner.initialize(total_size).await
    }

    async fn finalize(&mut self) -> Result<()> {
        self.inner.finalize().await
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// A stage that applies multiple stages in parallel
#[derive(Debug)]
pub struct ParallelStage {
    stages: Vec<Box<dyn ProcessingStage>>,
    name: String,
}

impl ParallelStage {
    /// Create a new parallel stage
    pub fn new(stages: Vec<Box<dyn ProcessingStage>>) -> Self {
        let stage_names: Vec<_> = stages.iter().map(|s| s.name()).collect();
        let name = format!("Parallel[{}]", stage_names.join(", "));
        Self { stages, name }
    }
}

#[async_trait]
impl ProcessingStage for ParallelStage {
    async fn process(&mut self, chunk: &[u8]) -> Result<()> {
        // Process all stages (sequentially for now, true parallelism would require Arc<Mutex>)
        for stage in &mut self.stages {
            stage.process(chunk).await?;
        }

        Ok(())
    }

    async fn initialize(&mut self, total_size: u64) -> Result<()> {
        for stage in &mut self.stages {
            stage.initialize(total_size).await?;
        }
        Ok(())
    }

    async fn finalize(&mut self) -> Result<()> {
        for stage in &mut self.stages {
            stage.finalize().await?;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// A stage that transforms data before passing it to another stage
pub struct TransformStage<T>
where
    T: Fn(&[u8]) -> Vec<u8> + Send + Sync,
{
    inner: Box<dyn ProcessingStage>,
    transform: T,
    name: String,
}

impl<T> Debug for TransformStage<T>
where
    T: Fn(&[u8]) -> Vec<u8> + Send + Sync,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransformStage")
            .field("name", &self.name)
            .finish()
    }
}

impl<T> TransformStage<T>
where
    T: Fn(&[u8]) -> Vec<u8> + Send + Sync,
{
    /// Create a new transform stage
    pub fn new(inner: Box<dyn ProcessingStage>, transform: T) -> Self {
        let name = format!("Transform[{}]", inner.name());
        Self {
            inner,
            transform,
            name,
        }
    }
}

#[async_trait]
impl<T> ProcessingStage for TransformStage<T>
where
    T: Fn(&[u8]) -> Vec<u8> + Send + Sync,
{
    async fn process(&mut self, chunk: &[u8]) -> Result<()> {
        let transformed = (self.transform)(chunk);
        self.inner.process(&transformed).await
    }

    async fn initialize(&mut self, total_size: u64) -> Result<()> {
        self.inner.initialize(total_size).await
    }

    async fn finalize(&mut self) -> Result<()> {
        self.inner.finalize().await
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// A stage that buffers chunks until a certain size is reached
#[derive(Debug)]
pub struct BufferingStage {
    inner: Box<dyn ProcessingStage>,
    buffer: Vec<u8>,
    buffer_size: usize,
    name: String,
}

impl BufferingStage {
    /// Create a new buffering stage
    pub fn new(inner: Box<dyn ProcessingStage>, buffer_size: usize) -> Self {
        let name = format!("Buffering[{}, {}KB]", inner.name(), buffer_size / 1024);
        Self {
            inner,
            buffer: Vec::with_capacity(buffer_size),
            buffer_size,
            name,
        }
    }
}

#[async_trait]
impl ProcessingStage for BufferingStage {
    async fn process(&mut self, chunk: &[u8]) -> Result<()> {
        self.buffer.extend_from_slice(chunk);

        // Process when buffer is full
        while self.buffer.len() >= self.buffer_size {
            let process_chunk = self.buffer.drain(..self.buffer_size).collect::<Vec<_>>();
            self.inner.process(&process_chunk).await?;
        }

        Ok(())
    }

    async fn initialize(&mut self, total_size: u64) -> Result<()> {
        self.buffer.clear();
        self.inner.initialize(total_size).await
    }

    async fn finalize(&mut self) -> Result<()> {
        // Process any remaining data in the buffer
        if !self.buffer.is_empty() {
            let remaining = std::mem::take(&mut self.buffer);
            self.inner.process(&remaining).await?;
        }

        self.inner.finalize().await
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// A stage that rate-limits processing
#[derive(Debug)]
pub struct RateLimitedStage {
    inner: Box<dyn ProcessingStage>,
    min_interval: std::time::Duration,
    last_process: Option<std::time::Instant>,
    name: String,
}

impl RateLimitedStage {
    /// Create a new rate-limited stage
    pub fn new(inner: Box<dyn ProcessingStage>, max_per_second: f64) -> Self {
        let min_interval = std::time::Duration::from_secs_f64(1.0 / max_per_second);
        let name = format!("RateLimited[{}, {:.1}/s]", inner.name(), max_per_second);
        Self {
            inner,
            min_interval,
            last_process: None,
            name,
        }
    }
}

#[async_trait]
impl ProcessingStage for RateLimitedStage {
    async fn process(&mut self, chunk: &[u8]) -> Result<()> {
        // Check if we need to wait
        if let Some(last) = self.last_process {
            let elapsed = last.elapsed();
            if elapsed < self.min_interval {
                let wait_time = self.min_interval - elapsed;
                tokio::time::sleep(wait_time).await;
            }
        }

        // Process the chunk
        let result = self.inner.process(chunk).await;
        self.last_process = Some(std::time::Instant::now());
        result
    }

    async fn initialize(&mut self, total_size: u64) -> Result<()> {
        self.last_process = None;
        self.inner.initialize(total_size).await
    }

    async fn finalize(&mut self) -> Result<()> {
        self.inner.finalize().await
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Extension trait for composing stages
pub trait StageExt: ProcessingStage + Sized {
    /// Apply this stage conditionally based on a predicate
    fn when<P>(self, predicate: P) -> ConditionalStage<P>
    where
        P: Fn(&[u8]) -> bool + Send + Sync,
        Self: 'static,
    {
        ConditionalStage::new(Box::new(self), predicate)
    }

    /// Transform data before processing
    fn transform<T>(self, transform: T) -> TransformStage<T>
    where
        T: Fn(&[u8]) -> Vec<u8> + Send + Sync,
        Self: 'static,
    {
        TransformStage::new(Box::new(self), transform)
    }

    /// Buffer chunks before processing
    fn buffered(self, buffer_size: usize) -> BufferingStage
    where
        Self: 'static,
    {
        BufferingStage::new(Box::new(self), buffer_size)
    }

    /// Rate-limit processing
    fn rate_limited(self, max_per_second: f64) -> RateLimitedStage
    where
        Self: 'static,
    {
        RateLimitedStage::new(Box::new(self), max_per_second)
    }
}

// Implement StageExt for all ProcessingStage types
impl<T: ProcessingStage> StageExt for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct CountingStage {
        count: Arc<AtomicUsize>,
    }

    impl CountingStage {
        fn new() -> Self {
            Self {
                count: Arc::new(AtomicUsize::new(0)),
            }
        }

        #[allow(dead_code)]
        fn count(&self) -> usize {
            self.count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl ProcessingStage for CountingStage {
        async fn process(&mut self, chunk: &[u8]) -> Result<()> {
            let len = chunk.len();
            self.count.fetch_add(len, Ordering::SeqCst);
            Ok(())
        }

        fn name(&self) -> &str {
            "CountingStage"
        }
    }

    #[tokio::test]
    async fn test_conditional_stage() {
        let counting = CountingStage::new();
        let count_ref = counting.count.clone();

        // Only process chunks larger than 10 bytes
        let mut conditional = counting.when(|chunk| chunk.len() > 10);

        conditional.process(&[0; 5]).await.unwrap(); // Should not count
        assert_eq!(count_ref.load(Ordering::SeqCst), 0);

        conditional.process(&[0; 15]).await.unwrap(); // Should count
        assert_eq!(count_ref.load(Ordering::SeqCst), 15);
    }

    #[tokio::test]
    async fn test_transform_stage() {
        let counting = CountingStage::new();
        let count_ref = counting.count.clone();

        // Double the chunk size
        let mut transform = counting.transform(|chunk| {
            let mut doubled = Vec::with_capacity(chunk.len() * 2);
            doubled.extend_from_slice(chunk);
            doubled.extend_from_slice(chunk);
            doubled
        });

        transform.process(&[0; 10]).await.unwrap();
        assert_eq!(count_ref.load(Ordering::SeqCst), 20); // 10 * 2
    }

    #[tokio::test]
    async fn test_buffering_stage() {
        let counting = CountingStage::new();
        let count_ref = counting.count.clone();

        let mut buffering = counting.buffered(20);

        // Initialize the buffering stage
        buffering.initialize(100).await.unwrap();
        assert_eq!(count_ref.load(Ordering::SeqCst), 0, "After init");

        // These should be buffered
        buffering.process(&[0; 5]).await.unwrap();
        assert_eq!(count_ref.load(Ordering::SeqCst), 0, "After 5 bytes");

        buffering.process(&[0; 5]).await.unwrap();
        assert_eq!(count_ref.load(Ordering::SeqCst), 0, "After 10 bytes");

        buffering.process(&[0; 5]).await.unwrap();
        assert_eq!(count_ref.load(Ordering::SeqCst), 0, "After 15 bytes"); // Not processed yet

        // This should trigger processing of 20 bytes
        buffering.process(&[0; 10]).await.unwrap();
        assert_eq!(count_ref.load(Ordering::SeqCst), 20, "After 25 bytes");

        // Finalize should process remaining 5 bytes
        buffering.finalize().await.unwrap();
        assert_eq!(count_ref.load(Ordering::SeqCst), 25, "After finalize");
    }

    #[tokio::test]
    async fn test_parallel_stage() {
        let counting1 = CountingStage::new();
        let count1_ref = counting1.count.clone();

        let counting2 = CountingStage::new();
        let count2_ref = counting2.count.clone();

        let mut parallel = ParallelStage::new(vec![Box::new(counting1), Box::new(counting2)]);

        parallel.process(&[0; 10]).await.unwrap();

        assert_eq!(count1_ref.load(Ordering::SeqCst), 10);
        assert_eq!(count2_ref.load(Ordering::SeqCst), 10);
    }

    #[tokio::test]
    async fn test_chained_combinators() {
        // Test that combinators can be chained together
        let counting = CountingStage::new();
        let count_ref = counting.count.clone();

        // Create a chain that:
        // 1. Only processes chunks >= 5 bytes
        // 2. Takes first 5 bytes of each chunk
        // 3. Buffers to 10 bytes before processing
        let mut chained = counting
            .buffered(10) // Buffer to 10 bytes
            .transform(|chunk| {
                // Take first 5 bytes
                let len = chunk.len().min(5);
                chunk[..len].to_vec()
            })
            .when(|chunk| chunk.len() >= 5); // Only process chunks >= 5 bytes

        chained.process(&[0; 3]).await.unwrap(); // Filtered out (< 5)
        assert_eq!(count_ref.load(Ordering::SeqCst), 0);

        chained.process(&[0; 8]).await.unwrap(); // Passes filter, transformed to 5, buffered
        assert_eq!(count_ref.load(Ordering::SeqCst), 0);

        chained.process(&[0; 7]).await.unwrap(); // Passes filter, transformed to 5, now have 10 in buffer
        assert_eq!(count_ref.load(Ordering::SeqCst), 10);
    }
}
