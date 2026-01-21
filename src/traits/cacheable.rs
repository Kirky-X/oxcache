//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cacheable trait for types that can be stored in cache

/// Trait for types that can be stored in and retrieved from cache
///
/// This trait combines serialization and deserialization requirements
/// for cache values. Any type that implements `Serialize` and `DeserializeOwned`
/// automatically implements `Cacheable`.
///
/// # Example
///
/// ```rust,ignore
/// use serde::{Deserialize, Serialize};
/// use oxcache::traits::Cacheable;
///
/// #[derive(Debug, Serialize, Deserialize, PartialEq)]
/// struct User {
///     id: u64,
///     name: String,
/// }
///
/// // User automatically implements Cacheable
/// let user = User {
///     id: 123,
///     name: "Alice".to_string(),
/// };
///
/// // Can be stored in cache
/// // cache.set("user:123", &user).await?;
/// ```
///
/// # Type Bounds
///
/// - `Sized`: The type must have a known size at compile time
/// - `Serialize`: The type must be serializable to bytes
/// - `DeserializeOwned`: The type must be deserializable from owned data
pub trait Cacheable: Sized + serde::Serialize + for<'de> serde::Deserialize<'de> {}

// Blanket implementation for all types that meet the bounds
impl<T> Cacheable for T where T: serde::Serialize + for<'de> serde::Deserialize<'de> {}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestStruct {
        value: i32,
    }

    #[test]
    fn test_cacheable_trait() {
        let instance = TestStruct { value: 42 };
        // This should compile because TestStruct implements Cacheable
        assert!(instance.value == 42);
    }
}
