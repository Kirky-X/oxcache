# Capability: cache-core

## ADDED Requirements

### Requirement: Cache Key Generation
The system SHALL provide a standardized `KeyGenerator` utility for creating, validating, and managing cache keys.

#### Scenario: Generate key from template
- **WHEN** a user calls `KeyGenerator::generate("user:{id}", [123])`
- **THEN** the result SHALL be `"user:123"`

#### Scenario: Generate key with namespace
- **WHEN** a user configures a namespace "app:v1" and generates a key "user:{id}"
- **THEN** the result SHALL be `"app:v1:user:123"`

#### Scenario: Validate key format
- **WHEN** a user validates a key `"user:123"`
- **THEN** the validation SHALL pass for valid characters
- **AND** the validation SHALL fail for keys containing control characters

#### Scenario: Normalize key by trimming and lowercasing
- **WHEN** a user normalizes a key `"  User:123  "`
- **THEN** the result SHALL be `"user:123"`

### Requirement: Key Hash Fingerprint
The system SHALL support generating deterministic hash fingerprints for long keys.

#### Scenario: Generate 32-bit hash fingerprint
- **WHEN** a user requests a hash fingerprint for a key longer than 256 characters
- **THEN** the system SHALL return a murmur3_32 hash as the cache key
- **AND** the hash SHALL be deterministic for the same input

### Requirement: Dynamic Strategy Switching
The system SHALL support runtime configuration updates for cache strategies without restarting the service.

#### Scenario: Update TTL dynamically
- **WHEN** a user calls `manager.update_strategy(service_name, ttl = 7200)`
- **THEN** new cache entries SHALL use the updated TTL
- **AND** existing entries SHALL retain their original TTL

#### Scenario: Update capacity dynamically
- **WHEN** a user calls `manager.update_strategy(service_name, capacity = 20000)`
- **THEN** the L1 cache capacity SHALL be updated immediately
- **AND** excess entries SHALL be evicted using the current policy

#### Scenario: Switch eviction policy at runtime
- **WHEN** a user calls `manager.update_strategy(service_name, policy = "lfu")`
- **THEN** the L1 cache SHALL switch to LFU eviction policy
- **AND** the cache SHALL be rebuilt to apply the new policy

### Requirement: L1 Eviction Policy Configuration
The system SHALL expose configurable eviction policies for the L1 cache layer.

#### Scenario: Configure LRU policy
- **WHEN** a user initializes L1 cache with `EvictionPolicy::Lru`
- **THEN** cache entries SHALL be evicted based on least recently used order

#### Scenario: Configure LFU policy
- **WHEN** a user initializes L1 cache with `EvictionPolicy::Lfu`
- **THEN** cache entries SHALL be evicted based on least frequently used order

#### Scenario: Configure TinyLFU policy
- **WHEN** a user initializes L1 cache with `EvictionPolicy::TinyLfu`
- **THEN** cache entries SHALL use TinyLFU admission policy with LRU eviction

#### Scenario: Configure Random policy
- **WHEN** a user initializes L1 cache with `EvictionPolicy::Random`
- **THEN** cache entries SHALL be evicted randomly when capacity is reached

### Requirement: Strategy Change Events
The system SHALL emit events when cache strategies are modified.

#### Scenario: Emit strategy change event
- **WHEN** a strategy update is applied
- **THEN** the system SHALL emit a `StrategyChanged` event with old and new configuration
- **AND** the event SHALL include a timestamp and service identifier
