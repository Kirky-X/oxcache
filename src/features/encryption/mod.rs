// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 值级静态加密装饰器（`encrypt` feature）
//!
//! [`EncryptedBackend`] 对任意 [`CacheBackend`](crate::backend::CacheBackend)
//! 的 value 做**透明加解密**：写入时加密、读取时解密，L2 存储侧不见明文。
//!
//! # 加密口径（与 confers 一致）
//!
//! - 算法：**XChaCha20-Poly1305**（AEAD，24 字节随机 nonce，32 字节 key）；
//! - 信封格式：`[ver: u8][nonce: 24B][ciphertext+tag]`，版本头支撑密钥轮换；
//! - AAD 绑定 cache key：密文不可移植（换键重放解密失败）。
//!
//! # 可组合
//!
//! 与其他装饰器（Bloom 过滤、HMAC 完整性、失效广播等）任意叠放。
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::features::encryption::EncryptedBackend;
//!
//! let backend = EncryptedBackend::new(inner_backend, [7u8; 32]);
//! backend.set("k".into(), Arc::new(b"secret".to_vec()), None).await?;
//! // inner_backend 中为密文；backend.get("k") 解密返回明文
//! ```

#[cfg(feature = "integrity")]
pub mod integrity;

use crate::backend::interface::{BackendKind, CacheSetItem};
use crate::backend::{CacheBackend, CacheConnector, CacheReader, CacheWriter};
use crate::error::{OxCacheError, OxCacheResult};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// 信封版本字节：当前版本 1（支撑密钥轮换的向前兼容）
pub const ENVELOPE_VERSION: u8 = 1;

/// XNonce 长度（24 字节）
const NONCE_SIZE: usize = 24;

/// 密钥长度（32 字节）
const KEY_SIZE: usize = 32;

/// XChaCha20-Poly1305 值加密器（与 confers `XChaCha20Crypto` 口径一致）
#[derive(Clone)]
pub struct ValueCipher {
    key: Arc<[u8; KEY_SIZE]>,
}

impl std::fmt::Debug for ValueCipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 不输出密钥材料
        f.debug_struct("ValueCipher")
            .field("key", &"<redacted 32-byte key>")
            .finish()
    }
}

impl ValueCipher {
    /// 从 32 字节密钥创建加密器
    pub fn new(key: [u8; KEY_SIZE]) -> Self {
        Self {
            key: Arc::new(key),
        }
    }

    /// 从字节切片创建（长度错误返回 Err）
    pub fn from_slice(key: &[u8]) -> OxCacheResult<Self> {
        let arr: [u8; KEY_SIZE] = key.try_into().map_err(|_| {
            OxCacheError::InvalidInput(format!(
                "value cipher key must be exactly {KEY_SIZE} bytes, got {}",
                key.len()
            ))
        })?;
        Ok(Self::new(arr))
    }

    /// 加密：返回 `[ver][nonce][ciphertext+tag]` 信封
    pub fn encrypt(&self, plaintext: &[u8], aad: &[u8]) -> OxCacheResult<Vec<u8>> {
        let cipher = XChaCha20Poly1305::new_from_slice(self.key.as_ref())
            .map_err(|_| OxCacheError::Operation("value cipher: bad key".to_string()))?;

        let mut nonce_bytes = [0u8; NONCE_SIZE];
        // OS 熵源；失败必须显式报错而非降级
        getrandom::fill(&mut nonce_bytes)
            .map_err(|_| OxCacheError::Operation("value cipher: os rng failed".to_string()))?;
        let nonce = XNonce::try_from(&nonce_bytes[..])
            .map_err(|_| OxCacheError::Operation("value cipher: bad nonce".to_string()))?;

        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| OxCacheError::Operation("value cipher: encryption failed".to_string()))?;

