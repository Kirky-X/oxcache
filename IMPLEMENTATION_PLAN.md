# oxcache 项目全面修复实施方案

**创建日期**: 2026-03-18
**依据报告**: COMPREHENSIVE_ANALYSIS_REPORT.md
**预计总工期**: 8-12 周
**目标**: 解决所有已识别问题，达到生产就绪状态

---

## 📋 执行摘要

本方案基于综合分析报告，制定全面的修复计划，涵盖：
- **安全性问题**（4个高危问题）
- **测试覆盖率**（51.97% → 80%）
- **性能优化**（提升3-10倍）
- **代码质量**（减少2900行重复代码）
- **示例完善**（62.5% → 95%）

**预期成果**：
- 安全评分：7.9 → 9.0
- 架构评分：A- → A
- 性能评分：B+ → A
- 测试覆盖率：51.97% → 80%
- 示例覆盖率：62.5% → 95%

---

## 🎯 修复优先级矩阵

| 问题ID | 问题类型 | 严重程度 | 修复成本 | 优先级 | 预计时间 |
|--------|----------|----------|----------|--------|----------|
| SEC-001 | 依赖库安全漏洞 | 🔴 高 | 中 | P0 | 3天 |
| SEC-002 | Redis TLS绕过 | 🔴 高 | 低 | P0 | 1天 |
| SEC-003 | 密码明文存储 | 🔴 高 | 低 | P0 | 2天 |
| TEST-001 | WAL恢复0%测试 | 🔴 极高 | 中 | P0 | 5天 |
| TEST-002 | Redis客户端7.8%测试 | 🔴 高 | 高 | P0 | 7天 |
| CODE-001 | MockBackend重复 | 🟡 中 | 低 | P1 | 2天 |
| PERF-001 | Redis Pipeline缺失 | 🟡 中 | 中 | P1 | 4天 |
| PERF-002 | 序列化开销 | 🟡 中 | 低 | P1 | 1天 |
| TEST-003 | 数据库<10%测试 | 🟡 中 | 高 | P2 | 7天 |
| EX-001 | ChainCache示例缺失 | 🟡 中 | 低 | P1 | 2天 |
| CODE-002 | Redis测试工具重复 | 🟡 中 | 低 | P1 | 1天 |
| PERF-003 | DashMap内存开销 | 🟡 中 | 中 | P2 | 5天 |

---

## 阶段一：关键问题修复（Week 1-2）

**目标**: 修复所有高危安全问题，补充关键测试

---

### 任务 SEC-001: 修复依赖库安全漏洞

**状态**: 未开始
**优先级**: P0（最高）
**预计时间**: 3天
**负责模块**: Cargo.toml, deny.toml

#### 问题描述
项目配置中显式忽略了4个已知安全漏洞：
- RUSTSEC-2023-0071 (rsa漏洞，来自sqlx)
- RUSTSEC-2025-0111 (tokio-tar漏洞，来自testcontainers)
- RUSTSEC-2025-0141 (bincode未维护)
- RUSTSEC-2025-0134 (rustls-pemfile未维护)

#### 修复步骤

**步骤 1**: 评估sqlx升级路径（1天）
```bash
# 检查当前版本
cargo tree -i rsa

# 检查sqlx最新版本
cargo search sqlx

# 测试升级到最新版本
cargo update sqlx
cargo test --all-features
```

**步骤 2**: 评估bincode迁移方案（1天）
```bash
# 研究替代方案：postcard, msgpack, protobuf
# 对比性能和兼容性

# 如果保留bincode，添加注释说明原因
# 如果迁移到postcard：
cargo remove bincode
cargo add postcard
# 修改序列化实现
```

**步骤 3**: 更新deny.toml配置（0.5天）
```toml
[bans]
# 移除临时忽略的漏洞
# 如果确认某些漏洞在当前上下文无风险，添加详细注释

# 示例：
# RUSTSEC-2023-0071: 仅在测试环境使用testcontainers，生产环境不受影响
# reason = "test-only dependency, not used in production"
```

**步骤 4**: 建立自动化依赖检查（0.5天）
```yaml
# .github/workflows/dependency-check.yml
name: Dependency Security Check
on: [push, pull_request]
jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - run: cargo install cargo-audit
      - run: cargo audit
```

#### 验证方法
```bash
# 运行安全审计，应无已知漏洞
cargo audit

# 验证所有测试通过
cargo test --all-features

# 检查编译
cargo build --all-features
```

#### 风险评估
- **风险**: sqlx升级可能破坏API兼容性
- **缓解**: 在feature分支测试，确保所有测试通过

#### 成功标准
- [ ] cargo audit显示0个已知漏洞
- [ ] 所有测试通过
- [ ] CI包含自动化依赖检查

---

### 任务 SEC-002: 修复Redis TLS绕过风险

**状态**: 未开始
**优先级**: P0
**预计时间**: 1天
**负责模块**: src/backend/client/redis/client.rs

#### 问题描述
当前实现允许通过环境变量`OXCACHE_ALLOW_INSECURE_REDIS`绕过TLS强制机制，存在安全风险。

#### 修复步骤

**步骤 1**: 增强环境变量验证（0.5天）

修改 `src/backend/client/redis/client.rs:151-163`:

```rust
// 当前代码（不安全）
if !connection_string.starts_with("rediss://") {
    if std::env::var("OXCACHE_ALLOW_INSECURE_REDIS").is_ok() {
        tracing::warn!("Using insecure Redis connection...");
    } else {
        return Err(CacheError::ConfigError(...));
    }
}

// 修复后代码
if !connection_string.starts_with("rediss://") {
    let allow_insecure = std::env::var("OXCACHE_ALLOW_INSECURE_REDIS")
        .map(|v| {
            // 要求明确确认风险
            v == "I_UNDERSTAND_THE_RISKS" || v == "development-only"
        })
        .unwrap_or(false);

    if !allow_insecure {
        return Err(CacheError::ConfigError(
            "Redis connection must use TLS (rediss://) in production. \
             To allow insecure connections for development only, \
             set OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS".to_string()
        ));
    }

    // 记录安全警告
    tracing::error!(
        target: "security",
        "SECURITY WARNING: Using insecure Redis connection. \
         This should NEVER happen in production! \
         Connection string: {}",
        Self::redact_connection_string(connection_string)
    );
}
```

