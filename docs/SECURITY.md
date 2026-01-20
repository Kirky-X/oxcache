# Oxcache Security Documentation

This document describes the security features and best practices for using Oxcache.

## Security Features

### 1. Input Validation

Oxcache validates all user inputs before passing them to Redis to prevent injection attacks and other security issues.

#### Redis Key Validation

The `validate_redis_key()` function checks:
- Keys cannot be empty
- Keys cannot exceed 512KB in length
- Keys cannot contain dangerous characters: `\r`, `\n`, `\0` (Redis protocol separators)

```rust
use oxcache::security::validate_redis_key;

// Validate a key before use
validate_redis_key("user:123")?; // Ok(())
validate_redis_key("")?; // Error: Empty key
validate_redis_key("key\r\nvalue")?; // Error: Contains CRLF
```

#### Lua Script Validation

The `validate_lua_script()` function validates Lua scripts before execution:
- Maximum script length: 10KB
- Maximum number of keys: 100
- Blocks dangerous commands:
  - `FLUSHALL`, `FLUSHDB` - Data destruction
  - `KEYS` - Can block Redis
  - `SHUTDOWN` - Service disruption
  - `DEBUG`, `CONFIG`, `SAVE`, `BGSAVE`, `MONITOR`, `SYNC` - Administrative commands

```rust
use oxcache::security::validate_lua_script;

// Safe script
validate_lua_script("return redis.call('GET', KEYS[1])", 1)?; // Ok(())

// Dangerous script - will be rejected
validate_lua_script("return redis.call('FLUSHALL')", 0)?; // Error!
```

#### SCAN Pattern Validation

The `validate_scan_pattern()` function prevents ReDoS attacks:
- Maximum pattern length: 256 characters
- Maximum wildcard (`*`) count: 10

```rust
use oxcache::security::validate_scan_pattern;

// Safe pattern
validate_scan_pattern("user:*")?; // Ok(())

// Dangerous pattern - too many wildcards
validate_scan_pattern("*".repeat(20))?; // Error!
```

### 2. Timeout Protection

All long-running operations have timeout protection:

| Operation | Timeout | Description |
|-----------|---------|-------------|
| Lua Scripts | 30 seconds | Prevents Redis blocking from long-running scripts |
| SCAN Operations | 30 seconds | Prevents hanging scans on large datasets |

Timeouts cannot be disabled to ensure system stability.

### 3. Secure Lock Values

Distributed locks use cryptographically secure UUID v4 values generated automatically:

```rust
use oxcache::CacheOps;

// Old API (insecure - requires user to provide lock value)
client.lock("lock_key", "user_provided_value", 60).await?;

// New API (secure - library generates UUID automatically)
let lock_value = client.lock("lock_key", 60).await?;
if let Some(value) = lock_value {
    // Use value to release the lock later
    client.unlock("lock_key", &value).await?;
}
```

### 4. Connection String Redaction

Passwords in connection strings are automatically redacted in logs:

```rust
use oxcache::database::normalize_connection_string_with_redaction;

// Without redaction
let s = normalize_connection_string_with_redaction("redis://user:password123@localhost:6379", false);
// Returns: "redis://user:password123@localhost:6379"

// With redaction (for logging)
let s = normalize_connection_string_with_redaction("redis://user:password123@localhost:6379", true);
// Returns: "redis://user:****@localhost:6379"
```

## Security Best Practices

### 1. Use the Library's Validation

Always use the library's validation functions instead of implementing your own:

```rust
// DON'T: Custom validation that might miss edge cases
if key.contains('\r') { return Err("invalid"); }

// DO: Use the library's validated function
validate_redis_key(key)?;
```

### 2. Avoid Custom Lua Scripts

Use the built-in cache operations whenever possible:

```rust
// DO: Use built-in operations
client.set("key", "value", Some(60)).await?;

// AVOID: Custom Lua when not needed
// let script = "return redis.call('SET', KEYS[1], ARGV[1])";
```

### 3. Keep Timeouts Enabled

The 30-second timeout is a security feature. Don't try to disable it:

```rust
// DON'T: Try to bypass timeout
// This is not possible by design

// DO: Work with the timeout
match client.eval(script, &keys, &args).await {
    Ok(result) => result,
    Err(CacheError::Timeout(msg)) => {
        // Handle timeout gracefully
    }
}
```

### 4. Don't Log Connection Strings

Never log connection strings directly:

```rust
// DON'T: Log connection string directly
debug!("Connecting to: {}", connection_string);

// DO: Use redaction
debug!("Connecting to: {}", normalize_connection_string_with_redaction(&connection_string, true));
```

### 5. Validate Keys at Boundaries

Validate keys at the boundary between your application and the cache:

```rust
async fn set_user_cache(user_id: &str, data: &[u8]) -> Result<()> {
    // Validate at the boundary
    validate_redis_key(user_id)?;

    // Now use the key
    cache.set_bytes(user_id, data.to_vec(), None).await
}
```

## Security Audit Log

All security-related events are logged with appropriate levels:

- **WARN**: Invalid key detected, operation skipped
- **WARN**: Invalid Lua script detected, operation rejected
- **WARN**: SCAN pattern too complex, operation rejected
- **DEBUG**: Lock acquired with generated UUID

## Reporting Security Issues

If you discover a security vulnerability, please:
1. Don't open a public issue
2. Email the maintainer at the address in Cargo.toml
3. Provide details of the vulnerability
4. Allow time for a fix before disclosure

## Changelog

### v0.1.3

- Added comprehensive input validation module
- Added Lua script validation (blocks dangerous commands)
- Added SCAN pattern validation (prevents ReDoS)
- Added timeout protection for Lua and SCAN operations
- Added secure UUID-based lock values
- Added connection string password redaction
- Added WAL replay key validation