        let mut envelope = Vec::with_capacity(1 + NONCE_SIZE + ciphertext.len());
        envelope.push(ENVELOPE_VERSION);
        envelope.extend_from_slice(&nonce_bytes);
        envelope.extend_from_slice(&ciphertext);
        Ok(envelope)
    }

    /// 解密：输入 `[ver][nonce][ciphertext+tag]` 信封
    pub fn decrypt(&self, envelope: &[u8], aad: &[u8]) -> OxCacheResult<Vec<u8>> {
        if envelope.is_empty() || envelope[0] != ENVELOPE_VERSION {
            return Err(OxCacheError::Operation(format!(
                "value cipher: unknown envelope version {:?}",
                envelope.first().copied()
            )));
        }
        if envelope.len() < 1 + NONCE_SIZE {
            return Err(OxCacheError::Operation(
                "value cipher: envelope too short".to_string(),
            ));
        }
        let nonce_bytes: [u8; NONCE_SIZE] = envelope[1..1 + NONCE_SIZE]
            .try_into()
            .map_err(|_| OxCacheError::Operation("value cipher: bad nonce".to_string()))?;
        let ciphertext = &envelope[1 + NONCE_SIZE..];

        let cipher = XChaCha20Poly1305::new_from_slice(self.key.as_ref())
            .map_err(|_| OxCacheError::Operation("value cipher: bad key".to_string()))?;
        let nonce = XNonce::try_from(&nonce_bytes[..])
            .map_err(|_| OxCacheError::Operation("value cipher: bad nonce".to_string()))?;
        cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: ciphertext,
                    aad,
                },
            )
            .map_err(|_| OxCacheError::Operation("value cipher: decryption failed".to_string()))
    }
}

/// 值级加密装饰器：对内层后端的值透明加解密
pub struct EncryptedBackend {
    inner: Arc<dyn CacheBackend>,
    cipher: ValueCipher,
}

impl std::fmt::Debug for EncryptedBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptedBackend")
            .field("inner", &self.inner.backend_kind())
            .field("cipher", &"<redacted>")
            .finish()
    }
}

impl EncryptedBackend {
    /// 包装内层后端并注入 32 字节密钥
    pub fn new(inner: Arc<dyn CacheBackend>, key: [u8; KEY_SIZE]) -> Self {
        Self {
            inner,
            cipher: ValueCipher::new(key),
        }
    }

    /// 从字节切片注入密钥（长度错误在构造期报错）
    pub fn from_slice_key(inner: Arc<dyn CacheBackend>, key: &[u8]) -> OxCacheResult<Self> {
        Ok(Self {
            inner,
            cipher: ValueCipher::from_slice(key)?,
        })
    }

    /// 加密器（诊断/测试）
    pub fn cipher(&self) -> &ValueCipher {
        &self.cipher
    }
}

#[async_trait::async_trait]
impl CacheReader for EncryptedBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        match self.inner.get(key).await? {
            Some(envelope) => {
                // AAD 绑定 cache key：防换键重放
                let plaintext = self.cipher.decrypt(&envelope, key.as_bytes())?;
                Ok(Some(plaintext))
            }
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
impl CacheWriter for EncryptedBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let envelope = self.cipher.encrypt(value.as_slice(), key.as_bytes())?;
        self.inner.set(key, Arc::new(envelope), ttl).await
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
        let mut encrypted = Vec::with_capacity(items.len());
        for (key, value, ttl) in items {
            let envelope = self.cipher.encrypt(value.as_slice(), key.as_bytes())?;
            encrypted.push((key.clone(), Arc::new(envelope), *ttl));
        }
        self.inner.set_many(&encrypted).await
    }

    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        self.inner.delete_many(keys).await
    }
}

#[async_trait::async_trait]
impl CacheConnector for EncryptedBackend {
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

    fn key32(seed: u8) -> [u8; KEY_SIZE] {
        let mut key = [0u8; KEY_SIZE];
        for (i, b) in key.iter_mut().enumerate() {
            *b = seed.wrapping_add(i as u8);
        }
        key
    }

    fn encrypted(seed: u8) -> EncryptedBackend {
        EncryptedBackend::new(Arc::new(MockBackend::new("mock", 100, false)), key32(seed))
    }