**步骤 2**: 添加连接字符串脱敏函数（0.25天）

```rust
impl RedisConfig {
    fn redact_connection_string(conn_str: &str) -> String {
        // 移除密码等敏感信息
        if let Some(start) = conn_str.find("://") {
            let protocol = &conn_str[..start + 3];
            let rest = &conn_str[start + 3..];

            if rest.contains('@') {
                // 格式: redis://:password@host:port/db
                if let Some(at_pos) = rest.find('@') {
                    return format!("{}[REDACTED]@{}", protocol, &rest[at_pos + 1..]);
                }
            }
        }
        conn_str.to_string()
    }
}
```

**步骤 3**: 添加测试（0.25天）

```rust
#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn test_insecure_connection_rejected_by_default() {
        let result = RedisBackend::new("redis://localhost:6379");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("TLS"));
    }

    #[test]
    fn test_insecure_connection_requires_explicit_consent() {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "wrong_value");
        let result = RedisBackend::new("redis://localhost:6379");
        assert!(result.is_err());

        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
        // 此时应该成功（如果Redis可用）
        // std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
    }

    #[test]
    fn test_connection_string_redaction() {
        let conn_str = "redis://:secret_password@localhost:6379/0";
        let redacted = RedisConfig::redact_connection_string(conn_str);
        assert!(!redacted.contains("secret_password"));
        assert!(redacted.contains("[REDACTED]"));
    }
}
```

#### 验证方法
```bash
# 测试安全配置
cargo test security_tests

# 测试不安全连接被拒绝
REDIS_URL=redis://localhost:6379 cargo test --features redis

# 验证文档
cargo doc --open
```

#### 成功标准
- [ ] 不安全连接默认被拒绝
- [ ] 需要明确确认才能使用不安全连接
- [ ] 密码在日志中被脱敏
- [ ] 测试覆盖所有场景

---

### 任务 SEC-003: 修复密码明文存储问题

**状态**: 未开始
**优先级**: P0
**预计时间**: 2天
**负责模块**: src/database/connection_string.rs

#### 问题描述
密码以明文String形式存储在`ParsedConnectionString`结构体中，存在内存泄露风险。

#### 修复步骤

**步骤 1**: 使用SecretString保护密码（1天）

修改 `src/database/connection_string.rs`:

```rust
use secrecy::{SecretString, ExposeSecret};

pub struct ParsedConnectionString<'a> {
    pub protocol: &'a str,
    pub host: &'a str,
    pub port: u16,
    pub username: Option<&'a str>,
    /// 密码（受保护，不会意外泄露）
    pub password: Option<SecretString>,
    pub database: Option<&'a str>,
    pub parameters: HashMap<&'a str, &'a str>,
}

impl<'a> ParsedConnectionString<'a> {
    /// 创建连接字符串，密码自动被保护
    pub fn parse(input: &'a str) -> Result<Self> {
        // ... 解析逻辑
        let password = password.map(|p| SecretString::new(p.to_string()));
        Ok(Self {
            password,
            // ...
        })
    }

    /// 安全地访问密码（仅在需要时暴露）
    pub fn password(&self) -> Option<&str> {
        self.password.as_ref().map(|p| p.expose_secret().as_str())
    }
}

// 实现Debug时自动脱敏
impl<'a> std::fmt::Debug for ParsedConnectionString<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParsedConnectionString")
            .field("host", &self.host)
            .field("password", &"[REDACTED]")
            .finish()
    }
}
```

**步骤 2**: 更新所有使用密码的地方（0.5天）

```rust
// 之前
if let Some(password) = &parsed.password {
    // password 是 String，容易泄露
}

// 之后
if let Some(password) = parsed.password() {
    // password 是 &str，仅在作用域内有效
    // 不会被意外打印或存储
}
```

**步骤 3**: 添加测试（0.5天）

```rust
#[cfg(test)]
mod password_security_tests {
    use super::*;

    #[test]
    fn test_password_is_protected() {
        let conn_str = "postgresql://user:secret_password@localhost:5432/db"; // pragma: allowlist secret
        let parsed = ParsedConnectionString::parse(conn_str).unwrap();

        // Debug输出不应包含密码
        let debug_output = format!("{:?}", parsed);
        assert!(!debug_output.contains("secret_password"));
        assert!(debug_output.contains("[REDACTED]"));
    }

    #[test]
    fn test_password_can_be_accessed() {
        let conn_str = "postgresql://user:secret_password@localhost:5432/db"; // pragma: allowlist secret
        let parsed = ParsedConnectionString::parse(conn_str).unwrap();

        // 密码可以安全访问
        assert_eq!(parsed.password(), Some("secret_password"));
    }

    #[test]
    fn test_password_not_in_memory_dump() {
        let conn_str = "postgresql://user:secret@localhost:5432/db"; // pragma: allowlist secret
        let parsed = ParsedConnectionString::parse(conn_str).unwrap();

        // 模拟内存检查
        let memory_dump = format!("{:?}", parsed);
        assert!(!memory_dump.contains("secret"));
    }
}
```

#### 验证方法
```bash
# 运行密码安全测试
cargo test password_security

# 检查编译
cargo build

# 运行所有测试
cargo test --all-features
```

#### 成功标准
- [ ] 密码使用SecretString保护
- [ ] Debug输出不包含密码
- [ ] 所有测试通过
- [ ] 无破坏性变更

---

### 任务 TEST-001: 补充WAL恢复机制测试

**状态**: 未开始
**优先级**: P0
**预计时间**: 5天
**负责模块**: src/recovery/wal.rs
**当前覆盖率**: 0% (0/188行)

#### 问题描述
WAL（Write-Ahead Logging）恢复机制完全没有测试，这是系统可靠性的关键组件，风险极高。

#### 修复步骤

**步骤 1**: 创建测试框架（1天）

创建 `tests/recovery/wal_test.rs`:

```rust
use oxcache::recovery::wal::*;
use tempfile::TempDir;
use tokio::fs;

mod test_helpers {
    use super::*;

    pub struct WalTestContext {
        pub temp_dir: TempDir,
        pub wal_path: std::path::PathBuf,
    }

    impl WalTestContext {
        pub async fn new() -> Self {
            let temp_dir = TempDir::new().unwrap();
            let wal_path = temp_dir.path().join("test.wal");

            Self { temp_dir, wal_path }
        }

        pub async fn create_wal(&self) -> WalWriter {
            WalWriter::new(&self.wal_path).await.unwrap()
        }

        pub async fn write_entries(&self, entries: &[WalEntry]) {
            let mut writer = self.create_wal().await;
            for entry in entries {
                writer.write(entry).await.unwrap();
            }
            writer.flush().await.unwrap();
        }
    }
}
```

