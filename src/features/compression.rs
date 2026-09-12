// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 自适应压缩装饰器（`compression` feature）
//!
//! [`CompressingBackend`] 对**超过阈值**的 value 做 zstd 压缩（小值零压缩
//! 开销），读取端按魔数自动识别：
//!
//! - zstd 魔数 `28 B5 2F FD` → zstd 解压；
//! - gzip 魔数 `1F 8B` → 兼容旧 gzip 数据（`serialization/utils` 产物）；
//! - 其余 → 原样透传。
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::features::compression::CompressingBackend;
//!
//! let backend = CompressingBackend::with_threshold(inner, 1024);
//! // < 1024 字节原样存储；≥ 1024 字节自动 zstd 压缩
//! ```

use crate::backend::interface::{BackendKind, CacheSetItem};
use crate::backend::{CacheBackend, CacheConnector, CacheReader, CacheWriter};
use crate::error::OxCacheResult;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// zstd 帧魔数：`0x28 B5 2F FD`
pub const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

/// 默认压缩阈值（字节）：低于该值不做压缩
pub const DEFAULT_COMPRESSION_THRESHOLD: usize = 1024;

/// 默认 zstd 压缩级别（3 为 zstd 默认，CPU/压缩率平衡）
pub const DEFAULT_ZSTD_LEVEL: i32 = 3;

/// 按阈值自适应压缩装饰器
pub struct CompressingBackend {
    inner: Arc<dyn CacheBackend>,
    threshold: usize,
    level: i32,
}

impl CompressingBackend {
    /// 以默认阈值（1024 字节）包装内层后端
    pub fn new(inner: Arc<dyn CacheBackend>) -> Self {
        Self::with_threshold(inner, DEFAULT_COMPRESSION_THRESHOLD)
    }

    /// 指定阈值与 zstd 级别
    pub fn with_threshold(inner: Arc<dyn CacheBackend>, threshold: usize) -> Self {
        Self {
            inner,
            threshold,
            level: DEFAULT_ZSTD_LEVEL,
        }
    }

    /// 自定义 zstd 压缩级别（1..=19）
    pub fn with_level(mut self, level: i32) -> Self {
        self.level = level.clamp(1, 19);
        self
    }

    /// 压缩阈值
    pub fn threshold(&self) -> usize {
        self.threshold
    }

    fn is_zstd(data: &[u8]) -> bool {
        data.len() >= 4 && data[..4] == ZSTD_MAGIC
    }

    fn is_gzip(data: &[u8]) -> bool {
        data.len() >= 2 && data[0] == 0x1F && data[1] == 0x8B
    }

    /// 解压（按魔数分发；非压缩数据原样返回）
    fn decode(&self, data: &[u8]) -> OxCacheResult<Vec<u8>> {
        if Self::is_zstd(data) {
            use std::io::Read;
            let mut decoder = zstd::stream::Decoder::new(data)
                .map_err(|e| crate::error::OxCacheError::Serialization(format!("zstd decode: {e}")))?;
            let mut out = Vec::new();
            decoder
                .read_to_end(&mut out)
                .map_err(|e| crate::error::OxCacheError::Serialization(format!("zstd decode: {e}")))?;
            Ok(out)
        } else if Self::is_gzip(data) {
            // 兼容旧 gzip 数据
            crate::infra::serialization::utils::decompress_data_with_limit(
                data,
                crate::infra::serialization::utils::MAX_DECOMPRESS_SIZE,
            )
        } else {
            Ok(data.to_vec())
        }
    }
}

#[async_trait::async_trait]
impl CacheReader for CompressingBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        match self.inner.get(key).await? {
            Some(data) => Ok(Some(self.decode(&data)?)),
            None => Ok(None),
        }
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        self.inner.exists(key).await
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        self.inner.ttl(key).await
    }

    async fn len(&self) -> OxCacheResult<u64> {
        self.inner.len().await
    }

    async fn capacity(&self) -> OxCacheResult<u64> {
        self.inner.capacity().await
    }

    async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        self.inner.stats().await
    }

    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        self.inner.keys(pattern).await
    }
}

#[async_trait::async_trait]
impl CacheWriter for CompressingBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        // 自适应：阈值以下零压缩开销
        let stored: Arc<Vec<u8>> = if value.len() >= self.threshold {
            let compressed = zstd::stream::encode_all(value.as_slice(), self.level)
                .map_err(|e| crate::error::OxCacheError::Serialization(format!("zstd encode: {e}")))?;
            Arc::new(compressed)
        } else {
            value
        };
        self.inner.set(key, stored, ttl).await
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.inner.delete(key).await
    }

    async fn clear(&self) -> OxCacheResult<()> {
        self.inner.clear().await
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        self.inner.expire(key, ttl).await
    }

    async fn set_many(&self, items: &[CacheSetItem]) -> OxCacheResult<()> {
        let mut adapted: Vec<CacheSetItem> = Vec::with_capacity(items.len());
        for (key, value, ttl) in items {
            let stored: Arc<Vec<u8>> = if value.len() >= self.threshold {
                let compressed = zstd::stream::encode_all(value.as_slice(), self.level)
                    .map_err(|e| {
                        crate::error::OxCacheError::Serialization(format!("zstd encode: {e}"))
                    })?;
                Arc::new(compressed)
            } else {
                value.clone()
            };
            adapted.push((key.clone(), stored, *ttl));
        }
        self.inner.set_many(&adapted).await
    }

    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        self.inner.delete_many(keys).await
    }
}

