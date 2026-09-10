// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! `BatchWriter` — capacity / time-interval dual-threshold buffered writer.
//!
//! Buffers `set` operations and flushes them to the underlying
//! [`CacheBackend`] in bulk when either:
//!
//! - the buffer reaches `capacity` entries, **or**
//! - `flush_interval` elapses (driven by a background tokio task).
//!
//! # Feature gate
//!
//! This module is only compiled when the `batch` cargo feature is enabled.
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::batch::BatchWriter;
//! use oxcache::backend::MokaMemoryBackend;
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! let backend = Arc::new(MokaMemoryBackend::builder().capacity(1000).build());
//! let writer = BatchWriter::builder(backend)
//!     .capacity(100)
//!     .flush_interval(Duration::from_secs(5))
//!     .build();
//! writer.start();
//! writer.enqueue("key".into(), Arc::new(b"value".to_vec()), None).await;
//! // Flushes automatically when buffer fills or interval elapses.
//! writer.flush().await; // manual flush
//! writer.stop().await;  // stop background task
//! ```

use crate::backend::CacheBackend;
use crate::error::OxCacheResult;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Notify};

/// A single buffered write operation.
#[derive(Debug, Clone)]
struct BatchEntry {
    key: Arc<str>,
    value: Arc<Vec<u8>>,
    ttl: Option<Duration>,
}

/// Buffered batch writer that flushes to a [`CacheBackend`] on dual
/// thresholds (capacity and time interval).
pub struct BatchWriter {
    backend: Arc<dyn CacheBackend>,
    buffer: Mutex<Vec<BatchEntry>>,
    capacity: usize,
    flush_interval: Duration,
    /// Signalled when `stop()` is called to terminate the background flusher.
    stop_notify: Arc<Notify>,
}

impl BatchWriter {
    /// Create a new [`BatchWriterBuilder`].
    pub fn builder(backend: Arc<dyn CacheBackend>) -> BatchWriterBuilder {
        BatchWriterBuilder::new(backend)
    }

    /// Enqueue a `set` operation. If the buffer reaches capacity, an
    /// automatic flush is triggered.
    pub async fn enqueue(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let should_flush = {
            let mut buf = self.buffer.lock().await;
            buf.push(BatchEntry { key, value, ttl });
            buf.len() >= self.capacity
        };
        if should_flush {
            self.flush().await?;
        }
        Ok(())
    }

    /// Flush all buffered entries to the backend.
    pub async fn flush(&self) -> OxCacheResult<()> {
        let entries: Vec<BatchEntry> = {
            let mut buf = self.buffer.lock().await;
            std::mem::take(&mut *buf)
        };
        if entries.is_empty() {
            return Ok(());
        }
        for entry in &entries {
            self.backend
                .set(entry.key.clone(), entry.value.clone(), entry.ttl)
                .await?;
        }
        Ok(())
    }

    /// Return the current number of buffered entries.
    pub async fn pending(&self) -> usize {
        self.buffer.lock().await.len()
    }

    /// Start the background flush task that fires every `flush_interval`.
    ///
    /// Call [`stop`](Self::stop) to terminate it.
    pub fn start(self: &Arc<Self>) {
        let writer = Arc::clone(self);
        let stop = Arc::clone(&self.stop_notify);
        let interval = self.flush_interval;
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(interval) => {
                        let _ = writer.flush().await;
                    }
                    _ = stop.notified() => {
                        // Drain remaining entries before exiting.
                        let _ = writer.flush().await;
                        return;
                    }
                }
            }
        });
    }

    /// Stop the background flush task and perform a final drain.
    pub async fn stop(&self) -> OxCacheResult<()> {
        self.stop_notify.notify_one();
        // Give the background task a moment to drain.
        tokio::time::sleep(Duration::from_millis(10)).await;
        self.flush().await
    }
}

/// Builder for [`BatchWriter`].
pub struct BatchWriterBuilder {
    backend: Arc<dyn CacheBackend>,
    capacity: usize,
    flush_interval: Duration,
}