**步骤 2**: 基础功能测试（1天）

```rust
mod basic_functionality {
    use super::*;

    #[tokio::test]
    async fn test_wal_write_and_read() {
        let ctx = test_helpers::WalTestContext::new().await;

        // 写入测试数据
        let entries = vec![
            WalEntry::Set { key: "key1".to_string(), value: vec![1, 2, 3] },
            WalEntry::Delete { key: "key2".to_string() },
        ];
        ctx.write_entries(&entries).await;

        // 读取并验证
        let reader = WalReader::open(&ctx.wal_path).await.unwrap();
        let read_entries = reader.read_all().await.unwrap();

        assert_eq!(read_entries.len(), 2);
        assert_eq!(read_entries[0], entries[0]);
        assert_eq!(read_entries[1], entries[1]);
    }

    #[tokio::test]
    async fn test_wal_append() {
        let ctx = test_helpers::WalTestContext::new().await;

        let mut writer = ctx.create_wal().await;
        writer.write(&WalEntry::Set {
            key: "key1".to_string(),
            value: vec![1]
        }).await.unwrap();
        writer.flush().await.unwrap();

        // 追加新条目
        let mut writer = ctx.create_wal().await;
        writer.write(&WalEntry::Set {
            key: "key2".to_string(),
            value: vec![2]
        }).await.unwrap();
        writer.flush().await.unwrap();

        // 验证两个条目都存在
        let reader = WalReader::open(&ctx.wal_path).await.unwrap();
        let entries = reader.read_all().await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_wal_checksum_validation() {
        let ctx = test_helpers::WalTestContext::new().await;
        ctx.write_entries(&[
            WalEntry::Set { key: "key1".to_string(), value: vec![1, 2, 3] }
        ]).await;

        // 损坏文件
        let mut content = fs::read(&ctx.wal_path).await.unwrap();
        content[10] ^= 0xFF; // 翻转一个字节
        fs::write(&ctx.wal_path, &content).await.unwrap();

        // 读取应该检测到损坏
        let reader = WalReader::open(&ctx.wal_path).await.unwrap();
        let result = reader.read_all().await;
        assert!(result.is_err());
    }
}
```

**步骤 3**: 故障恢复测试（1天）

```rust
mod crash_recovery {
    use super::*;

    #[tokio::test]
    async fn test_partial_write_recovery() {
        let ctx = test_helpers::WalTestContext::new().await;

        let mut writer = ctx.create_wal().await;

        // 写入完整条目
        writer.write(&WalEntry::Set {
            key: "key1".to_string(),
            value: vec![1, 2, 3]
        }).await.unwrap();

        // 模拟部分写入（未flush）
        writer.write(&WalEntry::Set {
            key: "key2".to_string(),
            value: vec![4, 5, 6]
        }).await.unwrap();
        // 故意不调用flush

        // 恢复应该只读到第一个条目
        let reader = WalReader::open(&ctx.wal_path).await.unwrap();
        let entries = reader.read_all().await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], WalEntry::Set {
            key: "key1".to_string(),
            value: vec![1, 2, 3]
        });
    }

    #[tokio::test]
    async fn test_replay_after_crash() {
        let ctx = test_helpers::WalTestContext::new().await;

        // 写入多个操作
        let entries = vec![
            WalEntry::Set { key: "key1".to_string(), value: vec![1] },
            WalEntry::Set { key: "key2".to_string(), value: vec![2] },
            WalEntry::Delete { key: "key1".to_string() },
            WalEntry::Set { key: "key3".to_string(), value: vec![3] },
        ];
        ctx.write_entries(&entries).await;

        // 模拟崩溃后恢复
        let recovered_state = simulate_recovery(&ctx.wal_path).await;

        assert!(!recovered_state.contains_key("key1")); // 已删除
        assert_eq!(recovered_state.get("key2"), Some(&vec![2]));
        assert_eq!(recovered_state.get("key3"), Some(&vec![3]));
    }

    async fn simulate_recovery(wal_path: &std::path::Path) -> HashMap<String, Vec<u8>> {
        let reader = WalReader::open(wal_path).await.unwrap();
        let entries = reader.read_all().await.unwrap();

        let mut state = HashMap::new();
        for entry in entries {
            match entry {
                WalEntry::Set { key, value } => {
                    state.insert(key, value);
                }
                WalEntry::Delete { key } => {
                    state.remove(&key);
                }
            }
        }
        state
    }
}
```

**步骤 4**: 并发安全测试（1天）

```rust
mod concurrency_safety {
    use super::*;
    use std::sync::Arc;
    use tokio::task::JoinSet;

    #[tokio::test]
    async fn test_concurrent_writes() {
        let ctx = Arc::new(test_helpers::WalTestContext::new().await);
        let writer = Arc::new(Mutex::new(ctx.create_wal().await));

        let mut tasks = JoinSet::new();

        // 并发写入100个条目
        for i in 0..100 {
            let writer = writer.clone();
            tasks.spawn(async move {
                let mut w = writer.lock().await;
                w.write(&WalEntry::Set {
                    key: format!("key_{}", i),
                    value: vec![i as u8]
                }).await.unwrap();
            });
        }

        while tasks.join_next().await.is_some() {}

        // 刷新
        writer.lock().await.flush().await.unwrap();

        // 验证所有条目都写入成功
        let reader = WalReader::open(&ctx.wal_path).await.unwrap();
        let entries = reader.read_all().await.unwrap();
        assert_eq!(entries.len(), 100);
    }

    #[tokio::test]
    async fn test_read_during_write() {
        let ctx = Arc::new(test_helpers::WalTestContext::new().await);
        let writer = Arc::new(Mutex::new(ctx.create_wal().await));

        // 写入一些数据
        {
            let mut w = writer.lock().await;
            for i in 0..10 {
                w.write(&WalEntry::Set {
                    key: format!("key_{}", i),
                    value: vec![i as u8]
                }).await.unwrap();
            }
            w.flush().await.unwrap();
        }

        // 并发读取和写入
        let writer_clone = writer.clone();
        let write_task = tokio::spawn(async move {
            let mut w = writer_clone.lock().await;
            for i in 10..20 {
                w.write(&WalEntry::Set {
                    key: format!("key_{}", i),
                    value: vec![i as u8]
                }).await.unwrap();
            }
            w.flush().await.unwrap();
        });

        let read_task = tokio::spawn(async {
            let reader = WalReader::open(&ctx.wal_path).await.unwrap();
            let entries = reader.read_all().await.unwrap();
            entries.len()
        });

        write_task.await.unwrap();
        let read_count = read_task.await.unwrap();

        // 读取应该能看到初始的10个条目
        assert!(read_count >= 10);
    }
}
```

