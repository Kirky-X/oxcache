// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! canonical JSON 键归一化
//!
//! 提供键序无关的确定性 JSON 序列化：对象键递归字典序重排、数组保序、
//! 标量原样。用于"同一 JSON 语义必然生成同一缓存键"的场景（如响应体
//! 缓存键、完整性摘要的输入归一化）。
//!
//! 归一化只消除键序歧义；数值与字符串的字节形态由 `serde_json` 自身
//! 序列化规则决定（Number 经 ryu 输出，本身稳定）。
//!
//! 深度边界：实现按输入树递归，与 `serde_json::Value` 自身 Drop 的递归
//! 一致——输入应来自常规解析（`serde_json` 默认 128 层深度上限）；
//! 程序化构造超深嵌套 `Value` 时，调用方自行控制深度。
//!
//! # 示例
//!
//! ```
//! use oxcache::canonical_json_string;
//! use serde_json::json;
//!
//! let a = canonical_json_string(&json!({"b": 1, "a": 2}));
//! let b = canonical_json_string(&json!({"a": 2, "b": 1}));
//! assert_eq!(a, b);
//! ```

use serde_json::Value;

/// 递归归一化 JSON 值：对象键按字典序重排，数组保序，标量原样克隆。
///
/// 无论 `serde_json` 是否启用 `preserve_order`，输出键序恒为字典序。
pub fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted: std::collections::BTreeMap<&String, &Value> = map.iter().collect();
            Value::Object(
                sorted
                    .into_iter()
                    .map(|(k, v)| (k.clone(), canonical_json(v)))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical_json).collect()),
        other => other.clone(),
    }
}

/// 归一化并序列化为紧凑 JSON 字符串。
///
/// 对 `Value` 的序列化不存在失败路径（无 IO、数值已合法），故不返回
/// `Result`。
pub fn canonical_json_string(value: &Value) -> String {
    serde_json::to_string(&canonical_json(value)).expect("serializing a Value cannot fail")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_canonical_json_sorts_object_keys_recursively() {
        let input = json!({"b": {"d": 1, "c": 2}, "a": 3});
        let out = canonical_json(&input);
        assert_eq!(canonical_json_string(&out), r#"{"a":3,"b":{"c":2,"d":1}}"#);
    }

    #[test]
    fn test_canonical_json_preserves_array_order() {
        let input = json!({"list": [3, 1, 2], "objs": [{"z": 1, "a": 2}]});
        assert_eq!(
            canonical_json_string(&input),
            r#"{"list":[3,1,2],"objs":[{"a":2,"z":1}]}"#
        );
    }

    #[test]
    fn test_canonical_json_scalars_unchanged() {
        for v in [
            json!(null),
            json!(true),
            json!(42),
            json!(-1.5),
            json!("text"),
        ] {
            assert_eq!(canonical_json(&v), v);
        }
    }

    #[test]
    fn test_canonical_json_independent_of_input_order_and_preserve_order() {
        let a = json!({"x": 1, "y": {"p": 1, "q": 2}});
        let b = json!({"y": {"q": 2, "p": 1}, "x": 1});
        assert_eq!(canonical_json(&a), canonical_json(&b));
        assert_eq!(canonical_json_string(&a), canonical_json_string(&b));
    }

    #[test]
    fn test_canonical_json_empty_containers() {
        assert_eq!(canonical_json_string(&json!({})), "{}");
        assert_eq!(canonical_json_string(&json!([])), "[]");
    }

    #[test]
    fn test_canonical_json_unicode_keys_sort_by_scalar_value() {
        // 字典序按 String 的字节序（标量值序），非 locale
        let input = json!({"中": 1, "a": 2});
        assert_eq!(canonical_json_string(&input), r#"{"a":2,"中":1}"#);
    }
}
