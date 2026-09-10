// Copyright (c) 2025-2026 Kirky.X
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

#[cfg(feature = "bloom")]
pub use bloom_filter::BloomFilter;

#[cfg(all(
    feature = "bloom",
    any(
        feature = "memory",
        feature = "redis",
        feature = "minimal",
        feature = "core",
        feature = "full"
    )
))]
pub use bloom_filter::{BloomFilterBackend, BloomFilterBackendBuilder};

#[cfg(feature = "lock")]
pub use dist_lock::{DefaultLockProvider, DistLockBuilder, DistributedLock, LockProvider};

#[cfg(feature = "invalidation")]
pub use invalidation::{
    InMemoryPubSubTransport, InvalidationBus, InvalidationConfig, InvalidationKind,
    InvalidationMessage, InvalidatingBackend, KeyspaceNotificationConfig,
    KeyspaceNotificationListener, PubSubTransport, RedisPubSubTransport,
};

#[cfg(feature = "encrypt")]
#[cfg(feature = "encrypt")]
pub use encryption::{EncryptedBackend, ENVELOPE_VERSION, ValueCipher};

#[cfg(feature = "integrity")]
pub use encryption::integrity::{HmacSigner, HMAC_ENVELOPE_VERSION, IntegrityBackend};

#[cfg(feature = "config-confers")]
pub use confers_config::{
    CacheConfigSource, ConfigChangeListener, ConfigSnapshot, ConfersConfigSource,
    ConfersConfigWatcher, OxcacheConfig,
};

#[cfg(feature = "degradation")]
pub use degradation::{DegradableBackend, DegradationController, DegradationState};

#[cfg(feature = "audit")]
pub use audit::{
    AuditAction, AuditEvent, AuditEventPublisher, InMemoryAuditPublisher, NoOpAuditPublisher,
    redact_key_for_audit,
};

#[cfg(all(feature = "audit", feature = "telemetry"))]
pub use audit::TracingAuditPublisher;

#[cfg(feature = "compression")]
pub use compression::{
    CompressingBackend, DEFAULT_COMPRESSION_THRESHOLD, DEFAULT_ZSTD_LEVEL, ZSTD_MAGIC,
};