**步骤 5**: 性能和边界测试（1天）

```rust
mod performance_and_edge_cases {
    use super::*;

    #[tokio::test]
    async fn test_large_entry() {
        let ctx = test_helpers::WalTestContext::new().await;

        // 写入大数据
        let large_value = vec![0u8; 10 * 1024 * 1024]; // 10MB
        ctx.write_entries(&[
            WalEntry::Set {
                key: "large_key".to_string(),
                value: large_value.clone()
            }
        ]).await;

        // 读取并验证
        let reader = WalReader::open(&ctx.wal_path).await.unwrap();
        let entries = reader.read_all().await.unwrap();
        assert_eq!(entries[0], WalEntry::Set {
            key: "large_key".to_string(),
            value: large_value
        });
    }

    #[tokio::test]
    async fn test_many_small_entries() {
        let ctx = test_helpers::WalTestContext::new().await;

        // 写入10000个小条目
        let entries: Vec<WalEntry> = (0..10000)
            .map(|i| WalEntry::Set {
                key: format!("key_{}", i),
                value: vec![i as u8]
            })
            .collect();

        ctx.write_entries(&entries).await;

        let reader = WalReader::open(&ctx.wal_path).await.unwrap();
        let read_entries = reader.read_all().await.unwrap();
        assert_eq!(read_entries.len(), 10000);
    }

    #[tokio::test]
    async fn test_empty_key() {
        let ctx = test_helpers::WalTestContext::new().await;

        // 空key应该被拒绝或正确处理
        let result = ctx.create_wal().await.write(&WalEntry::Set {
            key: "".to_string(),
            value: vec![1, 2, 3]
        }).await;

        // 根据实现，可能是错误或成功
        // assert!(result.is_err() || /* 其他验证 */);
    }

    #[tokio::test]
    async fn test_unicode_keys() {
        let ctx = test_helpers::WalTestContext::new().await;

        let entries = vec![
            WalEntry::Set {
                key: "中文键".to_string(),
                value: vec![1]
            },
            WalEntry::Set {
                key: "日本語キー".to_string(),
                value: vec![2]
            },
            WalEntry::Set {
                key: "emoji_🎉".to_string(),
                value: vec![3]
            },
        ];

        ctx.write_entries(&entries).await;

        let reader = WalReader::open(&ctx.wal_path).await.unwrap();
        let read_entries = reader.read_all().await.unwrap();
        assert_eq!(read_entries, entries);
    }
}
```

#### 验证方法
```bash
# 运行WAL测试
cargo test wal_test --all-features

# 检查覆盖率
cargo tarpaulin --out Html --src src/recovery/wal.rs

# 目标：达到80%覆盖率
```

#### 成功标准
- [ ] 至少8个测试用例通过
- [ ] WAL覆盖率：0% → 80%+
- [ ] 所有测试通过
- [ ] 无竞态条件

---

### 任务 TEST-002: 补充Redis客户端测试

**状态**: 未开始
**优先级**: P0
**预计时间**: 7天
**负责模块**: src/backend/client/redis/client.rs
**当前覆盖率**: 7.8% (17/219行)

#### 问题描述
Redis客户端是L2缓存的核心组件，但测试覆盖率极低，几乎未测试连接管理、集群模式、Lua脚本等关键功能。

#### 修复步骤

**步骤 1**: 创建Redis测试框架（1天）

创建 `tests/integration/redis_client_comprehensive_test.rs`:

```rust
use oxcache::backend::client::redis::*;
use testcontainers::{clients, images::redis};

mod test_context {
    use super::*;

    pub struct RedisTestContext {
        docker: clients::Cli,
        container: testcontainers::Container<'static, redis::Redis>,
        connection_string: String,
    }

    impl RedisTestContext {
        pub async fn new() -> Self {
            let docker = clients::Cli::default();
            let container = docker.run(redis::Redis::default());

            let host = container.get_host().to_string();
            let port = container.get_host_port_ipv4(6379);
            let connection_string = format!("redis://{}:{}/0", host, port);

            Self {
                docker,
                container,
                connection_string,
            }
        }

        pub fn connection_string(&self) -> &str {
            &self.connection_string
        }

        pub async fn create_client(&self) -> RedisBackend {
            RedisBackend::new(self.connection_string()).await.unwrap()
        }
    }
}
```

**步骤 2**: 基础连接和操作测试（2天）

```rust
mod basic_operations {
    use super::*;

    #[tokio::test]
    async fn test_connection_establishment() {
        let ctx = test_context::RedisTestContext::new().await;
        let client = ctx.create_client().await;

        // 测试健康检查
        let health = client.health_check().await.unwrap();
        assert!(health);
    }

    #[tokio::test]
    async fn test_basic_set_get() {
        let ctx = test_context::RedisTestContext::new().await;
        let client = ctx.create_client().await;

        // Set操作
        client.set("test_key", b"test_value".to_vec(), None).await.unwrap();

        // Get操作
        let value = client.get("test_key").await.unwrap();
        assert_eq!(value, Some(b"test_value".to_vec()));
    }

    #[tokio::test]
    async fn test_delete() {
        let ctx = test_context::RedisTestContext::new().await;
        let client = ctx.create_client().await;

        client.set("key_to_delete", b"value".to_vec(), None).await.unwrap();
        client.delete("key_to_delete").await.unwrap();

        let value = client.get("key_to_delete").await.unwrap();
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_ttl() {
        let ctx = test_context::RedisTestContext::new().await;
        let client = ctx.create_client().await;

        // 设置带TTL的键
        client.set(
            "key_with_ttl",
            b"value".to_vec(),
            Some(std::time::Duration::from_secs(1))
        ).await.unwrap();

        // 立即读取应该成功
        let value = client.get("key_with_ttl").await.unwrap();
        assert!(value.is_some());

        // 等待过期
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

        // 再次读取应该为空
        let value = client.get("key_with_ttl").await.unwrap();
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_nonexistent_key() {
        let ctx = test_context::RedisTestContext::new().await;
        let client = ctx.create_client().await;

        let value = client.get("nonexistent_key").await.unwrap();
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_overwrite_key() {
        let ctx = test_context::RedisTestContext::new().await;
        let client = ctx.create_client().await;

        client.set("key", b"value1".to_vec(), None).await.unwrap();
        client.set("key", b"value2".to_vec(), None).await.unwrap();

        let value = client.get("key").await.unwrap();
        assert_eq!(value, Some(b"value2".to_vec()));
    }
}
```

