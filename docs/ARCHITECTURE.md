# Architecture Documentation

This document describes the architecture, design decisions, and technical details of the Oxcache library.

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Components](#components)
- [Data Flow](#data-flow)
- [Consistency Model](#consistency-model)
- [Failure Handling](#failure-handling)
- [Performance Optimization](#performance-optimization)
- [Security](#security)
- [Scalability](#scalability)

## Overview

Oxcache is a multi-level caching system designed for high-performance, production-ready applications. It combines:

- **L1 Cache**: In-memory cache using Moka (LRU/TinyLFU eviction)
- **L2 Cache**: Distributed cache using Redis
- **Sync Layer**: Pub/Sub-based invalidation for multi-instance consistency
- **Recovery Layer**: Write-ahead log (WAL) for durability and failover

### Design Goals

1. **Performance**: L1 latency 50-100ns, L2 latency 1-5ms (P99, varies by environment)
2. **Reliability**: Automatic failover, data consistency across instances
3. **Usability**: Zero-boilerplate integration via `#[cached]` macro
4. **Observability**: Comprehensive metrics, tracing, and health checks
5. **Security**: Protection against cache penetration and DoS attacks

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         Application                         │
│                  (Functions with #[cached])                  │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                      Cache Manager                           │
│                   (DashMap: Service → Client)                │
└────────────────────────┬────────────────────────────────────┘
                         │
        ┌────────────────┼────────────────┐
        ▼                ▼                ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│  L1 Client   │  │ L2 Client    │  │  Sync Layer  │
│   (Moka)     │  │  (Redis)     │  │  (Pub/Sub)   │
└──────────────┘  └──────────────┘  └──────────────┘
        │                │                │
        ▼                ▼                ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│   Memory     │  │   Redis      │  │   Pub/Sub    │
│   (LRU)      │  │   Cluster    │  │   Channel    │
└──────────────┘  └──────────────┘  └──────────────┘
                                            │
                                            ▼
                                    ┌──────────────┐
                                    │  Recovery    │
                                    │     WAL      │
                                    └──────────────┘
```

## Components

### 1. Cache Manager (`manager.rs`)

**Responsibility**: Central registry for all cache clients

**Data Structures**:
- `DashMap<String, Arc<dyn CacheOps>>`: Thread-safe service-to-client mapping

**Key Methods**:
- `init(config: OxcacheConfig)`: Initialize all services
- `get_client(name: &str)`: Retrieve client by service name
- `shutdown_all()`: Cleanup all clients

**Thread Safety**: Uses `DashMap` for lock-free concurrent access

### 2. L1 Cache Backend (`backend/l1.rs`)

**Technology**: Moka (high-performance concurrent cache)

**Eviction Policy**: TinyLFU (Least Frequently Used with frequency sketch)

**Configuration**:
```rust
pub struct L1Config {
    pub max_capacity: u64,      // Maximum number of entries
    pub time_to_live: Option<u64>,  // TTL in seconds
    pub time_to_idle: Option<u64>,   // Idle TTL in seconds
}
```

**Performance Characteristics**:
- Read: 50-100ns (P99, in-memory)
- Write: 50-200ns (P99, in-memory)
- Thread-safe with lock-free design

> **Note**: Performance varies based on hardware, data size, and access patterns

### 3. L2 Cache Backend (`backend/l2.rs`)

**Technology**: Redis (Standalone/Sentinel/Cluster)

**Connection Management**:
- Connection pooling via `connection-manager`
- Automatic reconnection on failure
- Cluster topology awareness

**Serialization**:
- JSON: Human-readable, larger size
- Bincode: Binary, smaller size, faster

**Features**:
- Batch write optimization
- Pub/Sub for invalidation
- Write-ahead logging

### 4. Two-Level Cache Client (`client/two_level.rs`)

**Read Path**:
```
1. Check L1 cache
2. If hit → Return value
3. If miss → Check L2 cache
4. If L2 hit → Populate L1 → Return value
5. If L2 miss → Return None
```

**Write Path**:
```
1. Write to L1 cache (async)
2. Write to L2 cache (async, can be batched)
3. Write to WAL for durability
4. Publish invalidation if needed
```

### 5. Batch Writer (`sync/batch_writer.rs`)

**Purpose**: Optimize L2 write throughput by batching multiple operations

**Algorithm**:
1. Accumulate operations in buffer
2. Flush when buffer size > threshold OR timeout
3. Use Redis MSET for batch writes

**Performance**: 10-50x improvement in throughput for write-heavy workloads

### 6. Invalidation Service (`sync/invalidation.rs`)

**Purpose**: Ensure consistency across multiple instances

**Protocol**:
```
1. Instance A updates key "user:123"
2. Instance A publishes invalidation message:
   {
     "key": "user:123",
     "version": "v5",
     "timestamp": 1704921600
   }
3. Instance B receives message via Pub/Sub
4. Instance B removes "user:123" from L1 if version < v5
```

**Version-Based Invalidation**: Prevents race conditions and thundering herd

### 7. Recovery Layer (`recovery/`)

#### Write-Ahead Log (WAL) (`wal.rs`)

**Purpose**: Ensure no data loss during Redis failures

**Structure**:
```
WAL Entry:
{
  "type": "SET" | "DELETE",
  "key": "user:123",
  "value": "...",  // Base64 encoded
  "timestamp": 1704921600
}
```

**Replay Logic**:
```
1. Redis recovers
2. System reads WAL entries
3. Replay entries to Redis in order
4. Clear WAL after successful replay
```

#### Health Checker (`health.rs`)

**Health Checks**:
- L1 availability (memory usage)
- L2 connectivity (ping/pong)
- WAL size (disk space)

**Degradation Modes**:
- **L2 failure**: Operate in L1-only mode
- **Low memory**: Reduce L1 capacity
- **Disk full**: Pause WAL, log warning

### 8. Database Integration (`database/`)

**Supported Databases**:
- MySQL (`sqlx-mysql`)
- PostgreSQL (`sqlx-postgres`)
- SQLite (`sqlx-sqlite`)

**Partition Support**:
```rust
pub enum PartitionStrategy {
    TimeBased(TimeUnit),  // Partition by time
    HashBased(u32),       // Partition by hash
    Custom(Box<dyn Fn(&str) -> String>),  // Custom logic
}
```

**Cache-Aside Pattern**:
```
1. Check cache
2. If miss, load from database
3. Populate cache
4. Return value
```

### 9. Security Features

#### Bloom Filter (`bloom_filter.rs`)

**Purpose**: Prevent cache penetration attacks

**Algorithm**: MurmurHash3 with bit array

**Configuration**:
```rust
pub struct BloomFilterConfig {
    pub expected_elements: u64,
    pub false_positive_rate: f64,
}
```

**Usage**:
```
Before cache lookup → Check Bloom filter
If filter says "definitely not" → Skip cache, go to DB
If filter says "maybe" → Check cache
```

#### Rate Limiter (`rate_limiting.rs`)

**Purpose**: Prevent DoS attacks

**Algorithm**: Token bucket with refill

**Configuration**:
```rust
pub struct RateLimitConfig {
    pub max_requests_per_second: u32,
    pub burst_capacity: u32,
    pub block_duration_secs: u64,
}
```

## Data Flow

### Read Operation

```
┌─────────────┐
│ Application │
│  #[cached]  │
└──────┬──────┘
       │
       ▼
┌─────────────────────────────────┐
│  Check L1 (Moka)                │
│  - If hit: Return value         │
│  - If miss: Continue            │
└──────┬──────────────────────────┘
       │ miss
       ▼
┌─────────────────────────────────┐
│  Check L2 (Redis)               │
│  - If hit: Populate L1 → Return │
│  - If miss: Return None        │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Return value to application    │
└─────────────────────────────────┘
```

### Write Operation

```
┌─────────────┐
│ Application │
└──────┬──────┘
       │
       ▼
┌─────────────────────────────────┐
│  Write to L1 (async, immediate)  │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Add to Batch Writer buffer     │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Write to WAL (async)           │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Flush to L2 (batch)            │
│  - When buffer full OR timeout  │
│  - Use Redis MSET               │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Publish invalidation (Pub/Sub) │
│  - Key, version, timestamp      │
└─────────────────────────────────┘
```

## Consistency Model

### Eventual Consistency

Oxcache provides **eventual consistency** across instances:

- **Strong consistency within instance**: L1 + L2 are always consistent
- **Eventual consistency across instances**: Propagation delay of < 100ms typically

### Invalidation Propagation

```
Instance A                Pub/Sub Channel               Instance B
   │                          │                            │
   │─── UPDATE key:123 ──────►│                            │
   │                          │─── INVALIDATE key:123 ────►│
   │                          │                            │
   │                          │                            │
```

### Versioning Scheme

```
Version format: "v{timestamp}_{instance_id}"

Example: "v1704921600_i32"

Compare versions lexicographically:
- v1704921600_i32 < v1704921601_i45  (newer wins)
```

## Failure Handling

### Redis Failure

**Detection**:
- Connection timeout
- Ping failure
- Connection closed by remote

**Recovery**:
```
1. Switch to L1-only mode
2. Log warning
3. Continue serving from L1
4. Reconnect in background
5. Replay WAL on reconnect
6. Resume normal operation
```

### Network Partition

**Behavior**:
- Instances continue operating with local data
- Invalidation messages queued
- On recovery: Reconcile using versioning

### Disk Failure (WAL)

**Degradation**:
- Pause WAL writes
- Log critical error
- Continue operating (less durable)

## Performance Optimization

### Optimization Techniques

1. **Batch Write**: Buffer multiple operations, flush with MSET
2. **Connection Pooling**: Reuse Redis connections
3. **Lock-Free L1**: Moka's concurrent cache design
4. **Binary Serialization**: Bincode for smaller payload size
5. **AHash**: High-performance hash algorithm

### Performance Tuning

```toml
[optimization]
# L1 cache
l1_max_capacity = 10000
l1_time_to_idle = 600

# L2 cache
l2_batch_size = 100
l2_batch_timeout_ms = 50

# Serialization
serialization_type = "bincode"  # "json" or "bincode"
```

### Benchmark Results

> Test environment: M1 Pro, 16GB RAM, macOS, Redis 7.0
> 
> **Note**: Performance varies based on hardware, network conditions, and data size.

| Operation | Throughput | Latency (P99) |
|-----------|------------|---------------|
| L1 Read | 5-10M ops/sec | 50-100ns |
| L1 Write | 2-5M ops/sec | 50-200ns |
| L2 Read | 50-100K ops/sec | 1-5ms |
| L2 Write (batch) | 200-500K ops/sec | 1-10ms |

## Security

### Threat Model

1. **Cache Penetration**: Attacker requests non-existent keys
2. **Cache Breakdown**: Hot key expires, many requests hit DB
3. **DoS Attack**: High request rate overwhelms system

### Defenses

1. **Bloom Filter**: Prevent cache penetration
2. **Cache Locking**: Prevent cache breakdown
3. **Rate Limiting**: Prevent DoS attacks
4. **Sensitive Data Redaction**: Auto-redact in logs

### Best Practices

1. **Key Design**: Use stable, predictable keys
2. **TTL Strategy**: Set appropriate TTL based on data volatility
3. **Access Control**: Use Redis AUTH + TLS
4. **Monitoring**: Track metrics for anomalies

## Scalability

### Horizontal Scaling

Add more instances:
```
┌──────────┐  ┌──────────┐  ┌──────────┐
│ Instance │  │ Instance │  │ Instance │
│    1     │  │    2     │  │    3     │
└────┬─────┘  └────┬─────┘  └────┬─────┘
     │             │             │
     └─────────────┴─────────────┘
                   │
              ┌────▼────┐
              │  Redis  │
              │ Cluster │
              └─────────┘
```

### Vertical Scaling

- Increase L1 capacity (more memory)
- Use faster Redis (SSD, dedicated server)
- Enable Redis persistence (AOF + RDB)

### Partitioning

Database partitioning for large datasets:
```rust
PartitionConfig::time_based(TimeUnit::Month)  // By month
PartitionConfig::hash_based(16)                // 16 shards
```

## Future Enhancements

1. **L3 Cache**: Add support for other distributed caches (Cassandra, Memcached)
2. **Adaptive TTL**: Machine learning-based TTL optimization
3. **Geo-Distribution**: Multi-region replication
4. **Cache Warming**: Intelligent warmup strategies
5. **Compression**: Zstd compression for large values

## References

- [Moka Documentation](https://github.com/moka-rs/moka)
- [Redis Documentation](https://redis.io/documentation)
- [TinyLFU Paper](https://arxiv.org/abs/1512.00757)
- [Bloom Filter](https://en.wikipedia.org/wiki/Bloom_filter)
