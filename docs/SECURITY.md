# 安全策略

本文档描述 oxcache 内置的安全措施、其缓解的威胁模型，以及报告安全漏洞的流程。

## 概述

oxcache 为 Redis 缓存提供纵深防御安全层。所有安全函数通过 `redis` 特性门控，由 `RedisBackend` 自动强制执行。也可以从应用代码中直接调用以进行自定义校验。

## 1. Redis TLS 强制

默认情况下，oxcache 要求 TLS 加密的 Redis 连接（`rediss://` 协议）。非 TLS 连接（`redis://`）在后端构建时被拒绝，并给出明确的错误信息。

### 开发环境绕过

仅在环境变量 `OXCACHE_ALLOW_INSECURE_REDIS` 显式设置为以下值之一时允许非 TLS 连接：

- `I_UNDERSTAND_THE_RISKS`
- `development-only`

```bash
# 仅用于开发 — 切勿在生产环境使用
export OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS
```

> **警告**：在生产环境设置此变量将使 Redis 流量（包括凭据）暴露于网络拦截。应用日志会在绕过激活时记录警告。

## 2. 键校验

**函数**：`oxcache::validate_redis_key(key: &str) -> OxCacheResult<()>`

在 Redis 键发送到服务器前进行校验，防止命令注入和畸形键攻击。

### 规则

| 规则 | 限制 | 说明 |
|------|------|------|
| 非空 | — | 拒绝空键 |
| 最大长度 | 524,288 字节（512 KB） | 拒绝超过 `MAX_KEY_LENGTH` 的键 |
| 危险字符 | `\r`、`\n`、`\0` | 拒绝 CR/LF/NULL（防止 CRLF 注入） |
| 控制字符 | 所有 Unicode 控制字符（`\t` 除外） | 防止二进制/转义序列注入 |

### 示例

```rust
use oxcache::validate_redis_key;

validate_redis_key("user:123")?;           // OK
validate_redis_key("user\r\nSET foo bar")?; // Err — 检测到 CRLF 注入
validate_redis_key("")?;                    // Err — 空键
```

## 3. Lua 脚本沙箱

**函数**：`oxcache::validate_lua_script(script: &str, key_count: usize) -> OxCacheResult<()>`

在 `EVAL`/`EVALSHA` 执行前校验 Lua 脚本，防止服务端资源耗尽和危险命令执行。

### 规则

| 规则 | 限制 | 说明 |
|------|------|------|
| 最大脚本长度 | 10,240 字节（10 KB） | `MAX_LUA_SCRIPT_LENGTH` — 防止内存耗尽 |
| 最大键数量 | 100 个键 | `MAX_LUA_SCRIPT_KEYS` — 防止参数泛滥 |
| 禁止命令 | `FLUSHALL`、`FLUSHDB`、`SHUTDOWN`、`CONFIG`、`KEYS *`、无限循环模式 | 阻止破坏性和资源消耗操作 |

校验器在模式匹配前预处理脚本以剥离注释、字符串字面量和长括号内容，防止通过字符串混淆绕过。

### 示例

```rust
use oxcache::validate_lua_script;

// 安全脚本
validate_lua_script("return redis.call('GET', KEYS[1])", 1)?;

// 拒绝 — FLUSHALL 被禁止
validate_lua_script("redis.call('FLUSHALL')", 0)?;

// 拒绝 — 键过多
validate_lua_script("return 1", 101)?;
```

## 4. SCAN 模式限制

**函数**：`oxcache::validate_scan_pattern(pattern: &str) -> OxCacheResult<()>`
**函数**：`oxcache::clamp_scan_count(count: usize) -> usize`

校验 SCAN 模式并钳制 COUNT 以防止 Redis 服务器过载。

### 规则

| 规则 | 限制 | 说明 |
|------|------|------|
| 最大模式长度 | 256 字符 | `MAX_SCAN_PATTERN_LENGTH` — 防止正则 DoS |
| 最大通配符数 | 10 | `MAX_SCAN_WILDCARDS` — 防止广域扫描 |
| 钳制 COUNT | 1,000 | `clamp_scan_count` 限制 COUNT 防止全键空间扫描 |

