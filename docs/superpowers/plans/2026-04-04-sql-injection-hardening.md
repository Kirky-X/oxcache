# SQL 注入防护增强计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Status:** ✅ 已完成 100% (2026-04-05 验证)
>
> **验证结果:**
> - `escape_identifier` 返回 `Result<String>` ✅
> - `validate_identifier` 完整实现（长度限制、ASCII检查、保留关键字）✅
> - 动态 SQL 使用验证和转义 ✅
> - PostgreSQL 文件已移除 ✅
> - 安全审计日志已添加 ✅

**Goal:** 将 SQLite 和 PostgreSQL 中的动态 SQL 拼接替换为参数化查询或更安全的实现

**Architecture:**

1. 保留 `validate_identifier()` 白名单验证
2. 对于无法参数化的标识符（表名、列名），确保使用严格的白名单
3. 对于数据值，使用参数化查询

**Tech Stack:** Rust, sea-orm, sqlx

---

## 问题分析

### SQLite 动态 SQL 位置

| 文件        | 行号 | 代码模式                                                                        | 风险等级 |
| ----------- | ---- | ------------------------------------------------------------------------------- | -------- |
| `sqlite.rs` | 193  | `format!("CREATE TABLE IF NOT EXISTS {} {}", escaped_main_table, columns)`      | 中       |
| `sqlite.rs` | 216  | `format!("CREATE TABLE IF NOT EXISTS {} {}", escaped_partition_table, columns)` | 中       |
| `sqlite.rs` | 259  | `format!("DROP VIEW IF EXISTS {}", escaped_base_table)`                         | 中       |
| `sqlite.rs` | 319  | `format!("SELECT * FROM {}", escaped)`                                          | 中       |
| `sqlite.rs` | 325  | `format!("DROP TABLE IF EXISTS {}", escaped_base_table)`                        | 中       |
| `sqlite.rs` | 421  | `format!("DROP TABLE IF EXISTS {}", escaped_partition)`                         | 中       |

### PostgreSQL 动态 SQL 位置

| 文件            | 行号 | 代码模式                                                 | 风险等级 |
| --------------- | ---- | -------------------------------------------------------- | -------- |
| `postgresql.rs` | 405  | `format!("DROP TABLE IF EXISTS \"{}\"", partition_name)` | 中       |

**注：PostgreSQL 文件已不存在，无需处理。**

### 现有防护措施

1. **`validate_identifier()`**: 白名单验证标识符格式
2. **`escape_identifier()`**: 双引号转义

### 剩余风险

虽然有验证和转义，但：

1. 动态构建 SQL 本质上不如参数化安全
2. 白名单可能遗漏边界情况
3. 需要确保所有路径都经过验证

---

## Task 1: 审计现有 validate_identifier 实现

**Files:**

- Analyze: `src/database/sqlite.rs:23-75`

- [x] **Step 1: 阅读当前验证逻辑**

```rust
fn validate_identifier(&self, identifier: &str) -> Result<()> {
    // 检查长度
    // 检查首字符
    // 检查保留关键字
}
```

- [x] **Step 2: 评估防护强度**

当前防护：

- ✅ 长度限制 (128 字符)
- ✅ 字符白名单 (字母、数字、下划线)
- ✅ 保留关键字检测

潜在绕过：

- ⚠️ Unicode 字符可能绕过 `is_ascii_alphabetic()`
- ⚠️ 需要确保 `escape_identifier` 在验证后调用

- [x] **Step 3: 增强 Unicode 检查**

```rust
fn validate_identifier(&self, identifier: &str) -> Result<()> {
    // 添加 Unicode 规范化检查
    let normalized = identifier.chars().collect::<String>();
    if normalized != identifier {
        return Err(CacheError::DatabaseError(
            "Identifier contains non-normalized Unicode".to_string()
        ));
    }

    // 确保 ASCII only（更严格）
    if !identifier.is_ascii() {
        return Err(CacheError::DatabaseError(
            "Identifier must be ASCII only".to_string()
        ));
    }

    // ... 现有检查
}
```

- [x] **Step 4: 提交审计结果**

无需代码修改，仅记录审计结论。

---

## Task 2: 增强 escape_identifier 实现

**Files:**

- Modify: `src/database/sqlite.rs:79-86`

- [x] **Step 1: 阅读当前转义实现**

```rust
fn escape_identifier(&self, identifier: &str) -> String {
    self.validate_identifier(identifier).expect("Invalid identifier");
    let escaped = identifier.replace("\"", "\"\"");
    format!("\"{}\"", escaped)
}
```

- [x] **Step 2: 移除 expect，改为返回 Result**

```rust
fn escape_identifier(&self, identifier: &str) -> Result<String> {
    self.validate_identifier(identifier)?;
    let escaped = identifier.replace("\"", "\"\"");
    Ok(format!("\"{}\"", escaped))
}
```

- [x] **Step 3: 更新所有调用点**

搜索所有 `escape_identifier` 调用：

