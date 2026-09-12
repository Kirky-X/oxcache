// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 二进制序列化格式
//!
//! [`SerializationFormat`] 提供 JSON 之外的可选传输格式：
//!
//! - `serde-bincode` feature → [`SerializationFormat::Bincode`]（bincode 1.x）；
//! - `postcard` feature → [`SerializationFormat::Postcard`]（postcard 1.x）。
//!
//! 二进制格式天然无嵌套 DoS（无深度递归文本解析），仍保留 5 MiB 大小上限
//! 的纵深防御。**同一键前缀不得混用格式**：格式无自描述头，混用会产生
//! 脏数据——请按命名空间隔离格式选择。
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::infra::serialization::{SerializationFormat, UnifiedSerializer};
//!
//! let ser = UnifiedSerializer::with_format(SerializationFormat::Bincode);
//! let bytes = ser.serialize(&value)?;   // L2 传输体积显著小于 JSON
//! let value: MyType = ser.deserialize(&bytes)?;
//! ```

use crate::error::{OxCacheError, OxCacheResult};
use serde::{Serialize, de::DeserializeOwned};

/// 序列化传输格式
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SerializationFormat {
    /// JSON（serde_json，默认；带深度防御解析）
    #[default]
    Json,
    /// bincode 1.x（紧凑二进制，需 `serde-bincode` feature）
    #[cfg(feature = "serde-bincode")]
    Bincode,
    /// postcard 1.x（紧凑二进制，需 `postcard` feature）
    #[cfg(feature = "postcard")]
    Postcard,
}

impl SerializationFormat {
    /// 当前编译配置下可用的全部格式（用于注册表/诊断输出）
    pub fn available() -> &'static [SerializationFormat] {
        #[cfg(not(any(feature = "serde-bincode", feature = "postcard")))]
        {
            &[SerializationFormat::Json]
        }
        #[cfg(any(feature = "serde-bincode", feature = "postcard"))]
        {
            // JSON 恒可用；其余按 feature
            &[
                SerializationFormat::Json,
                #[cfg(feature = "serde-bincode")]
                SerializationFormat::Bincode,
                #[cfg(feature = "postcard")]
                SerializationFormat::Postcard,
            ]
        }
    }

    /// 格式名（诊断/Prometheus 标签用）
    pub fn name(&self) -> &'static str {
        match self {
            SerializationFormat::Json => "json",
            #[cfg(feature = "serde-bincode")]
            SerializationFormat::Bincode => "bincode",
            #[cfg(feature = "postcard")]
            SerializationFormat::Postcard => "postcard",
        }
    }

    /// 按名查找格式（未知名返回 None）
    pub fn by_name(name: &str) -> Option<SerializationFormat> {
        SerializationFormat::available()
            .iter()
            .copied()
            .find(|f| f.name() == name)
    }
}

/// 按格式序列化（统一 5 MiB 上限纵深防御）
pub fn serialize_with_format<T: Serialize>(
    format: SerializationFormat,
    value: &T,
) -> OxCacheResult<Vec<u8>> {
    let bytes = match format {
        SerializationFormat::Json => serde_json::to_vec(value)
            .map_err(|e| OxCacheError::Serialization(e.to_string()))?,
        #[cfg(feature = "serde-bincode")]
        SerializationFormat::Bincode => bincode::serialize(value)
            .map_err(|e| OxCacheError::Serialization(e.to_string()))?,
        #[cfg(feature = "postcard")]
        SerializationFormat::Postcard => postcard::to_allocvec(value)
            .map_err(|e| OxCacheError::Serialization(e.to_string()))?,
    };
    crate::infra::serialization::utils::check_data_size(
        &bytes,
        crate::core::MAX_JSON_SIZE,
        format.name(),
    )?;
    Ok(bytes)
}