#[async_trait::async_trait]
impl CacheConnector for CompressingBackend {
    async fn health_check(&self) -> OxCacheResult<()> {
        self.inner.health_check().await
    }

    async fn shutdown(&self) {
        self.inner.shutdown().await;
    }

    fn backend_kind(&self) -> BackendKind {
        self.inner.backend_kind()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MockBackend;
    use crate::backend::interface::{CacheConnector, CacheReader, CacheWriter};

    fn backend() -> CompressingBackend {
        CompressingBackend::with_threshold(
            Arc::new(MockBackend::new("mock", 100, false)),
            256,
        )
    }

    fn compressible(len: usize) -> Vec<u8> {
        b"compressible-pattern-".repeat(len / 21 + 1)[..len].to_vec()
    }

    #[tokio::test]
    async fn small_values_stored_verbatim() {
        let backend = backend();
        let small = b"tiny".to_vec();
        backend
            .set(Arc::from("k"), Arc::new(small.clone()), None)
            .await
            .unwrap();

        // 阈值以下：原始字节原样存储（无压缩开销）
        let raw = backend.inner.get("k").await.unwrap().unwrap();
        assert_eq!(raw, small, "阈值以下不得压缩");
        assert_eq!(backend.get("k").await.unwrap(), Some(small));
    }

    #[tokio::test]
    async fn large_values_are_zstd_compressed_and_roundtrip() {
        let backend = backend();
        let large = compressible(8192);
        backend
            .set(Arc::from("big"), Arc::new(large.clone()), None)
            .await
            .unwrap();

        let raw = backend.inner.get("big").await.unwrap().unwrap();
        assert_eq!(raw[..4], ZSTD_MAGIC, "阈值以上应为 zstd 帧");
        assert!(
            raw.len() < large.len() / 4,
            "可压缩负载应显著缩小 (raw={} original={})",
            raw.len(),
            large.len()
        );
        assert_eq!(backend.get("big").await.unwrap(), Some(large));
    }

    #[tokio::test]
    async fn legacy_gzip_values_still_readable() {
        let backend = backend();
        // 直接向内层写入 gzip 数据（模拟旧版本产物）
        let original = compressible(2048);
        let gzip = crate::infra::serialization::utils::compress_data(&original).unwrap();
        assert!(gzip[0] == 0x1F && gzip[1] == 0x8B);
        backend
            .inner
            .set(Arc::from("legacy"), Arc::new(gzip), None)
            .await
            .unwrap();

        assert_eq!(backend.get("legacy").await.unwrap(), Some(original));
    }

    /// 体积对比记录（docs/PERFORMANCE.md）
    #[tokio::test]
    async fn size_comparison_for_record() {
        let backend = backend();
        for size in [1024usize, 8192, 65536] {
            let payload = compressible(size);
            backend
                .set(Arc::from("m"), Arc::new(payload.clone()), None)
                .await
                .unwrap();
            let raw = backend.inner.get("m").await.unwrap().unwrap();
            println!(
                "zstd threshold=256: original={size} stored={} ratio={:.1}%",
                raw.len(),
                raw.len() as f64 / size as f64 * 100.0
            );
            assert!(raw.len() < size, "可压缩负载必须缩小");
            assert_eq!(backend.get("m").await.unwrap(), Some(payload));
        }
    }

    #[tokio::test]
    async fn set_many_and_metadata_passthrough() {
        let backend = backend();
        let items: Vec<CacheSetItem> = vec![
            (Arc::from("a"), Arc::new(compressible(512)), None),
            (Arc::from("b"), Arc::new(b"small".to_vec()), None),
        ];
        backend.set_many(&items).await.unwrap();
        assert_eq!(
            backend.get("a").await.unwrap(),
            Some(compressible(512)),
            "set_many 大值也应压缩并透明解压"
        );
        assert_eq!(backend.get("b").await.unwrap(), Some(b"small".to_vec()));
        assert!(backend.exists("a").await.unwrap());
        assert!(backend.keys("a").await.unwrap().contains(&"a".to_string()));
        assert_eq!(backend.threshold(), 256);
    }
}
