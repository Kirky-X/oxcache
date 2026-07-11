// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Depth-limited JSON deserialization utilities
//
// Provides safe JSON deserialization with depth limits to prevent
// stack overflow attacks and excessive memory usage.

use serde::Deserialize;
use std::fmt;

/// Maximum allowed nesting depth for JSON deserialization
pub const MAX_DESERIALIZE_DEPTH: usize = 32;

/// Error returned when JSON nesting depth is exceeded
#[derive(Debug, PartialEq)]
pub struct DepthLimitExceededError {
    pub depth: usize,
    pub max_depth: usize,
}

impl std::fmt::Display for DepthLimitExceededError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "JSON nesting depth {} exceeds maximum allowed depth {}",
            self.depth, self.max_depth
        )
    }
}

impl std::error::Error for DepthLimitExceededError {}

/// Calculates the nesting depth of a JSON value
/// - Empty container: 1
/// - Container with primitives only: 1
/// - Container with nested containers: 1 + max child depth
fn calculate_depth(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Object(map) => {
            if map.is_empty() {
                1
            } else {
                let child_depths: Vec<usize> = map.values().map(calculate_depth).collect();
                let max_child_depth = child_depths.iter().max().unwrap_or(&1);
                // Check if any child is a container (not just a primitive)
                let has_container_child = map
                    .values()
                    .any(|v| matches!(v, serde_json::Value::Object(_) | serde_json::Value::Array(_)));
                if has_container_child {
                    max_child_depth + 1
                } else {
                    1
                }
            }
        }
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                1
            } else {
                let child_depths: Vec<usize> = arr.iter().map(calculate_depth).collect();
                let max_child_depth = child_depths.iter().max().unwrap_or(&1);
                // Check if any child is a container (not just a primitive)
                let has_container_child = arr
                    .iter()
                    .any(|v| matches!(v, serde_json::Value::Object(_) | serde_json::Value::Array(_)));
                if has_container_child {
                    max_child_depth + 1
                } else {
                    1
                }
            }
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => 1,
    }
}

/// Checks if JSON data exceeds the maximum depth limit
///
/// # Arguments
///
/// * `data` - JSON bytes to check
/// * `max_depth` - Maximum allowed nesting depth
///
/// # Returns
///
/// * `Ok(bool)` - true if depth limit would be exceeded
/// * `Err(serde_json::Error)` - JSON parsing failed
pub fn would_exceed_depth_limit(data: &[u8], max_depth: usize) -> Result<bool, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_slice(data)?;
    let depth = calculate_depth(&value);
    Ok(depth > max_depth)
}

/// A wrapper type that limits deserialization depth for serde_json::Value
#[derive(Debug, Clone)]
pub struct DepthLimited {
    pub value: serde_json::Value,
    max_depth: usize,
}

impl DepthLimited {
    /// Creates a new DepthLimited from JSON bytes with depth validation
    pub fn from_slice(data: &[u8], max_depth: usize) -> Result<Self, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_slice(data)?;
        let depth = calculate_depth(&value);
        if depth > max_depth {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("JSON depth {} exceeds maximum allowed depth {}", depth, max_depth),
            )));
        }
        Ok(DepthLimited { value, max_depth })
    }

    /// Returns the inner value
    pub fn into_inner(self) -> serde_json::Value {
        self.value
    }

    /// Returns the maximum depth that was used
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }
}

