// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Features module

#[cfg(feature = "bloom")]
pub mod bloom_filter;

#[cfg(feature = "lock")]
pub mod dist_lock;

#[cfg(feature = "invalidation")]
pub mod invalidation;

#[cfg(feature = "encrypt")]
pub mod encryption;

#[cfg(feature = "config-confers")]
pub mod confers_config;

#[cfg(feature = "degradation")]
pub mod degradation;

#[cfg(feature = "audit")]
pub mod audit;

#[cfg(feature = "compression")]
pub mod compression;

/// Offload 后台任务子系统（absorb-hitbox-features）：去重 + 限并发 + 超时策略
#[cfg(feature = "offload")]
pub mod offload;

/// SWR 三态过期装饰器（absorb-hitbox-features）：Actual/Stale/Expired + 三策略
#[cfg(feature = "stale")]
pub mod stale;

/// 热 key 采样观测（审计 F11）：独立组件，不入读路径
#[cfg(feature = "hotkey")]
pub mod hotkey;

#[cfg(feature = "adaptive-ttl")]
pub mod adaptive_ttl;

#[cfg(feature = "versioning")]
pub mod versioning;

#[cfg(feature = "hotkey")]
pub use hotkey::HotKeyTracker;

#[cfg(feature = "adaptive-ttl")]
pub use adaptive_ttl::{AdaptiveTtlBackend, AdaptiveTtlConfig};

#[cfg(feature = "bloom")]
pub use bloom_filter::BloomFilter;

#[cfg(all(feature = "bloom", any(feature = "memory", feature = "redis")))]
pub use bloom_filter::{BloomFilterBackend, BloomFilterBackendBuilder};

#[cfg(feature = "lock")]
pub use dist_lock::{DefaultLockProvider, DistLockBuilder, DistributedLock, LockProvider};

#[cfg(feature = "invalidation")]
pub use invalidation::{
    InMemoryPubSubTransport, InvalidatingBackend, InvalidationBus, InvalidationConfig,
    InvalidationKind, InvalidationMessage, KeyspaceNotificationConfig,
    KeyspaceNotificationListener, PubSubTransport, RedisPubSubTransport,
};

#[cfg(feature = "encrypt")]
pub use encryption::{ENVELOPE_VERSION, EncryptedBackend, ValueCipher};

#[cfg(feature = "integrity")]
pub use encryption::integrity::{HMAC_ENVELOPE_VERSION, HmacSigner, IntegrityBackend};

#[cfg(feature = "config-confers")]
pub use confers_config::{
    CacheConfigSource, ConfersConfigSource, ConfersConfigWatcher, ConfigChangeListener,
    ConfigSnapshot, OxcacheConfig,
};

#[cfg(feature = "degradation")]
pub use degradation::{
    DegradableBackend, DegradationController, DegradationSnapshot, DegradationState,
};

/// 降级观测桥依赖 `degradation` 的类型与 `telemetry` 的 tracing 依赖
#[cfg(all(feature = "degradation", feature = "telemetry"))]
pub use degradation::DegradationTracing;

#[cfg(feature = "audit")]
pub use audit::{
    AuditAction, AuditEvent, AuditEventPublisher, InMemoryAuditPublisher, NoOpAuditPublisher,
    redact_key_for_audit,
};

#[cfg(all(feature = "audit", feature = "telemetry"))]
pub use audit::TracingAuditPublisher;

// inklog 依赖精确钉 =0.3.0-rc.7（其自身 registry 依赖 oxcache 0.5.0-rc.6，与本仓根包
// 同版本号而异 source）；升级 oxcache 时须同步确认对齐，且 examples 的精确钉须同改
#[cfg(all(feature = "audit", feature = "inklog"))]
pub use audit::InklogAuditPublisher;

#[cfg(feature = "compression")]
pub use compression::{
    CompressingBackend, DEFAULT_COMPRESSION_THRESHOLD, DEFAULT_ZSTD_LEVEL, ZSTD_MAGIC,
};

#[cfg(feature = "versioning")]
pub use versioning::{MemoryVersionedCache, VersionedStore, VersionedValue};

#[cfg(all(feature = "versioning", feature = "redis"))]
pub use versioning::RedisVersionedCache;