    #[tokio::test]
    async fn encrypt_roundtrip_is_transparent() {
        let backend = encrypted(1);
        backend
            .set(Arc::from("user:1"), Arc::new(b"alice-secret".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(
            backend.get("user:1").await.unwrap(),
            Some(b"alice-secret".to_vec()),
            "读路径应透明解密"
        );
    }

    #[tokio::test]
    async fn raw_storage_never_contains_plaintext() {
        let backend = encrypted(1);
        backend
            .set(Arc::from("user:1"), Arc::new(b"plaintext-value".to_vec()), None)
            .await
            .unwrap();

        // 直接读内层原始字节：必须是密文信封
        let raw = backend.inner.get("user:1").await.unwrap().unwrap();
        assert_ne!(raw, b"plaintext-value".to_vec(), "存储层不得出现明文");
        assert_eq!(raw[0], ENVELOPE_VERSION, "信封首字节应为版本号");
        assert_eq!(raw.len(), 1 + NONCE_SIZE + 15 + 16, "ver+nonce+明文长度+tag");
    }

    #[tokio::test]
    async fn wrong_key_fails_decryption() {
        let backend = encrypted(1);
        backend
            .set(Arc::from("k"), Arc::new(b"value".to_vec()), None)
            .await
            .unwrap();

        let wrong = EncryptedBackend::new(backend.inner.clone(), key32(200)); // 不同密钥
        let err = wrong.get("k").await.expect_err("错误密钥必须解密失败");
        assert!(
            matches!(err, OxCacheError::Operation(_)),
            "解密失败应映射为 Operation 错误，got {err:?}"
        );
    }

    #[tokio::test]
    async fn aad_binds_ciphertext_to_key_name() {
        // 同一密钥下，把 key A 的密文移植到 key B：解密必须失败（AAD 绑定）
        let backend = encrypted(1);
        backend
            .set(Arc::from("aaa"), Arc::new(b"payload".to_vec()), None)
            .await
            .unwrap();

        let envelope = backend.inner.get("aaa").await.unwrap().unwrap();
        backend
            .inner
            .set(Arc::from("bbb"), Arc::new(envelope), None)
            .await
            .unwrap();

        let err = backend.get("bbb").await.expect_err("移植密文必须解密失败");
        assert!(matches!(err, OxCacheError::Operation(_)));
    }

    #[tokio::test]
    async fn decorators_compose() {
        // 双层加密叠加：证明装饰器可组合（任意叠放）
        let inner: Arc<dyn CacheBackend> = Arc::new(MockBackend::new("mock", 100, false));
        let outer = EncryptedBackend::new(Arc::new(encrypted(3)), key32(9));
        let _ = inner;

        outer
            .set(Arc::from("k"), Arc::new(b"twice-wrapped".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(
            outer.get("k").await.unwrap(),
            Some(b"twice-wrapped".to_vec())
        );
    }

    #[tokio::test]
    async fn key_length_validated_at_construction() {
        let inner: Arc<dyn CacheBackend> = Arc::new(MockBackend::new("mock", 100, false));
        let err = EncryptedBackend::from_slice_key(inner, b"short-key")
            .expect_err("密钥长度错误必须在构造期报错");
        assert!(matches!(err, OxCacheError::InvalidInput(_)));
    }

    #[test]
    fn cipher_rejects_non_utf8_envelope_version() {
        let cipher = ValueCipher::new(key32(1));
        let err = cipher
            .decrypt(&[0xFF, 0, 1, 2], b"aad")
            .expect_err("未知版本必须报错");
        assert!(err.to_string().contains("envelope version"));
    }

    #[tokio::test]
    async fn read_path_passthrough_still_works() {
        let backend = encrypted(1);
        assert_eq!(backend.get("missing").await.unwrap(), None);
        assert!(!backend.exists("missing").await.unwrap());
        assert_eq!(backend.backend_kind(), BackendKind::Mock);
        backend.health_check().await.unwrap();
    }
}
