// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 跨实例安全事件关联（`security-events` feature）
//!
//! 把单实例的完整性失败与 fencing 异常经 Pub/Sub 总线升格为集群态势。
//! 传输复用 [`crate::features::invalidation::PubSubTransport`]，默认响应器
//! 与 [`crate::features::degradation::DegradationController`] 联动。

use std::time::Duration;

mod bus;
mod monitor;
mod signal;

pub use bus::{SecurityEventBus, SecurityEventConfig};
pub use monitor::{SecurityHook, SecurityResponder, degradation_responder};
pub use signal::SecuritySignal;

/// 默认 Pub/Sub 通道
pub const DEFAULT_CHANNEL: &str = "oxcache:sec-events";
/// 默认信号时效（过期信号在订阅端丢弃）
pub const DEFAULT_SIGNAL_TTL: Duration = Duration::from_secs(30);