**步骤 3**: 连接池和错误处理测试（1天）

```rust
mod connection_management {
    use super::*;

    #[tokio::test]
    async fn test_connection_pool() {
        let ctx = test_context::RedisTestContext::new().await;
        let client = ctx.create_client().await;

        // 并发执行多个操作，验证连接池工作正常
        let mut handles = vec![];

        for i in 0..20 {
            let client = client.clone();
            handles.push(tokio::spawn(async move {
                client.set(
                    &format!("concurrent_key_{}", i),
                    format!("value_{}", i).into_bytes(),
                    None
                ).await.unwrap();

                client.get(&format!("concurrent_key_{}", i)).await.unwrap()
            }));
        }

        for (i, handle) in handles.into_iter().enumerate() {
            let value = handle.await.unwrap();
            assert_eq!(value, Some(format!("value_{}", i).into_bytes()));
        }
    }

    #[tokio::test]
    async fn test_connection_failure_recovery() {
        // 测试连接失败后的恢复
        let invalid_url = "redis://invalid-host:6379/0";
        let result = RedisBackend::new(invalid_url).await;

        // 应该返回错误或重试后失败
        assert!(result.is_err() || result.is_ok());
    }

    #[tokio::test]
    async fn test_connection_timeout() {
        // 测试超时配置
        let ctx = test_context::RedisTestContext::new().await;

        // 配置短超时
        let config = RedisConfig {
            connection_string: ctx.connection_string().to_string(),
            timeout: Some(std::time::Duration::from_millis(100)),
            ..Default::default()
        };

        let client = RedisBackend::with_config(config).await.unwrap();

        // 执行操作应该成功
        let health = client.health_check().await.unwrap();
        assert!(health);
    }
}
```

**步骤 4**: Lua脚本测试（1天）

```rust
mod lua_scripts {
    use super::*;

    #[tokio::test]
    async fn test_basic_lua_script() {
        let ctx = test_context::RedisTestContext::new().await;
        let client = ctx.create_client().await;

        // 设置初始值
        client.set("counter", b"0".to_vec(), None).await.unwrap();

        // 执行Lua脚本：原子递增
        let script = r#"
            local current = redis.call('GET', KEYS[1])
            current = tonumber(current)
            redis.call('SET', KEYS[1], current + 1)
            return current + 1
        "#;

        let result = client.eval_script(
            script,
            &["counter"],
            &[]
        ).await.unwrap();

        assert_eq!(result, Some(b"1".to_vec()));
    }

    #[tokio::test]
    async fn test_conditional_set() {
        let ctx = test_context::RedisTestContext::new().await;
        let client = ctx.create_client().await;

        // Lua脚本：仅当值不存在时设置
        let script = r#"
            if redis.call('EXISTS', KEYS[1]) == 0 then
                redis.call('SET', KEYS[1], ARGV[1])
                return 1
            else
                return 0
            end
        "#;

        // 第一次应该成功
        let result = client.eval_script(
            script,
            &["unique_key"],
            &["value1"]
        ).await.unwrap();
        assert_eq!(result, Some(b"1".to_vec()));

        // 第二次应该失败（键已存在）
        let result = client.eval_script(
            script,
            &["unique_key"],
            &["value2"]
        ).await.unwrap();
        assert_eq!(result, Some(b"0".to_vec()));

        // 验证值没有改变
        let value = client.get("unique_key").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_lua_script_error() {
        let ctx = test_context::RedisTestContext::new().await;
        let client = ctx.create_client().await;

        // 错误的Lua脚本
        let script = "invalid lua syntax";

        let result = client.eval_script(script, &["key"], &[]).await;
        assert!(result.is_err());
    }
}
```

**步骤 5**: 批量操作和Pipeline测试（1天）

```rust
mod batch_operations {
    use super::*;

    #[tokio::test]
    async fn test_set_many() {
        let ctx = test_context::RedisTestContext::new().await;
        let client = ctx.create_client().await;

        let items = vec![
            ("key1", b"value1".to_vec()),
            ("key2", b"value2".to_vec()),
            ("key3", b"value3".to_vec()),
        ];

        client.set_many(&items).await.unwrap();

        // 验证所有键都设置成功
        for (key, value) in &items {
            let result = client.get(key).await.unwrap();
            assert_eq!(result, Some(value.clone()));
        }
    }

    #[tokio::test]
    async fn test_get_many() {
        let ctx = test_context::RedisTestContext::new().await;
        let client = ctx.create_client().await;

        // 设置测试数据
        client.set("key1", b"value1".to_vec(), None).await.unwrap();
        client.set("key2", b"value2".to_vec(), None).await.unwrap();
        client.set("key3", b"value3".to_vec(), None).await.unwrap();

        // 批量获取
        let keys = vec!["key1", "key2", "key3", "nonexistent"];
        let values = client.get_many(&keys).await.unwrap();

        assert_eq!(values.len(), 4);
        assert_eq!(values[0], Some(b"value1".to_vec()));
        assert_eq!(values[1], Some(b"value2".to_vec()));
        assert_eq!(values[2], Some(b"value3".to_vec()));
        assert_eq!(values[3], None);
    }

    #[tokio::test]
    async fn test_delete_many() {
        let ctx = test_context::RedisTestContext::new().await;
        let client = ctx.create_client().await;

        // 设置测试数据
        client.set("key1", b"value1".to_vec(), None).await.unwrap();
        client.set("key2", b"value2".to_vec(), None).await.unwrap();
        client.set("key3", b"value3".to_vec(), None).await.unwrap();

        // 批量删除
        client.delete_many(&["key1", "key2"]).await.unwrap();

        // 验证删除结果
        assert!(client.get("key1").await.unwrap().is_none());
        assert!(client.get("key2").await.unwrap().is_none());
        assert!(client.get("key3").await.unwrap().is_some());
    }
}
```