/// 按格式反序列化（二进制格式无深度递归问题；JSON 保留深度防御）
pub fn deserialize_with_format<T: DeserializeOwned>(
    format: SerializationFormat,
    data: &[u8],
) -> OxCacheResult<T> {
    crate::infra::serialization::utils::check_data_size(
        data,
        crate::core::MAX_JSON_SIZE,
        format.name(),
    )?;
    match format {
        SerializationFormat::Json => crate::infra::serialization::depth_limited::deserialize_safe(
            data,
            crate::core::MAX_JSON_DEPTH,
        )
        .map_err(|e| OxCacheError::Serialization(e.to_string())),
        #[cfg(feature = "serde-bincode")]
        SerializationFormat::Bincode => bincode::deserialize(data)
            .map_err(|e| OxCacheError::Serialization(e.to_string())),
        #[cfg(feature = "postcard")]
        SerializationFormat::Postcard => postcard::from_bytes(data)
            .map_err(|e| OxCacheError::Serialization(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    struct Sample {
        id: u64,
        name: String,
        tags: Vec<String>,
        score: f64,
    }

    fn sample() -> Sample {
        Sample {
            id: 42,
            name: "cache-entry".to_string(),
            tags: vec!["l1".to_string(), "l2".to_string(), "ttl".to_string()],
            score: 0.987,
        }
    }

    fn all_expected_formats() -> Vec<SerializationFormat> {
        SerializationFormat::available().to_vec()
    }

    /// 互操作：每个可用格式 roundtrip 均正确
    #[test]
    fn every_available_format_roundtrips() {
        for format in all_expected_formats() {
            let bytes = serialize_with_format(format, &sample()).unwrap();
            let decoded: Sample = deserialize_with_format(format, &bytes).unwrap();
            assert_eq!(decoded, sample(), "format {} roundtrip failed", format.name());
        }
    }

    /// 互操作：JSON 与二进制格式表达同一值（各自独立可解）
    #[cfg(all(feature = "serde-bincode", feature = "postcard"))]
    #[test]
    fn formats_are_interoperable_on_the_same_value() {
        let value = sample();
        let json = serialize_with_format(SerializationFormat::Json, &value).unwrap();
        let bin = serialize_with_format(SerializationFormat::Bincode, &value).unwrap();
        let card = serialize_with_format(SerializationFormat::Postcard, &value).unwrap();

        // 三种格式字节不同，但都解码回同一值
        assert_ne!(json, bin);
        assert_ne!(json, card);
        let a: Sample = deserialize_with_format(SerializationFormat::Json, &json).unwrap();
        let b: Sample = deserialize_with_format(SerializationFormat::Bincode, &bin).unwrap();
        let c: Sample = deserialize_with_format(SerializationFormat::Postcard, &card).unwrap();
        assert_eq!(a, b);
        assert_eq!(b, c);

        // L2 传输体积对比（记录 docs/PERFORMANCE.md）
        println!(
            "serialization size: json={} bincode={} postcard={}",
            json.len(),
            bin.len(),
            card.len()
        );
        // postcard（varint 编码）显著小于 JSON
        assert!(
            card.len() < json.len(),
            "postcard 应比 JSON 更紧凑 (card={}, json={})",
            card.len(),
            json.len()
        );

        // bincode 1.x 定宽编码：数值密集的大值场景下小于 JSON
        // （短字符串场景下 JSON 可能更小，选择格式时以真实负载度量为准）
        #[derive(Serialize, Deserialize, PartialEq)]
        struct NumericHeavy {
            a: u64,
            b: i64,
            c: f64,
            d: u64,
            e: i64,
            f: f64,
        }
        let heavy = NumericHeavy {
            a: 1700000000000,
            b: -42,
            c: 3.15,
            d: u64::MAX,
            e: 987654321,
            f: 2.71,
        };
        let h_json = serialize_with_format(SerializationFormat::Json, &heavy).unwrap();
        let h_bin = serialize_with_format(SerializationFormat::Bincode, &heavy).unwrap();
        println!("numeric-heavy size: json={} bincode={}", h_json.len(), h_bin.len());
        assert!(
            h_bin.len() < h_json.len(),
            "数值密集场景 bincode 应更紧凑 (bin={}, json={})",
            h_bin.len(),
            h_json.len()
        );
    }

    /// 二进制格式无 JSON 起始符，可直接判别
    #[cfg(feature = "serde-bincode")]
    #[test]
    fn bincode_bytes_are_not_json() {
        let bin = serialize_with_format(SerializationFormat::Bincode, &sample()).unwrap();
        assert_ne!(bin[0], b'{');
    }

    #[test]
    fn format_by_name_lookup() {
        assert_eq!(SerializationFormat::by_name("json"), Some(SerializationFormat::Json));
        #[cfg(feature = "serde-bincode")]
        assert_eq!(
            SerializationFormat::by_name("bincode"),
            Some(SerializationFormat::Bincode)
        );
        assert_eq!(SerializationFormat::by_name("msgpack"), None);
    }

    #[test]
    fn oversized_payload_rejected() {
        let big: Vec<u8> = Vec::new();
        // 小数据不应报错（正常路径）
        let _: Vec<u8> = deserialize_with_format(SerializationFormat::Json, &serialize_with_format(SerializationFormat::Json, &big).unwrap()).unwrap();
        // 超限数据必须被拒（纵深防御对二进制格式同样生效）
        let oversized = vec![0u8; crate::core::MAX_JSON_SIZE + 1];
        let err = serialize_with_format(SerializationFormat::Json, &oversized)
            .expect_err("超限序列化必须报错");
        assert!(matches!(err, OxCacheError::Serialization(_)));
    }
}
