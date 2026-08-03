# Security Policy

This document describes the security measures built into oxcache, the threat models they mitigate, and the process for reporting security vulnerabilities.

## Overview

oxcache provides a defense-in-depth security layer for Redis-backed caching. All security functions are gated behind the `redis` feature and are automatically enforced by the `RedisBackend`. They can also be called directly from application code for custom validation scenarios.

## 1. Redis TLS Enforcement

By default, oxcache requires TLS-encrypted Redis connections (`rediss://` scheme). Non-TLS connections (`redis://`) are rejected at backend construction time with a clear error message.

### Bypassing for Development

Non-TLS connections are allowed only when the environment variable `OXCACHE_ALLOW_INSECURE_REDIS` is explicitly set to one of:

- `I_UNDERSTAND_THE_RISKS`
- `development-only`

```bash
# Development only — never use in production
export OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS
```

> **Warning**: Setting this variable in production exposes Redis traffic (including credentials) to network interception. The application log will record a warning when this bypass is active.

## 2. Key Validation

**Function**: `oxcache::validate_redis_key(key: &str) -> OxCacheResult<()>`

Validates Redis keys before they are sent to the server, preventing command injection and malformed key attacks.

### Rules

| Rule | Limit | Description |
|------|-------|-------------|
| Non-empty | — | Empty keys are rejected |
| Max length | 524,288 bytes (512 KB) | Keys exceeding `MAX_KEY_LENGTH` are rejected |
| Dangerous chars | `\r`, `\n`, `\0` | CR/LF/NULL are rejected (prevent CRLF injection) |
| Control chars | All Unicode control chars (except `\t`) | Prevents binary/escape sequence injection |

### Example

```rust
use oxcache::validate_redis_key;

validate_redis_key("user:123")?;           // OK
validate_redis_key("user\r\nSET foo bar")?; // Err — CRLF injection detected
validate_redis_key("")?;                    // Err — empty key
```

## 3. Lua Script Sandbox

**Function**: `oxcache::validate_lua_script(script: &str, key_count: usize) -> OxCacheResult<()>`

Validates Lua scripts before `EVAL`/`EVALSHA` execution, preventing server-side resource exhaustion and dangerous command execution.

### Rules

| Rule | Limit | Description |
|------|-------|-------------|
| Max script length | 10,240 bytes (10 KB) | `MAX_LUA_SCRIPT_LENGTH` — prevents memory exhaustion |
| Max key count | 100 keys | `MAX_LUA_SCRIPT_KEYS` — prevents argument flooding |
| Forbidden commands | `FLUSHALL`, `FLUSHDB`, `SHUTDOWN`, `CONFIG`, `KEYS *`, infinite loop patterns | Blocks destructive and resource-draining operations |

The validator preprocesses the script to strip comments, string literals, and long-bracket content before pattern matching, preventing evasion via string obfuscation.

### Example

```rust
use oxcache::validate_lua_script;

// Safe script
validate_lua_script("return redis.call('GET', KEYS[1])", 1)?;

// Rejected — FLUSHALL is forbidden
validate_lua_script("redis.call('FLUSHALL')", 0)?;

// Rejected — too many keys
validate_lua_script("return 1", 101)?;
```

## 4. SCAN Pattern Restrictions

**Function**: `oxcache::validate_scan_pattern(pattern: &str) -> OxCacheResult<()>`
**Function**: `oxcache::clamp_scan_count(count: usize) -> usize`

Validates SCAN patterns and clamps COUNT to prevent Redis server overload.

### Rules

| Rule | Limit | Description |
|------|-------|-------------|
| Max pattern length | 256 chars | `MAX_SCAN_PATTERN_LENGTH` — prevents regex DoS |
| Max wildcards | 10 | `MAX_SCAN_WILDCARDS` — prevents broad scans |
| Clamped COUNT | 1,000 | `clamp_scan_count` caps COUNT to prevent full-keyspace scans |

### Example

```rust
use oxcache::{validate_scan_pattern, clamp_scan_count};

validate_scan_pattern("user:*")?;              // OK
validate_scan_pattern("*:*:*:*:*:*:*:*:*:*:*")?; // Err — too many wildcards

let count = clamp_scan_count(1_000_000);       // Returns 1000
```

## 5. Connection String Redaction

**Function**: `RedisBackend::redact_connection_string(conn_str: &str) -> String`
**Function**: `oxcache::redact_value(value: &str, visible_chars: usize) -> String`
**Function**: `oxcache::redact_cache_key(key: &str) -> String`
**Function**: `oxcache::redact_field(field_name: &str, value: &str) -> String`

These functions ensure credentials and sensitive data never appear in logs or error messages.

### Example

```rust
use oxcache::backend::memory::redis::RedisBackend;

let conn_str = "redis://:secret_password@localhost:6379/0";
let redacted = RedisBackend::redact_connection_string(conn_str);
// redacted == "redis://[REDACTED]@localhost:6379/0"
assert!(!redacted.contains("secret_password"));
```

## 6. Logging Security

**Function**: `oxcache::log_cache_key(key: &str) -> String`
**Function**: `oxcache::sanitize_message(msg: &str) -> String`

These utilities ensure cache keys and log messages are sanitized before being written to logs, preventing log injection attacks.

## Threat Model

| Threat | Mitigation |
|--------|-----------|
| CRLF injection via Redis keys | `validate_redis_key` rejects `\r`, `\n`, `\0` |
| Command injection via Lua scripts | `validate_lua_script` blocks `FLUSHALL`, `CONFIG`, etc. |
| Redis server overload via SCAN | `validate_scan_pattern` + `clamp_scan_count` |
| Credential leakage in logs | `redact_connection_string`, `redact_value` |
| Man-in-the-middle on Redis traffic | TLS enforcement (`rediss://` by default) |
| Resource exhaustion via large Lua scripts | `MAX_LUA_SCRIPT_LENGTH` (10 KB) and `MAX_LUA_SCRIPT_KEYS` (100) |
| Log injection via cache keys | `sanitize_message`, `log_cache_key` |

## Security Reporting Process

### Reporting a Vulnerability

If you discover a security vulnerability in oxcache:

1. **Do NOT open a public GitHub issue.**
2. Email the maintainer at the address listed in the `Cargo.toml` `authors` field.
3. Include:
   - A description of the vulnerability
   - Steps to reproduce (proof of concept)
   - Affected versions
   - Suggested fix (if any)
4. You will receive an acknowledgment within 48 hours.
5. A fix will be developed and released following responsible disclosure timelines.

### Supported Versions

Only the latest minor release receives security updates. When a new minor version is released, the previous minor version receives critical fixes for 30 days only.

## Configuration Summary

| Setting | Default | Override |
|---------|---------|----------|
| Redis TLS | Required (`rediss://`) | `OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS` |
| Max key length | 512 KB | Hardcoded (`MAX_KEY_LENGTH`) |
| Max Lua script length | 10 KB | Hardcoded (`MAX_LUA_SCRIPT_LENGTH`) |
| Max Lua script keys | 100 | Hardcoded (`MAX_LUA_SCRIPT_KEYS`) |
| Max SCAN pattern length | 256 chars | Hardcoded (`MAX_SCAN_PATTERN_LENGTH`) |
| Max SCAN wildcards | 10 | Hardcoded (`MAX_SCAN_WILDCARDS`) |
| Clamped SCAN COUNT | 1,000 | `clamp_scan_count()` |