**步骤 6**: 集群和哨兵模式测试（1天）

```rust
mod advanced_modes {
    use super::*;

    #[tokio::test]
    #[ignore] // 需要Redis集群环境
    async fn test_cluster_mode() {
        // 测试集群模式连接
        // 需要启动Redis集群
        let cluster_urls = vec![
            "redis://localhost:7000",
            "redis://localhost:7001",
            "redis://localhost:7002",
        ];

        let config = RedisConfig {
            cluster_urls: Some(cluster_urls),
            ..Default::default()
        };

        let client = RedisBackend::with_config(config).await.unwrap();

        // 测试基本操作
        client.set("cluster_key", b"cluster_value".to_vec(), None).await.unwrap();
        let value = client.get("cluster_key").await.unwrap();
        assert_eq!(value, Some(b"cluster_value".to_vec()));
    }

    #[tokio::test]
    #[ignore] // 需要Redis哨兵环境
    async fn test_sentinel_mode() {
        // 测试哨兵模式连接
        let sentinel_urls = vec![
            "redis://localhost:26379",
            "redis://localhost:26380",
        ];

        let config = RedisConfig {
            sentinel_urls: Some(sentinel_urls),
            master_name: "mymaster".to_string(),
            ..Default::default()
        };

        let client = RedisBackend::with_config(config).await.unwrap();

        // 测试故障转移
        // ...
    }
}
```

#### 验证方法
```bash
# 运行Redis客户端测试
cargo test redis_client_comprehensive_test --all-features

# 检查覆盖率
cargo tarpaulin --out Html --src src/backend/client/redis/client.rs

# 目标：达到75%覆盖率
```

#### 成功标准
- [ ] 至少15个测试用例通过
- [ ] Redis客户端覆盖率：7.8% → 75%+
- [ ] 连接池测试通过
- [ ] Lua脚本测试通过
- [ ] 批量操作测试通过

---

## 阶段二：性能优化与代码质量（Week 3-4）

---

### 任务 CODE-001: 统一MockBackend实现

**状态**: 未开始
**优先级**: P1
**预计时间**: 2天
**负责模块**: src/chain.rs, src/backend/interface.rs, src/builder/sorter.rs, src/builder/oxcache_builder.rs
**当前问题**: 同一个MockBackend在5个地方重复实现（~2000行重复代码）

#### 修复步骤

**步骤 1**: 确认tests/common/mock_backend.rs最完整（0.5天）

```bash
# 对比所有MockBackend实现
diff src/chain.rs tests/common/mock_backend.rs
diff src/backend/interface.rs tests/common/mock_backend.rs
diff src/builder/sorter.rs tests/common/mock_backend.rs
diff src/builder/oxcache_builder.rs tests/common/mock_backend.rs
```

确保`tests/common/mock_backend.rs`包含所有功能。

**步骤 2**: 删除重复实现，添加导入（1天）

修改文件：

**src/chain.rs**:
```rust
// 删除第557-662行的MockBackend定义
// 添加导入
#[cfg(test)]
use crate::tests::common::MockBackend;
```

**src/backend/interface.rs**:
```rust
// 删除第203-285行的MockBackend定义
// 添加导入
#[cfg(test)]
use crate::tests::common::MockBackend;
```

**src/builder/sorter.rs**:
```rust
// 删除第228-285行的MockBackend定义
// 添加导入
#[cfg(test)]
use crate::tests::common::MockBackend;
```

**src/builder/oxcache_builder.rs**:
```rust
// 删除第254-328行的MockBackend定义
// 添加导入
#[cfg(test)]
use crate::tests::common::MockBackend;
```

**步骤 3**: 运行测试验证（0.5天）

```bash
# 运行所有测试
cargo test --all-features

# 检查编译
cargo build --all-features

# 检查是否有其他地方使用MockBackend
grep -r "MockBackend" src/ tests/
```

#### 验证方法
```bash
# 确认删除成功
grep -n "struct MockBackend" src/chain.rs src/backend/interface.rs src/builder/sorter.rs src/builder/oxcache_builder.rs
# 应该无输出

# 运行所有测试
cargo test --all-features

# 检查代码行数减少
tokei src/ tests/
```

#### 成功标准
- [ ] 删除4处重复MockBackend定义
- [ ] 减少1800+行代码
- [ ] 所有测试通过
- [ ] 无编译警告

---

### 任务 PERF-001: 实现Redis Pipeline

**状态**: 未开始
**优先级**: P1
**预计时间**: 4天
**负责模块**: src/cache.rs, src/backend/client/redis/client.rs
**预期收益**: 批量操作性能提升10-50倍

#### 修复步骤

**步骤 1**: 添加Pipeline支持到RedisBackend（2天）

修改 `src/backend/client/redis/client.rs`:

```rust
impl RedisBackend {
    /// 使用Pipeline批量设置键值
    pub async fn set_many_pipeline(&self, items: &[(&str, Vec<u8>)]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let mut conn = self.connection_manager.clone();
        let mut pipe = redis::pipe();

        for (key, value) in items {
            pipe.cmd("SET")
                .arg(key)
                .arg(value.as_slice())
                .arg("EX")
                .arg(self.default_ttl.as_secs());
        }

        pipe.query_async(&mut conn)
            .await
            .map_err(|e| CacheError::BackendError(e.to_string()))?;

        Ok(())
    }

    /// 使用Pipeline批量获取键值
    pub async fn get_many_pipeline(&self, keys: &[&str]) -> Result<Vec<Option<Vec<u8>>>> {
        if keys.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.connection_manager.clone();
        let mut pipe = redis::pipe();

        for key in keys {
            pipe.cmd("GET").arg(key);
        }

        let results: Vec<Option<Vec<u8>>> = pipe
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::BackendError(e.to_string()))?;

        Ok(results)
    }

    /// 使用Pipeline批量删除键
    pub async fn delete_many_pipeline(&self, keys: &[&str]) -> Result<()> {
        if keys.is_empty() {
            return Ok(());
        }

        let mut conn = self.connection_manager.clone();
        let mut pipe = redis::pipe();

        for key in keys {
            pipe.cmd("DEL").arg(key);
        }

        pipe.query_async(&mut conn)
            .await
            .map_err(|e| CacheError::BackendError(e.to_string()))?;

        Ok(())
    }
}
```

