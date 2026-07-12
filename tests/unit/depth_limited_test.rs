// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// 深度限制序列化单元测试

use oxcache::infra::serialization::{
    would_exceed_depth_limit, DepthLimitExceededError, DepthLimited, MAX_DESERIALIZE_DEPTH,
};
use serde_json::json;

#[test]
fn test_would_exceed_depth_limit_within_limit() {
    let data = br#"{"a": 1}"#;
    let result = would_exceed_depth_limit(data, 10);
    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[test]
fn test_would_exceed_depth_limit_exceeds() {
    let data = br#"{"a": {"b": {"c": {"d": 1}}}}"#;
    let result = would_exceed_depth_limit(data, 2);
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[test]
fn test_would_exceed_depth_limit_invalid_json() {
    let data = br#"invalid json"#;
    let result = would_exceed_depth_limit(data, 10);
    assert!(result.is_err());
}

#[test]
fn test_depth_limited_from_slice_valid() {
    let data = br#"{"a": {"b": "value"}}"#;
    let result = DepthLimited::from_slice(data, 10);
    assert!(result.is_ok());
    let limited = result.unwrap();
    assert_eq!(limited.value, json!({"a": {"b": "value"}}));
}

#[test]
fn test_depth_limited_from_slice_exceeds() {
    let data = br#"{"a": {"b": {"c": {"d": {"e": 1}}}}}"#;
    let result = DepthLimited::from_slice(data, 3);
    assert!(result.is_err());
}

#[test]
fn test_depth_limited_into_inner() {
    let data = br#"{"test": "value"}"#;
    let limited = DepthLimited::from_slice(data, 10).unwrap();
    let value = limited.into_inner();
    assert_eq!(value, json!({"test": "value"}));
}

#[test]
fn test_depth_limited_max_depth() {
    let data = br#"{"a": 1}"#;
    let limited = DepthLimited::from_slice(data, 5).unwrap();
    assert_eq!(limited.max_depth(), 5);
}

#[test]
fn test_depth_limit_exceeded_error_display() {
    let error = DepthLimitExceededError {
        depth: 100,
        max_depth: 32,
    };
    let msg = error.to_string();
    assert!(msg.contains("100"));
    assert!(msg.contains("32"));
    assert!(msg.contains("exceeds"));
}

#[test]
fn test_max_deserialize_depth_constant() {
    assert_eq!(MAX_DESERIALIZE_DEPTH, 32);
}

#[test]
fn test_depth_limited_with_array() {
    let data = br#"[1, 2, [3, 4, [5]]]"#;
    let result = DepthLimited::from_slice(data, 5);
    assert!(result.is_ok());
}

#[test]
fn test_depth_limited_with_null() {
    let data = br#"null"#;
    let result = DepthLimited::from_slice(data, 10);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, json!(null));
}

#[test]
fn test_depth_limited_with_boolean() {
    let data = br#"true"#;
    let result = DepthLimited::from_slice(data, 10);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, json!(true));
}

#[test]
fn test_depth_limited_with_number() {
    let data = br#"42"#;
    let result = DepthLimited::from_slice(data, 10);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, json!(42));
}

#[test]
fn test_depth_limited_with_string() {
    let data = br#""hello world""#;
    let result = DepthLimited::from_slice(data, 10);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, json!("hello world"));
}

#[test]
fn test_depth_limited_empty_object() {
    let data = br#"{}"#;
    let result = DepthLimited::from_slice(data, 10);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, json!({}));
}

#[test]
fn test_depth_limited_empty_array() {
    let data = br#"[]"#;
    let result = DepthLimited::from_slice(data, 10);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, json!([]));
}

#[test]
fn test_depth_limited_complex_nested() {
    let data = br#"{
        "user": {
            "id": 123,
            "profile": {
                "name": "test",
                "settings": {
                    "theme": "dark",
                    "notifications": {
                        "email": true
                    }
                }
            }
        }
    }"#;
    let result = DepthLimited::from_slice(data, 10);
    assert!(result.is_ok());
}

#[test]
fn test_depth_limited_exactly_at_limit() {
    let data = br#"{"a": {"b": 1}}"#;
    let result = DepthLimited::from_slice(data, 2);
    assert!(result.is_ok());
}

#[test]
fn test_depth_limited_just_over_limit() {
    let data = br#"{"a": {"b": {"c": 1}}}"#;
    let result = DepthLimited::from_slice(data, 2);
    assert!(result.is_err());
}

#[test]
fn test_would_exceed_depth_flat_json() {
    let data = br#"{"name": "test", "value": 123}"#;
    let result = would_exceed_depth_limit(data, 1);
    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[test]
fn test_would_exceed_depth_nested_array() {
    let data = br#"[[[1]]]"#;
    let result = would_exceed_depth_limit(data, 2);
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[test]
fn test_depth_limited_clone() {
    let data = br#"{"key": "value"}"#;
    let limited = DepthLimited::from_slice(data, 10).unwrap();
    let cloned = limited.clone();
    assert_eq!(limited.value, cloned.value);
}

#[test]
fn test_depth_limited_debug() {
    let data = br#"{"key": "value"}"#;
    let limited = DepthLimited::from_slice(data, 10).unwrap();
    let debug_str = format!("{:?}", limited);
    assert!(debug_str.contains("DepthLimited"));
}
