//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! CacheKey trait for converting types to cache key strings

/// Trait for types that can be used as cache keys
///
/// This trait provides a way to convert any type into a string representation
/// suitable for use as a cache key. Blanket implementations are provided for
/// common types like String, &str, and numeric types.
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::traits::CacheKey;
///
/// // Using blanket implementation for String
/// let key: String = "user:123".to_string();
/// assert_eq!(key.to_key_string(), "user:123");
///
/// // Using blanket implementation for u64
/// let id: u64 = 12345;
/// assert_eq!(id.to_key_string(), "12345");
///
/// // Custom implementation
/// struct UserId(u64);
///
/// impl CacheKey for UserId {
///     fn to_key_string(&self) -> String {
///         format!("user:{}", self.0)
///     }
/// }
///
/// let user_id = UserId(12345);
/// assert_eq!(user_id.to_key_string(), "user:12345");
/// ```
pub trait CacheKey: Send + Sync {
    /// Convert this key to a string representation
    ///
    /// This method should return a unique string representation that can be
    /// used as a cache key. The implementation should be deterministic and
    /// produce the same output for equal values.
    ///
    /// # Returns
    ///
    /// A string representation of this key
    fn to_key_string(&self) -> String;
}

// Blanket implementations for common types

impl CacheKey for String {
    fn to_key_string(&self) -> String {
        self.clone()
    }
}

impl CacheKey for &str {
    fn to_key_string(&self) -> String {
        (*self).to_string()
    }
}

impl CacheKey for u64 {
    fn to_key_string(&self) -> String {
        self.to_string()
    }
}

impl CacheKey for u128 {
    fn to_key_string(&self) -> String {
        self.to_string()
    }
}

impl CacheKey for i64 {
    fn to_key_string(&self) -> String {
        self.to_string()
    }
}

impl CacheKey for i32 {
    fn to_key_string(&self) -> String {
        self.to_string()
    }
}

impl CacheKey for u32 {
    fn to_key_string(&self) -> String {
        self.to_string()
    }
}

impl CacheKey for usize {
    fn to_key_string(&self) -> String {
        self.to_string()
    }
}

impl CacheKey for isize {
    fn to_key_string(&self) -> String {
        self.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_key() {
        let key = "user:123".to_string();
        assert_eq!(key.to_key_string(), "user:123");
    }

    #[test]
    fn test_str_key() {
        let key: &str = "user:123";
        assert_eq!(key.to_key_string(), "user:123");
    }

    #[test]
    fn test_u64_key() {
        let key: u64 = 12345;
        assert_eq!(key.to_key_string(), "12345");
    }

    #[test]
    fn test_i64_key() {
        let key: i64 = -12345;
        assert_eq!(key.to_key_string(), "-12345");
    }

    #[test]
    fn test_u128_key() {
        let key: u128 = 12345678901234567890;
        assert_eq!(key.to_key_string(), "12345678901234567890");
    }
}