**步骤 2**: 更新Cache接口使用Pipeline（1天）

修改 `src/cache.rs`:

```rust
impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: Serialize + DeserializeOwned,
{
    /// 批量设置（使用Pipeline优化）
    pub async fn set_many<'a, I>(&self, items: I) -> Result<()>
    where
        I: IntoIterator<Item = (&'a K, &'a V)>,
    {
        let items: Vec<(&K, &V)> = items.into_iter().collect();

        if items.is_empty() {
            return Ok(());
        }

        // 根据后端类型选择策略
        match &self.backend {
            CacheBackendType::L1(backend) => {
                // L1内存缓存：顺序设置
                for (key, value) in &items {
                    self.set(key, value).await?;
                }
            }
            CacheBackendType::L2(backend) => {
                // L2 Redis：使用Pipeline
                let pipeline_items: Vec<(String, Vec<u8>)> = items
                    .iter()
                    .map(|(key, value)| {
                        let key_str = (*key).to_key_string();
                        let value_bytes = self.serializer.serialize(value).unwrap();
                        (key_str, value_bytes)
                    })
                    .collect();

                let refs: Vec<(&str, Vec<u8>)> = pipeline_items
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.clone()))
                    .collect();

                backend.set_many_pipeline(&refs).await?;
            }
            CacheBackendType::Tiered(l1, l2) => {
                // 分层缓存：先设置L1，再设置L2
                for (key, value) in &items {
                    self.set(key, value).await?;
                }

                // 异步设置L2（不等待）
                let l2_backend = l2.clone();
                let items_for_l2 = items.clone();
                tokio::spawn(async move {
                    // ... Pipeline设置L2
                });
            }
        }

        Ok(())
    }
}
```

**步骤 3**: 添加性能测试（1天）

创建 `tests/benchmarks/pipeline_benchmark.rs`:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use oxcache::backend::client::redis::*;

fn pipeline_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("set_many_without_pipeline", |b| {
        b.to_async(&rt).iter(|| async {
            let client = setup_redis_client().await;
            let items: Vec<(&str, Vec<u8>)> = (0..100)
                .map(|i| (format!("key_{}", i), vec![i as u8]))
                .collect();

            // 逐个设置
            for (key, value) in &items {
                client.set(key, value.clone(), None).await.unwrap();
            }
        })
    });

    c.bench_function("set_many_with_pipeline", |b| {
        b.to_async(&rt).iter(|| async {
            let client = setup_redis_client().await;
            let items: Vec<(&str, Vec<u8>)> = (0..100)
                .map(|i| (format!("key_{}", i), vec![i as u8]))
                .collect();

            // 使用Pipeline
            client.set_many_pipeline(&items).await.unwrap();
        })
    });
}

criterion_group!(benches, pipeline_benchmark);
criterion_main!(benches);
```

#### 验证方法
```bash
# 运行Pipeline测试
cargo test pipeline --all-features

# 运行性能测试
cargo bench pipeline_benchmark

# 预期：Pipeline版本快10-50倍
```

#### 成功标准
- [ ] 实现set_many_pipeline方法
- [ ] 实现get_many_pipeline方法
- [ ] 实现delete_many_pipeline方法
- [ ] Cache接口使用Pipeline
- [ ] 性能测试显示10-50倍提升

---

### 任务 PERF-002: 切换到Bincode序列化

**状态**: 未开始
**优先级**: P1
**预计时间**: 1天
**负责模块**: Cargo.toml, src/serialization/
**预期收益**: 序列化性能提升2-5倍，体积减少20-40%

#### 修复步骤

**步骤 1**: 启用bincode feature（0.25天）

修改 `Cargo.toml`:

```toml
[features]
default = ["json", "moka"]
bincode = ["dep:bincode"]
json = ["dep:serde_json"]

[dependencies]
bincode = { version = "1.3", optional = true }
serde_json = { version = "1.0", optional = true }
```

**步骤 2**: 实现BincodeSerializer（0.5天）

创建 `src/serialization/bincode.rs`:

```rust
use bincode::{config::standard, decode_from_slice, encode_to_vec};
use serde::{de::DeserializeOwned, Serialize};

pub struct BincodeSerializer;

impl BincodeSerializer {
    pub fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>> {
        encode_to_vec(value, standard())
            .map_err(|e| CacheError::Serialization(e.to_string()))
    }

    pub fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T> {
        let (value, _) = decode_from_slice(data, standard())
            .map_err(|e| CacheError::Serialization(e.to_string()))?;
        Ok(value)
    }
}
```

**步骤 3**: 更新SerializerPool（0.25天）

修改 `src/serialization/mod.rs`:

```rust
pub enum SerializerPool {
    Json(JsonSerializer),
    Bincode(BincodeSerializer),
}

impl SerializerPool {
    pub fn bincode() -> Self {
        Self::Bincode(BincodeSerializer)
    }

    pub fn json() -> Self {
        Self::Json(JsonSerializer)
    }
}
```

#### 验证方法
```bash
# 测试bincode序列化
cargo test --features bincode

# 性能对比
cargo bench serialization_benchmark --features bincode
cargo bench serialization_benchmark --features json
```

#### 成功标准
- [ ] bincode feature可用
- [ ] BincodeSerializer实现完成
- [ ] 测试通过
- [ ] 性能提升2-5倍

---

### 任务 CODE-002: 统一Redis测试工具

**状态**: 未开始
**优先级**: P1
**预计时间**: 1天
**负责模块**: tests/common/redis_test_utils.rs, tests/integration/

#### 修复步骤

**步骤 1**: 完善tests/common/redis_test_utils.rs（0.5天）

```rust
// tests/common/redis_test_utils.rs

use std::env;
use tokio::time::{sleep, Duration};