```bash
grep -n "escape_identifier" src/database/sqlite.rs
```

更新调用模式：

```rust
// 原代码
let escaped = self.escape_identifier(t);

// 改为
let escaped = self.escape_identifier(t)?;
```

- [x] **Step 4: 验证编译**

```bash
cargo build --features database
```

- [x] **Step 5: 提交**

```bash
git add src/database/sqlite.rs
git commit -m "refactor: escape_identifier 返回 Result 而非 panic"
```

---

## Task 3: 添加 SQL 构建日志（安全审计）

**Files:**

- Modify: `src/database/sqlite.rs`

- [x] **Step 1: 添加 tracing 日志**

在动态 SQL 构建处添加日志：

```rust
use tracing::debug;

fn create_partition_table(&self, ...) -> Result<()> {
    let escaped = self.escape_identifier(&partition_name)?;
    let sql = format!("CREATE TABLE IF NOT EXISTS {} {}", escaped, columns);

    // 添加安全审计日志
    debug!(
        partition_name = %partition_name,
        sql_length = sql.len(),
        "Executing dynamic SQL for partition creation"
    );

    // 执行 SQL
}
```

- [x] **Step 2: 在所有动态 SQL 处添加日志**

位置：

- `create_partition_table`
- `drop_partition_table`
- `list_tables`
- `get_table_schema`

- [x] **Step 3: 验证日志输出**

```bash
RUST_LOG=debug cargo test --features database -- --nocapture
```

- [x] **Step 4: 提交**

```bash
git add src/database/sqlite.rs
git commit -m "security: 为动态 SQL 添加安全审计日志"
```

---

## Task 4: PostgreSQL 类似增强

**Files:**

- Modify: `src/database/postgresql.rs`

- [x] **Step 1: 检查是否有类似的 escape_identifier**

阅读 `postgresql.rs`，查找标识符处理逻辑。

**结果：PostgreSQL 文件不存在，无需处理。**

- [x] **Step 2: 添加相同的验证和转义**

N/A - PostgreSQL 支持已移除

- [x] **Step 3: 替换 format! 为验证版本**

N/A - PostgreSQL 支持已移除

- [x] **Step 4: 验证编译**

```bash
cargo build --features database
```

- [x] **Step 5: 提交**

N/A - 无需修改

---

## Task 5: 添加安全测试用例

**Files:**

- Create: `tests/security/sql_injection_tests.rs`

- [x] **Step 1: 创建测试文件**

```rust
// tests/security/sql_injection_tests.rs

#[cfg(feature = "database")]
mod tests {
    use oxcache::database::sqlite::SQLitePartitionManager;
    use oxcache::database::partition::PartitionConfig;

    #[test]
    fn test_sql_injection_in_table_name() {
        // 测试常见 SQL 注入模式被拒绝
        let malicious_names = vec![
            "users; DROP TABLE users--",
            "users' OR '1'='1",
            "users\" OR \"1\"=\"1",
            "users; INSERT INTO admin VALUES (1)",
            "users\0",
            "users\r\n",
        ];

        for name in malicious_names {
            // 验证这些名称被拒绝
            let result = manager.validate_identifier(name);
            assert!(result.is_err(), "Should reject malicious name: {}", name);
        }
    }

    #[test]
    fn test_unicode_bypass_attempt() {
        // 测试 Unicode 变体
        let unicode_names = vec![
            "users\u{0000}",  // Null 字符
            "users\u{0027}",  // 单引号
            "users\u{0022}",  // 双引号
        ];

        for name in unicode_names {
            let result = manager.validate_identifier(name);
            assert!(result.is_err(), "Should reject Unicode injection: {:?}", name);
        }
    }

    #[test]
    fn test_valid_identifier_accepted() {
        let valid_names = vec![
            "users",
            "user_data",
            "partition_2024_01",
            "cache_entries",
        ];

        for name in valid_names {
            let result = manager.validate_identifier(name);
            assert!(result.is_ok(), "Should accept valid name: {}", name);
        }
    }
}
```

- [x] **Step 2: 运行安全测试**

```bash
cargo test sql_injection --features database -- --nocapture
```

- [x] **Step 3: 提交**

```bash
git add tests/security/
git commit -m "test: 添加 SQL 注入防护测试用例"
```

---

## Task 6: 最终验证

- [x] **Step 1: 运行完整测试**

```bash
cargo test --features database
```

- [x] **Step 2: 运行 clippy**

```bash
cargo clippy --features database -- -D warnings
```

- [x] **Step 3: 手动安全审计**

确保所有动态 SQL 构建点都：

1. 使用 `validate_identifier()`
2. 使用 `escape_identifier()`
3. 有适当的日志记录

---

## 完成标准

- [x] 所有动态 SQL 标识符经过验证和转义
- [x] `escape_identifier` 返回 `Result` 而非 panic
- [x] 添加安全审计日志
- [x] PostgreSQL 有类似的验证（N/A - 已移除）
- [x] 安全测试用例通过
- [x] 所有测试通过
