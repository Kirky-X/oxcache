//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! HTTP 缓存适配器 trait

use std::collections::HashMap;

use crate::features::http::HttpCacheResponse;

/// HTTP 缓存适配器
///
/// 提供 HTTP 响应缓存的底层存储接口。
#[async_trait::async_trait]
pub trait HttpCacheAdapter: Send + Sync {
    /// 获取缓存的响应
    async fn get_response(&self, key: &str) -> Result<Option<HttpCacheResponse>, crate::error::CacheError>;

    /// 设置缓存响应
    async fn set_response(&self, key: &str, response: &HttpCacheResponse) -> Result<(), crate::error::CacheError>;

    /// 删除缓存的响应
    async fn delete_response(&self, key: &str) -> Result<bool, crate::error::CacheError>;

    /// 按模式失效
    async fn invalidate_by_pattern(&self, pattern: &str) -> Result<u64, crate::error::CacheError>;

    /// 按路径模式失效
    async fn invalidate_by_path_pattern(&self, path_pattern: &str) -> Result<u64, crate::error::CacheError> {
        self.invalidate_by_pattern(path_pattern).await
    }

    /// 批量获取
    async fn get_responses(
        &self,
        keys: &[&str],
    ) -> Result<HashMap<String, HttpCacheResponse>, crate::error::CacheError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// In-memory adapter for testing
    #[derive(Clone, Debug)]
    struct MockAdapter {
        store: Arc<tokio::sync::Mutex<HashMap<String, HttpCacheResponse>>>,
    }

    impl MockAdapter {
        fn new() -> Self {
            Self {
                store: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl HttpCacheAdapter for MockAdapter {
        async fn get_response(&self, key: &str) -> Result<Option<HttpCacheResponse>, crate::error::CacheError> {
            let store = self.store.lock().await;
            Ok(store.get(key).cloned())
        }

        async fn set_response(&self, key: &str, response: &HttpCacheResponse) -> Result<(), crate::error::CacheError> {
            let mut store = self.store.lock().await;
            store.insert(key.to_string(), response.clone());
            Ok(())
        }

        async fn delete_response(&self, key: &str) -> Result<bool, crate::error::CacheError> {
            let mut store = self.store.lock().await;
            Ok(store.remove(key).is_some())
        }

        async fn invalidate_by_pattern(&self, _pattern: &str) -> Result<u64, crate::error::CacheError> {
            let mut store = self.store.lock().await;
            let count = store.len();
            store.clear();
            Ok(count as u64)
        }

        async fn invalidate_by_path_pattern(&self, _path_pattern: &str) -> Result<u64, crate::error::CacheError> {
            self.invalidate_by_pattern(_path_pattern).await
        }

        async fn get_responses(
            &self,
            keys: &[&str],
        ) -> Result<HashMap<String, HttpCacheResponse>, crate::error::CacheError> {
            let store = self.store.lock().await;
            let mut result = HashMap::new();
            for &key in keys {
                if let Some(resp) = store.get(key) {
                    result.insert(key.to_string(), resp.clone());
                }
            }
            Ok(result)
        }
    }

    fn make_response(status: u16, body: Vec<u8>) -> HttpCacheResponse {
        HttpCacheResponse {
            status,
            headers: HashMap::new(),
            body,
            cached_at: chrono::Utc::now(),
            ttl: Some(3600),
            etag: Some("\"abc123\"".to_string()),
            last_modified: Some("Mon, 01 Jan 2024 00:00:00 GMT".to_string()),
        }
    }

    #[tokio::test]
    async fn test_get_response_returns_none_for_missing_key() {
        let adapter = MockAdapter::new();
        let result = adapter.get_response("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_set_and_get_response() {
        let adapter = MockAdapter::new();
        let response = make_response(200, b"hello".to_vec());

        adapter.set_response("key1", &response).await.unwrap();
        let cached = adapter.get_response("key1").await.unwrap().unwrap();

        assert_eq!(cached.status, 200);
        assert_eq!(cached.body, b"hello");
        assert_eq!(cached.etag, Some("\"abc123\"".to_string()));
    }

    #[tokio::test]
    async fn test_delete_response_existing_key() {
        let adapter = MockAdapter::new();
        let response = make_response(200, b"data".to_vec());
        adapter.set_response("key1", &response).await.unwrap();

        let deleted = adapter.delete_response("key1").await.unwrap();
        assert!(deleted);

        let result = adapter.get_response("key1").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_response_missing_key() {
        let adapter = MockAdapter::new();
        let deleted = adapter.delete_response("nonexistent").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_invalidate_by_pattern_clears_all() {
        let adapter = MockAdapter::new();
        let response = make_response(200, b"data".to_vec());
        adapter.set_response("key1", &response).await.unwrap();
        adapter.set_response("key2", &response).await.unwrap();

        let count = adapter.invalidate_by_pattern("/api/*").await.unwrap();
        assert_eq!(count, 2);

        assert!(adapter.get_response("key1").await.unwrap().is_none());
        assert!(adapter.get_response("key2").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_invalidate_by_pattern_empty_store() {
        let adapter = MockAdapter::new();
        let count = adapter.invalidate_by_pattern("/api/*").await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_invalidate_by_path_pattern_delegates() {
        let adapter = MockAdapter::new();
        let response = make_response(200, b"data".to_vec());
        adapter.set_response("key1", &response).await.unwrap();

        let count = adapter.invalidate_by_path_pattern("/api/*").await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_get_responses_batch() {
        let adapter = MockAdapter::new();
        let resp1 = make_response(200, b"data1".to_vec());
        let resp2 = make_response(201, b"data2".to_vec());
        adapter.set_response("k1", &resp1).await.unwrap();
        adapter.set_response("k2", &resp2).await.unwrap();

        let results = adapter.get_responses(&["k1", "k2", "k3"]).await.unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.contains_key("k1"));
        assert!(results.contains_key("k2"));
        assert!(!results.contains_key("k3"));
    }

    #[tokio::test]
    async fn test_get_responses_empty_keys() {
        let adapter = MockAdapter::new();
        let results = adapter.get_responses(&[]).await.unwrap();
        assert!(results.is_empty());
    }
}