impl<'de> Deserialize<'de> for DepthLimited {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde::Deserialize::deserialize(deserializer)?;
        let depth = calculate_depth(&value);
        if depth > MAX_DESERIALIZE_DEPTH {
            return Err(serde::de::Error::custom(format!(
                "JSON depth {} exceeds maximum allowed depth {}",
                depth, MAX_DESERIALIZE_DEPTH
            )));
        }
        Ok(DepthLimited {
            value,
            max_depth: MAX_DESERIALIZE_DEPTH,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_would_exceed_depth_limit_false() {
        let data = br#"{"a": 1}"#;
        let result = would_exceed_depth_limit(data, 10);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_would_exceed_depth_limit_true() {
        let data = br#"{"a": {"b": {"c": "value"}}}"#;
        let result = would_exceed_depth_limit(data, 2);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_calculate_depth_primitives() {
        assert_eq!(calculate_depth(&serde_json::json!(1)), 1);
        assert_eq!(calculate_depth(&serde_json::json!("test")), 1);
        assert_eq!(calculate_depth(&serde_json::json!(true)), 1);
        assert_eq!(calculate_depth(&serde_json::json!(null)), 1);
    }

    #[test]
    fn test_calculate_depth_objects() {
        assert_eq!(calculate_depth(&serde_json::json!({})), 1);
        let nested = serde_json::json!({"a": 1});
        assert_eq!(calculate_depth(&nested), 1);
    }

    #[test]
    fn test_calculate_depth_arrays() {
        assert_eq!(calculate_depth(&serde_json::json!([])), 1);
        assert_eq!(calculate_depth(&serde_json::json!([1, 2, 3])), 1);
        assert_eq!(calculate_depth(&serde_json::json!([[1]])), 2);
    }

    #[test]
    fn test_depth_limit_exceeded_error() {
        let error = DepthLimitExceededError {
            depth: 100,
            max_depth: 32,
        };
        assert_eq!(
            error.to_string(),
            "JSON nesting depth 100 exceeds maximum allowed depth 32"
        );
    }

    #[test]
    fn test_depth_limited_from_slice() {
        let data = br#"{"a": {"b": "value"}}"#;
        let result = DepthLimited::from_slice(data, 3);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().value, serde_json::json!({"a": {"b": "value"}}));
    }

    #[test]
    fn test_depth_limited_exceeded() {
        let data = br#"{"a": {"b": {"c": {"d": "value"}}}}"#;
        let result = DepthLimited::from_slice(data, 3);
        assert!(result.is_err());
    }

    // ============================================================================
    // into_inner() 方法测试 (lines 121-122)
    // ============================================================================

    #[test]
    fn test_depth_limited_into_inner() {
        let data = br#"{"key": "value"}"#;
        let limited = DepthLimited::from_slice(data, 10).unwrap();
        let value = limited.into_inner();
        assert_eq!(value, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn test_depth_limited_into_inner_array() {
        let data = br#"[1, 2, 3]"#;
        let limited = DepthLimited::from_slice(data, 10).unwrap();
        let value = limited.into_inner();
        assert_eq!(value, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn test_depth_limited_into_inner_primitive() {
        let data = br#""hello""#;
        let limited = DepthLimited::from_slice(data, 10).unwrap();
        let value = limited.into_inner();
        assert_eq!(value, serde_json::json!("hello"));
    }

    // ============================================================================
    // max_depth() 方法测试 (lines 126-127)
    // ============================================================================

    #[test]
    fn test_depth_limited_max_depth() {
        let data = br#"{"key": "value"}"#;
        let limited = DepthLimited::from_slice(data, 10).unwrap();
        assert_eq!(limited.max_depth(), 10);
    }

    #[test]
    fn test_depth_limited_max_depth_custom() {
        let data = br#"{"key": "value"}"#;
        let limited = DepthLimited::from_slice(data, 5).unwrap();
        assert_eq!(limited.max_depth(), 5);
    }

    // ============================================================================
    // Deserialize trait 测试 (lines 136-146)
    // ============================================================================

    #[test]
    fn test_depth_limited_deserialize_valid() {
        let data = br#"{"key": "value"}"#;
        let value: serde_json::Value = serde_json::from_slice(data).unwrap();

        // 使用 serde_json::Deserializer 来测试 Deserialize impl
        let json_str = serde_json::to_string(&value).unwrap();
        let mut deserializer = serde_json::Deserializer::from_str(&json_str);
        let limited: DepthLimited = DepthLimited::deserialize(&mut deserializer).unwrap();
        assert_eq!(limited.value, value);
        assert_eq!(limited.max_depth(), MAX_DESERIALIZE_DEPTH);
    }

    #[test]
    fn test_depth_limited_deserialize_simple() {
        let json_str = r#"{"a": 1}"#;
        let mut deserializer = serde_json::Deserializer::from_str(json_str);
        let limited: DepthLimited = DepthLimited::deserialize(&mut deserializer).unwrap();
        assert_eq!(limited.value, serde_json::json!({"a": 1}));
        assert_eq!(limited.max_depth(), MAX_DESERIALIZE_DEPTH);
    }

    #[test]
    fn test_depth_limited_deserialize_array() {
        let json_str = r#"[1, 2, 3]"#;
        let mut deserializer = serde_json::Deserializer::from_str(json_str);
        let limited: DepthLimited = DepthLimited::deserialize(&mut deserializer).unwrap();
        assert_eq!(limited.value, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn test_depth_limited_deserialize_exceeds_depth() {
        // 创建一个超过 MAX_DESERIALIZE_DEPTH 的嵌套 JSON
        let mut json_str = String::new();
        for _ in 0..(MAX_DESERIALIZE_DEPTH + 5) {
            json_str.push_str("{\"a\":");
        }
        json_str.push('1');
        for _ in 0..(MAX_DESERIALIZE_DEPTH + 5) {
            json_str.push('}');
        }

        let mut deserializer = serde_json::Deserializer::from_str(&json_str);
        let result: Result<DepthLimited, _> = DepthLimited::deserialize(&mut deserializer);
        assert!(result.is_err());
    }

    // ============================================================================
    // calculate_depth 边界测试
    // ============================================================================

    #[test]
    fn test_calculate_depth_deeply_nested_object() {
        let value = serde_json::json!({"a": {"b": {"c": {"d": "value"}}}});
        assert_eq!(calculate_depth(&value), 4);
    }

    #[test]
    fn test_calculate_depth_deeply_nested_array() {
        let value = serde_json::json!([[[["value"]]]]);
        assert_eq!(calculate_depth(&value), 4);
    }

    #[test]
    fn test_calculate_depth_mixed_nested() {
        let value = serde_json::json!({"a": [{"b": [{"c": 1}]}]});
        assert_eq!(calculate_depth(&value), 5);
    }

    #[test]
    fn test_calculate_depth_object_with_array_of_primitives() {
        let value = serde_json::json!({"a": [1, 2, 3]});
        assert_eq!(calculate_depth(&value), 2);
    }

    #[test]
    fn test_calculate_depth_array_with_objects_of_primitives() {
        let value = serde_json::json!([{"a": 1}, {"b": 2}]);
        assert_eq!(calculate_depth(&value), 2);
    }

    // ============================================================================
    // would_exceed_depth_limit 边界测试
    // ============================================================================

    #[test]
    fn test_would_exceed_depth_limit_invalid_json() {
        let data = b"invalid json";
        let result = would_exceed_depth_limit(data, 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_would_exceed_depth_limit_empty_object() {
        let data = b"{}";
        let result = would_exceed_depth_limit(data, 1);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_would_exceed_depth_limit_exact_depth() {
        let data = br#"{"a": {"b": "value"}}"#;
        // depth is 2, max_depth is 2, so 2 > 2 is false
        let result = would_exceed_depth_limit(data, 2);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_would_exceed_depth_limit_one_over() {
        let data = br#"{"a": {"b": "value"}}"#;
        // depth is 2, max_depth is 1, so 2 > 1 is true
        let result = would_exceed_depth_limit(data, 1);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    // ============================================================================
    // DepthLimited from_slice 边界测试
    // ============================================================================

    #[test]
    fn test_depth_limited_from_slice_invalid_json() {
        let data = b"invalid json";
        let result = DepthLimited::from_slice(data, 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_depth_limited_from_slice_empty_object() {
        let data = b"{}";
        let result = DepthLimited::from_slice(data, 1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().value, serde_json::json!({}));
    }

    #[test]
    fn test_depth_limited_from_slice_empty_array() {
        let data = b"[]";
        let result = DepthLimited::from_slice(data, 1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().value, serde_json::json!([]));
    }

    #[test]
    fn test_depth_limited_from_slice_null() {
        let data = b"null";
        let result = DepthLimited::from_slice(data, 1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().value, serde_json::Value::Null);
    }

    #[test]
    fn test_depth_limited_from_slice_debug() {
        let data = br#"{"key": "value"}"#;
        let limited = DepthLimited::from_slice(data, 10).unwrap();
        let debug_str = format!("{:?}", limited);
        assert!(debug_str.contains("DepthLimited"));
    }

    #[test]
    fn test_depth_limited_clone() {
        let data = br#"{"key": "value"}"#;
        let limited = DepthLimited::from_slice(data, 10).unwrap();
        let cloned = limited.clone();
        assert_eq!(limited.value, cloned.value);
        assert_eq!(limited.max_depth, cloned.max_depth);
    }

    // ============================================================================
    // DepthLimitExceededError 测试
    // ============================================================================

    #[test]
    fn test_depth_limit_exceeded_error_equality() {
        let err1 = DepthLimitExceededError {
            depth: 10,
            max_depth: 5,
        };
        let err2 = DepthLimitExceededError {
            depth: 10,
            max_depth: 5,
        };
        let err3 = DepthLimitExceededError {
            depth: 10,
            max_depth: 6,
        };
        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    #[test]
    fn test_depth_limit_exceeded_error_debug() {
        let err = DepthLimitExceededError {
            depth: 10,
            max_depth: 5,
        };
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("DepthLimitExceededError"));
        assert!(debug_str.contains("10"));
        assert!(debug_str.contains("5"));
    }

    #[test]
    fn test_depth_limit_exceeded_error_is_std_error() {
        let err = DepthLimitExceededError {
            depth: 10,
            max_depth: 5,
        };
        let _: &dyn std::error::Error = &err;
    }
}
