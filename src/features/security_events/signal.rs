// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 跨实例安全信号类型（`security-events` feature）
//!
//! [`SecuritySignal`] 以 serde JSON（内部 tag）编码，经既有
//! `PubSubTransport` 在实例间传播。信号只陈述观测事实，不承载认证签名
//! （Pub/Sub 通道视为内网可信面）。

use crate::error::{OxCacheError, OxCacheResult};
use serde::{Deserialize, Serialize};

/// 跨实例安全信号
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SecuritySignal {
    /// 完整性校验连续失败达到阈值（键名已脱敏）
    IntegrityCompromised {
        instance_id: String,
        key: String,
        consecutive_failures: u32,
        raised_at_unix_ms: u64,
    },
    /// RedLock fencing token 出现非单调回退
    FencingAnomaly {
        instance_id: String,
        lock_key: String,
        node_index: usize,
        observed_token: u64,
        last_token: u64,
        raised_at_unix_ms: u64,
    },
}

impl SecuritySignal {
    /// 发出信号的实例 ID
    pub fn instance_id(&self) -> &str {
        match self {
            Self::IntegrityCompromised { instance_id, .. }
            | Self::FencingAnomaly { instance_id, .. } => instance_id,
        }
    }

    /// 信号产生时刻（Unix 毫秒）
    pub fn raised_at_unix_ms(&self) -> u64 {
        match self {
            Self::IntegrityCompromised {
                raised_at_unix_ms, ..
            }
            | Self::FencingAnomaly {
                raised_at_unix_ms, ..
            } => *raised_at_unix_ms,
        }
    }

    /// JSON 编码
    pub fn encode(&self) -> OxCacheResult<String> {
        serde_json::to_string(self)
            .map_err(|e| OxCacheError::Serialization(format!("security signal encode failed: {e}")))
    }

    /// JSON 解码；未知 type tag 返回错误
    pub fn decode(raw: &str) -> OxCacheResult<Self> {
        serde_json::from_str(raw)
            .map_err(|e| OxCacheError::Serialization(format!("security signal decode failed: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integrity() -> SecuritySignal {
        SecuritySignal::IntegrityCompromised {
            instance_id: "node-a".to_string(),
            key: "tenant:**".to_string(),
            consecutive_failures: 3,
            raised_at_unix_ms: 1_700_000_000_123,
        }
    }

    fn fencing() -> SecuritySignal {
        SecuritySignal::FencingAnomaly {
            instance_id: "node-b".to_string(),
            lock_key: "lease".to_string(),
            node_index: 2,
            observed_token: 5,
            last_token: 9,
            raised_at_unix_ms: 1_700_000_000_456,
        }
    }

    #[test]
    fn integrity_signal_roundtrip() {
        let signal = integrity();
        let decoded = SecuritySignal::decode(&signal.encode().unwrap()).unwrap();
        assert_eq!(decoded, signal);
    }

    #[test]
    fn fencing_signal_roundtrip() {
        let signal = fencing();
        let decoded = SecuritySignal::decode(&signal.encode().unwrap()).unwrap();
        assert_eq!(decoded, signal);
    }

    #[test]
    fn unknown_type_tag_is_rejected() {
        let raw = r#"{"type":"unknown_type","instance_id":"node-a"}"#;
        assert!(SecuritySignal::decode(raw).is_err());
    }

    #[test]
    fn accessors_work_for_both_variants() {
        assert_eq!(integrity().instance_id(), "node-a");
        assert_eq!(fencing().instance_id(), "node-b");
        assert_eq!(integrity().raised_at_unix_ms(), 1_700_000_000_123);
        assert_eq!(fencing().raised_at_unix_ms(), 1_700_000_000_456);
    }
}
