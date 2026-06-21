//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cache 批量操作方法

use super::Cache;
use crate::core::traits::CacheKey;
use crate::error::{CacheError, Result};
use std::collections::HashMap;

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    pub async fn set_many<'a, I>(&self, items: I) -> Result<()>
    where
        K: 'a,
        V: 'a,
        I: IntoIterator<Item = (&'a K, &'a V)>,
    {
        #[cfg(any(feature = "serialization", feature = "full"))]
        {
            let mut batch_items = Vec::new();
            for (key, value) in items {
                let key_str = key.to_key_string();
                let bytes = match serde_json::to_vec(value) {
                    Ok(b) => b,
                    Err(e) => return Err(CacheError::Serialization(e.to_string())),
                };
                batch_items.push((key_str, bytes, None));
            }
            self.backend.set_many(&batch_items).await
        }

        #[cfg(not(any(feature = "serialization", feature = "full")))]
        {
            let _ = items;
            Err(CacheError::Serialization(
                "Serialization feature is required for typed set_many operations".to_string(),
            ))
        }
    }

    pub async fn get_many<'a, I>(&self, keys: I) -> Result<HashMap<String, V>>
    where
        K: 'a,
        I: IntoIterator<Item = &'a K>,
    {
        #[cfg(any(feature = "serialization", feature = "full"))]
        {
            let key_strings: Vec<String> = keys.into_iter().map(|k| k.to_key_string()).collect();
            let values = self.backend.get_many(&key_strings).await?;

            let mut result = HashMap::new();
            for (key, value) in key_strings.into_iter().zip(values) {
                if let Some(bytes) = value {
                    if let Ok(decoded) = serde_json::from_slice::<V>(&bytes) {
                        result.insert(key, decoded);
                    }
                }
            }

            Ok(result)
        }

        #[cfg(not(any(feature = "serialization", feature = "full")))]
        {
            let _ = keys;
            Err(CacheError::Serialization(
                "Serialization feature is required for typed get_many operations".to_string(),
            ))
        }
    }

    pub async fn delete_many<'a, I>(&self, keys: I) -> Result<()>
    where
        K: 'a,
        I: IntoIterator<Item = &'a K>,
    {
        let key_strings: Vec<String> = keys.into_iter().map(|k| k.to_key_string()).collect();
        self.backend.delete_many(&key_strings).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_many_basic() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        let k1 = "key1".to_string();
        let k2 = "key2".to_string();
        let k3 = "key3".to_string();
        let v1 = "value1".to_string();
        let v2 = "value2".to_string();
        let v3 = "value3".to_string();

        let items: Vec<(&String, &String)> = vec![(&k1, &v1), (&k2, &v2), (&k3, &v3)];

        cache.set_many(items).await.unwrap();

        assert_eq!(cache.get(&k1).await.unwrap(), Some(v1.clone()));
        assert_eq!(cache.get(&k2).await.unwrap(), Some(v2.clone()));
        assert_eq!(cache.get(&k3).await.unwrap(), Some(v3.clone()));
    }

    #[tokio::test]
    async fn test_get_many_basic() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        cache.set(&"key1".to_string(), &"value1".to_string()).await.unwrap();
        cache.set(&"key2".to_string(), &"value2".to_string()).await.unwrap();
        cache.set(&"key3".to_string(), &"value3".to_string()).await.unwrap();

        let k1 = "key1".to_string();
        let k2 = "key2".to_string();
        let k3 = "missing".to_string();
        let keys: Vec<&String> = vec![&k1, &k2, &k3];

        let result = cache.get_many(keys).await.unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result.get("key1"), Some(&"value1".to_string()));
        assert_eq!(result.get("key2"), Some(&"value2".to_string()));
        assert!(result.get("missing").is_none());
    }

    #[tokio::test]
    async fn test_delete_many_basic() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        cache.set(&"key1".to_string(), &"value1".to_string()).await.unwrap();
        cache.set(&"key2".to_string(), &"value2".to_string()).await.unwrap();
        cache.set(&"key3".to_string(), &"value3".to_string()).await.unwrap();

        let k1 = "key1".to_string();
        let k2 = "key2".to_string();
        let keys: Vec<&String> = vec![&k1, &k2];
        cache.delete_many(keys).await.unwrap();

        assert!(cache.get(&"key1".to_string()).await.unwrap().is_none());
        assert!(cache.get(&"key2".to_string()).await.unwrap().is_none());
        assert_eq!(
            cache.get(&"key3".to_string()).await.unwrap(),
            Some("value3".to_string())
        );
    }

    #[tokio::test]
    async fn test_set_many_empty() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        let items: Vec<(&String, &String)> = vec![];
        cache.set_many(items).await.unwrap();
    }

    #[tokio::test]
    async fn test_get_many_empty_keys() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        let keys: Vec<&String> = vec![];
        let result = cache.get_many(keys).await.unwrap();

        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_delete_many_empty_keys() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        let keys: Vec<&String> = vec![];
        cache.delete_many(keys).await.unwrap();
    }

    #[tokio::test]
    async fn test_get_many_all_missing() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        let k1 = "missing1".to_string();
        let k2 = "missing2".to_string();
        let keys: Vec<&String> = vec![&k1, &k2];
        let result = cache.get_many(keys).await.unwrap();

        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_batch_ops_with_integers() {
        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();

        let k1 = "num1".to_string();
        let k2 = "num2".to_string();
        let k3 = "num3".to_string();
        let items: Vec<(&String, &i32)> = vec![(&k1, &10), (&k2, &20), (&k3, &30)];

        cache.set_many(items).await.unwrap();

        let keys: Vec<&String> = vec![&k1, &k2];
        let result = cache.get_many(keys).await.unwrap();

        assert_eq!(result.get("num1"), Some(&10));
        assert_eq!(result.get("num2"), Some(&20));
    }

    #[tokio::test]
    async fn test_batch_ops_with_struct() {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
        struct User {
            id: u64,
            name: String,
        }

        let cache: Cache<String, User> = Cache::builder().build().await.unwrap();

        let user1 = User {
            id: 1,
            name: "Alice".to_string(),
        };
        let user2 = User {
            id: 2,
            name: "Bob".to_string(),
        };

        let uk1 = "user:1".to_string();
        let uk2 = "user:2".to_string();
        let items: Vec<(&String, &User)> = vec![(&uk1, &user1), (&uk2, &user2)];

        cache.set_many(items).await.unwrap();

        let keys: Vec<&String> = vec![&uk1, &uk2];
        let result = cache.get_many(keys).await.unwrap();

        assert_eq!(result.get("user:1"), Some(&user1));
        assert_eq!(result.get("user:2"), Some(&user2));
    }
}