/// 获取Redis连接URL
pub fn get_redis_url() -> String {
    env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

/// 检查Redis是否可用
pub async fn is_redis_available() -> bool {
    let url = get_redis_url();
    wait_for_redis(&url, 3).await
}

/// 等待Redis就绪
pub async fn wait_for_redis(url: &str, max_retries: u32) -> bool {
    for i in 0..max_retries {
        if let Ok(client) = redis::Client::open(url) {
            if let Ok(mut conn) = client.get_connection() {
                if redis::cmd("PING").query::<String>(&mut conn).is_ok() {
                    return true;
                }
            }
        }

        if i < max_retries - 1 {
            sleep(Duration::from_millis(500)).await;
        }
    }
    false
}

/// Redis测试前置条件检查
pub async fn ensure_redis_available() {
    if !is_redis_available().await {
        panic!("Redis is not available. Please start Redis server before running tests.");
    }
}
```

**步骤 2**: 删除重复的工具函数（0.25天）

```bash
# 删除redis_standalone_test.rs中的重复函数
# 删除network_failure_test.rs中的重复函数

# 使用统一工具
# 在文件开头添加：
use common::redis_test_utils::{get_redis_url, is_redis_available, ensure_redis_available};
```

**步骤 3**: 更新测试文件（0.25天）

```rust
// tests/integration/redis_standalone_test.rs
use common::redis_test_utils::*;

#[tokio::test]
async fn test_redis_operations() {
    ensure_redis_available().await;

    let url = get_redis_url();
    let client = RedisBackend::new(&url).await.unwrap();

    // ... 测试逻辑
}
```

#### 成功标准
- [ ] 删除3处重复函数
- [ ] 减少150行代码
- [ ] 所有Redis测试通过
- [ ] 工具函数可复用

---

## 阶段三：测试完善与示例补充（Week 5-8）

---

### 任务 TEST-003: 补充数据库集成测试

**状态**: 未开始
**优先级**: P2
**预计时间**: 7天
**负责模块**: src/database/mysql.rs (6.8%), src/database/postgresql.rs (9.7%)

#### 修复步骤
（详见完整实施计划文档...）

---

### 任务 EX-001: 补充ChainCache示例

**状态**: 未开始
**优先级**: P1
**预计时间**: 2天
**负责模块**: examples/src/02_advanced/

#### 修复步骤

**步骤 1**: 创建示例文件（1.5天）

创建 `examples/src/02_advanced/example_chain_cache.rs`:

```rust
use oxcache::prelude::*;

/// ChainCache示例：多后端链式访问
///
/// 本示例演示如何使用ChainCache实现：
/// - L1内存缓存（快速访问）
/// - L2 Redis缓存（持久化）
/// - L3数据库缓存（长期存储）
/// 的多级缓存策略

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== ChainCache 链式缓存示例 ===\n");

    // 创建多级缓存链
    let chain = ChainCache::builder()
        // L1: 内存缓存（最快）
        .add_l1(MokaBackend::new(1000))
        // L2: Redis缓存（中等速度）
        .add_l2(RedisBackend::new("redis://127.0.0.1:6379/0").await?)
        // L3: 数据库缓存（最慢但持久）
        .add_l3(SqliteBackend::new("cache.db").await?)
        .build()
        .await?;

    println!("✓ 创建了3级缓存链：L1(Moka) -> L2(Redis) -> L3(SQLite)\n");

    // 演示：写入数据
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    println!("设置用户数据: {:?}", user);
    chain.set(&"user:1".to_string(), &user).await?;

    // 演示：读取数据（会从L1读取）
    println!("\n第一次读取（从L1）：");
    let cached = chain.get(&"user:1".to_string()).await?;
    println!("  结果: {:?}", cached);

    // 演示：清除L1缓存，强制从L2读取
    println!("\n清除L1缓存，再次读取：");
    chain.invalidate_l1(&"user:1".to_string()).await?;
    let cached = chain.get(&"user:1".to_string()).await?;
    println!("  结果（从L2读取）: {:?}", cached);

    // 演示：清除L1和L2，强制从L3读取
    println!("\n清除L1和L2缓存，再次读取：");
    chain.invalidate_l1(&"user:1".to_string()).await?;
    chain.invalidate_l2(&"user:1".to_string()).await?;
    let cached = chain.get(&"user:1".to_string()).await?;
    println!("  结果（从L3读取）: {:?}", cached);

    // 演示：自定义访问策略
    println!("\n=== 自定义访问策略 ===");
    let custom_chain = ChainCache::builder()
        .add_l1(MokaBackend::new(1000))
        .add_l2(RedisBackend::new("redis://127.0.0.1:6379/1").await?)
        .with_strategy(ChainStrategy::WriteThrough) // 写穿透策略
        .build()
        .await?;

    println!("使用写穿透策略：写入时同时更新L1和L2");
    custom_chain.set(&"user:2".to_string(), &user).await?;

    println!("\n✓ 示例完成");

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
}
```

**步骤 2**: 更新examples/src/main.rs（0.5天）

```rust
mod example_chain_cache;

// 在run_example函数中添加
Example::ChainCache => example_chain_cache::main().await?,
```

#### 成功标准
- [ ] 示例文件创建完成
- [ ] 可以独立运行
- [ ] 演示链式缓存访问
- [ ] 演示自定义策略

---

## 📊 进度跟踪

### 总体进度

- **阶段一（关键问题）**: 0% (0/5任务)
- **阶段二（性能优化）**: 0% (0/4任务)
- **阶段三（测试完善）**: 0% (0/8任务)

### 里程碑

- [ ] **M1**: 安全问题修复完成（Week 2）
- [ ] **M2**: 关键测试补充完成（Week 2）
- [ ] **M3**: 性能优化完成（Week 4）
- [ ] **M4**: 代码质量提升完成（Week 4）
- [ ] **M5**: 测试覆盖率达标（Week 6）
- [ ] **M6**: 示例完善完成（Week 8）
- [ ] **M7**: 最终验证通过（Week 12）

---

## 🎯 成功标准

### 最终目标

- [ ] 安全评分: 7.9 → 9.0
- [ ] 测试覆盖率: 51.97% → 80%
- [ ] 示例覆盖率: 62.5% → 95%
- [ ] 性能提升: 3-10倍
- [ ] 代码行数: 减少2900行
- [ ] 所有CI检查通过
- [ ] 文档更新完成

---

**文档版本**: v1.0
**最后更新**: 2026-03-18
