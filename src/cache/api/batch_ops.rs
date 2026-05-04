//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cache 批量操作方法

use super::Cache;
use crate::core::traits::{CacheKey, Cacheable};
use crate::error::{CacheError, Result};
use std::collections::HashMap;

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: Cacheable,
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
            for (key, value) in key_strings.into_iter().zip(values.into_iter()) {
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