impl BatchWriterBuilder {
    fn new(backend: Arc<dyn CacheBackend>) -> Self {
        Self {
            backend,
            capacity: 100,
            flush_interval: Duration::from_secs(5),
        }
    }

    /// Maximum number of buffered entries before auto-flush (default 100).
    pub fn capacity(mut self, cap: usize) -> Self {
        self.capacity = cap.max(1);
        self
    }

    /// Background flush interval (default 5 s).
    pub fn flush_interval(mut self, interval: Duration) -> Self {
        self.flush_interval = interval;
        self
    }

    /// Build the [`BatchWriter`].
    pub fn build(self) -> BatchWriter {
        BatchWriter {
            backend: self.backend,
            buffer: Mutex::new(Vec::new()),
            capacity: self.capacity,
            flush_interval: self.flush_interval,
            stop_notify: Arc::new(Notify::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MokaMemoryBackend;

    fn test_backend() -> Arc<dyn CacheBackend> {
        Arc::new(MokaMemoryBackend::builder().capacity(1000).build())
    }

    #[tokio::test]
    async fn batch_writer_enqueues_and_flushes() {
        let backend = test_backend();
        let bw = Arc::new(
            BatchWriter::builder(backend.clone())
                .capacity(10)
                .flush_interval(Duration::from_secs(60))
                .build(),
        );

        // Enqueue 3 entries — below capacity, so no auto-flush.
        for i in 0..3u8 {
            bw.enqueue(
                Arc::from(format!("k{i}")),
                Arc::new(vec![i]),
                None,
            )
            .await
            .expect("enqueue");
        }
        assert_eq!(bw.pending().await, 3);

        // Manual flush pushes all entries to the backend.
        bw.flush().await.expect("flush");
        assert_eq!(bw.pending().await, 0);

        // Verify backend received the data.
        let val = backend.get("k1").await.expect("get");
        assert_eq!(val, Some(vec![1]));
    }

    #[tokio::test]
    async fn batch_writer_auto_flushes_at_capacity() {
        let backend = test_backend();
        let bw = Arc::new(
            BatchWriter::builder(backend.clone())
                .capacity(3)
                .flush_interval(Duration::from_secs(60))
                .build(),
        );

        // Enqueue up to capacity — should trigger auto-flush.
        for i in 0..3u8 {
            bw.enqueue(
                Arc::from(format!("auto{i}")),
                Arc::new(vec![i]),
                None,
            )
            .await
            .expect("enqueue");
        }

        // Buffer should be drained after auto-flush.
        assert_eq!(bw.pending().await, 0);

        // Backend has the data.
        let val = backend.get("auto2").await.expect("get");
        assert_eq!(val, Some(vec![2]));
    }

    #[tokio::test]
    async fn batch_writer_stop_drains_remaining() {
        let backend = test_backend();
        let bw = Arc::new(
            BatchWriter::builder(backend.clone())
                .capacity(100)
                .flush_interval(Duration::from_secs(60))
                .build(),
        );
        bw.start();

        bw.enqueue(Arc::from("drain"), Arc::new(b"data".to_vec()), None)
            .await
            .expect("enqueue");
        assert_eq!(bw.pending().await, 1);

        bw.stop().await.expect("stop");

        // After stop, remaining entries should be drained.
        let val = backend.get("drain").await.expect("get");
        assert_eq!(val, Some(b"data".to_vec()));
    }

    #[tokio::test]
    async fn batch_writer_flush_empty_is_noop() {
        let bw = BatchWriter::builder(test_backend()).build();
        // Flushing an empty buffer should succeed without error.
        bw.flush().await.expect("flush empty");
    }

    #[test]
    fn batch_writer_builder_defaults() {
        let bw = BatchWriter::builder(test_backend()).build();
        assert_eq!(bw.capacity, 100);
        assert_eq!(bw.flush_interval, Duration::from_secs(5));
    }
}