### 示例

```rust
use oxcache::{validate_scan_pattern, clamp_scan_count};

validate_scan_pattern("user:*")?;              // OK
validate_scan_pattern("*:*:*:*:*:*:*:*:*:*:*")?; // Err — 通配符过多

let count = clamp_scan_count(1_000_000);       // 返回 1000
```

## 5. 连接字符串脱敏

**函数**：`RedisBackend::redact_connection_string(conn_str: &str) -> String`
**函数**：`oxcache::redact_value(value: &str, visible_chars: usize) -> String`
**函数**：`oxcache::redact_cache_key(key: &str) -> String`
**函数**：`oxcache::redact_field(field_name: &str, value: &str) -> String`

这些函数确保凭据和敏感数据不会出现在日志或错误消息中。

### 示例

```rust
use oxcache::backend::memory::redis::RedisBackend;

let conn_str = "redis://:secret_password@localhost:6379/0";
let redacted = RedisBackend::redact_connection_string(conn_str);
// redacted == "redis://[REDACTED]@localhost:6379/0"
assert!(!redacted.contains("secret_password"));
```

## 6. 日志安全

**函数**：`oxcache::log_cache_key(key: &str) -> String`
**函数**：`oxcache::sanitize_message(msg: &str) -> String`

这些工具确保缓存键和日志消息在写入日志前经过清理，防止日志注入攻击。

## 威胁模型

| 威胁 | 缓解措施 |
|------|----------|
| 通过 Redis 键进行 CRLF 注入 | `validate_redis_key` 拒绝 `\r`、`\n`、`\0` |
| 通过 Lua 脚本进行命令注入 | `validate_lua_script` 阻止 `FLUSHALL`、`CONFIG` 等 |
| 通过 SCAN 进行 Redis 服务器过载 | `validate_scan_pattern` + `clamp_scan_count` |
| 日志中凭据泄露 | `redact_connection_string`、`redact_value` |
| Redis 流量中间人攻击 | TLS 强制（默认 `rediss://`） |
| 通过大型 Lua 脚本进行资源耗尽 | `MAX_LUA_SCRIPT_LENGTH`（10 KB）和 `MAX_LUA_SCRIPT_KEYS`（100） |
| 通过缓存键进行日志注入 | `sanitize_message`、`log_cache_key` |

## 安全报告流程

### 报告漏洞

如果您发现 oxcache 中的安全漏洞：

1. **请勿在 GitHub 上提交公开 issue。**
2. 发送邮件至 `Cargo.toml` `authors` 字段中列出的维护者邮箱。
3. 包含以下内容：
   - 漏洞描述
   - 复现步骤（概念验证）
   - 受影响版本
   - 建议修复方案（如有）
4. 您将在 48 小时内收到确认回复。
5. 修复将按照负责任的安全披露时间线开发和发布。

### 支持的版本

仅最新次要版本接收安全更新。当新的次要版本发布后，前一个次要版本仅在 30 天内接收关键修复。

## 配置摘要

| 设置 | 默认值 | 覆盖方式 |
|------|--------|----------|
| Redis TLS | 必需（`rediss://`） | `OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS` |
| 最大键长度 | 512 KB | 硬编码（`MAX_KEY_LENGTH`） |
| 最大 Lua 脚本长度 | 10 KB | 硬编码（`MAX_LUA_SCRIPT_LENGTH`） |
| 最大 Lua 脚本键数 | 100 | 硬编码（`MAX_LUA_SCRIPT_KEYS`） |
| 最大 SCAN 模式长度 | 256 字符 | 硬编码（`MAX_SCAN_PATTERN_LENGTH`） |
| 最大 SCAN 通配符数 | 10 | 硬编码（`MAX_SCAN_WILDCARDS`） |
| 钳制 SCAN COUNT | 1,000 | `clamp_scan_count()` |
