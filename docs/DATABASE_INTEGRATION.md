## 数据库集成完整指南

## 概述

Oxcache 提供了数据库分区管理功能，支持 MySQL、PostgreSQL 和 SQLite 三种主流数据库。通过分区管理器（Partition Manager），可以自动管理数据库表的分区，实现按时间范围的数据分离和高效查询。

### 核心特性

- ✅ **多数据库支持**：MySQL、PostgreSQL、SQLite
- ✅ **时间分区**：支持按月、按季度、按年分区
- ✅ **自动分区管理**：自动创建、清理过期分区
- ✅ **连接池管理**：高效的数据库连接池
- ✅ **故障恢复**：数据库故障时的降级处理
- ✅ **配置灵活**：支持连接字符串和配置文件
- ✅ **类型安全**：基于 Sea-ORM 的类型安全查询

## 支持的数据库

| 数据库 | 驱动 | 特性 | 适用场景 |
|--------|------|------|----------|
| MySQL | `sqlx-mysql` | 高性能、成熟稳定 | Web 应用、电商系统 |
| PostgreSQL | `sqlx-postgres` | 高级特性、JSON 支持 | 企业应用、数据分析 |
| SQLite | `sqlx-sqlite` | 轻量级、无服务器 | 嵌入式系统、移动应用 |

## 使用方式

### MySQL 分区管理器

```rust
use oxcache::database::mysql::MySQLPartitionManager;
use oxcache::database::partition::{PartitionConfig, PartitionStrategy};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 配置分区策略
    let config = PartitionConfig {
        enabled: true,
        strategy: PartitionStrategy::Monthly,
        retention_months: 12,  // 保留 12 个月的数据
        precreate_months: 3,   // 预创建 3 个月的分区
    };

    // 创建 MySQL 分区管理器
    let partition_manager = MySQLPartitionManager::new(
        "mysql://user:password@localhost:3306/mydb",
        config
    ).await?;

    // 初始化分区表
    let schema = r#"
        CREATE TABLE IF NOT EXISTS logs (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            message TEXT NOT NULL,
            created_at DATE NOT NULL,
            INDEX idx_created_at (created_at)
        )
    "#;
    partition_manager.initialize_table("logs", schema).await?;

    // 预创建未来分区
    partition_manager.precreate_partitions("logs", 3).await?;

    // 获取所有分区
    let partitions = partition_manager.get_partitions("logs").await?;
    for partition in partitions {
        println!("分区: {} ({} ~ {})",
            partition.name,
            partition.start_date.format("%Y-%m-%d"),
            partition.end_date.format("%Y-%m-%d")
        );
    }

    Ok(())
}
```

### PostgreSQL 分区管理器

```rust
use oxcache::database::postgresql::PostgresPartitionManager;
use oxcache::database::partition::{PartitionConfig, PartitionStrategy};
use chrono::{Utc, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 配置分区策略
    let config = PartitionConfig {
        enabled: true,
        strategy: PartitionStrategy::Monthly,
        retention_months: 12,
        precreate_months: 3,
    };

    // 创建 PostgreSQL 分区管理器
    let partition_manager = PostgresPartitionManager::new(
        "postgresql://user:password@localhost:5432/mydb",
        config
    ).await?;

    // 初始化分区表（PostgreSQL 使用声明式分区）
    let schema = r#"
        CREATE TABLE IF NOT EXISTS events (
            id BIGSERIAL PRIMARY KEY,
            event_type VARCHAR(100) NOT NULL,
            payload JSONB,
            created_at TIMESTAMP NOT NULL
        )
    "#;
    partition_manager.initialize_table("events", schema).await?;

    // 预创建未来分区
    partition_manager.precreate_partitions("events", 3).await?;

    // 清理过期分区（保留 6 个月）
    let cutoff_date = Utc::now() - Duration::days(180);
    let dropped_count = partition_manager.cleanup_old_partitions("events", cutoff_date).await?;
    println!("清理了 {} 个过期分区", dropped_count);

    Ok(())
}
```

### SQLite 分区管理器

```rust
use oxcache::database::sqlite::SQLitePartitionManager;
use oxcache::database::partition::{PartitionConfig, PartitionStrategy};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 配置分区策略
    let config = PartitionConfig {
        enabled: true,
        strategy: PartitionStrategy::Monthly,
        retention_months: 6,
        precreate_months: 2,
    };

    // 创建 SQLite 分区管理器
    let partition_manager = SQLitePartitionManager::new(
        "sqlite:///path/to/database.db",
        config
    ).await?;

    // 初始化分区表
    let schema = r#"
        CREATE TABLE IF NOT EXISTS metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            metric_name TEXT NOT NULL,
            value REAL NOT NULL,
            timestamp TEXT NOT NULL
        )
    "#;
    partition_manager.initialize_table("metrics", schema).await?;

    // 获取所有分区
    let partitions = partition_manager.get_partitions("metrics").await?;
    println!("当前分区数: {}", partitions.len());

    Ok(())
}
```

## 数据库分区

### 分区策略

Oxcache 支持两种分区策略：

1. **按月分区 (Monthly)**：将数据按月进行分区，适合时间序列数据
2. **按范围分区 (Range)**：自定义范围分区，适合特定业务场景

```rust
use oxcache::database::partition::{PartitionConfig, PartitionStrategy};

// 按月分区（推荐）
let config = PartitionConfig {
    enabled: true,
    strategy: PartitionStrategy::Monthly,
    retention_months: 12,  // 保留 12 个月的数据
    precreate_months: 3,   // 预创建 3 个月的分区
};

// 按范围分区（自定义）
let config = PartitionConfig {
    enabled: true,
    strategy: PartitionStrategy::Range,
    retention_months: 6,
    precreate_months: 2,
};
```

### 分区配置参数

| 参数 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| `enabled` | `bool` | 是否启用分区功能 | `true` |
| `strategy` | `PartitionStrategy` | 分区策略 | `Monthly` |
| `retention_months` | `u32` | 保留数据的月数 | `12` |
| `precreate_months` | `u32` | 预创建分区月数 | `3` |

### 分区操作示例

```rust
use oxcache::database::mysql::MySQLPartitionManager;
use oxcache::database::partition::PartitionConfig;
use chrono::{Utc, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = PartitionConfig::default();
    let manager = MySQLPartitionManager::new(
        "mysql://user:password@localhost:3306/mydb",
        config
    ).await?;

    // 1. 初始化分区表
    let schema = "CREATE TABLE logs (id BIGINT, created_at DATE)";
    manager.initialize_table("logs", schema).await?;

    // 2. 确保分区存在
    let now = Utc::now();
    manager.ensure_partition_exists(now, "logs").await?;

    // 3. 预创建未来分区
    manager.precreate_partitions("logs", 3).await?;

    // 4. 清理过期分区
    let cutoff = Utc::now() - Duration::days(180);
    let dropped = manager.cleanup_old_partitions("logs", cutoff).await?;
    println!("清理了 {} 个过期分区", dropped);

    // 5. 获取所有分区
    let partitions = manager.get_partitions("logs").await?;
    for p in partitions {
        println!("分区: {} ({} ~ {})", p.name, p.start_date, p.end_date);
    }

    Ok(())
}
```

## 连接池配置

分区管理器内部使用 Sea-ORM 的连接池，配置参数在创建管理器时设置：

```rust
use oxcache::database::mysql::MySQLPartitionManager;
use oxcache::database::partition::PartitionConfig;
use sea_orm::ConnectOptions;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = PartitionConfig::default();

    // 注意：分区管理器的连接池配置是内置的
    // 如需自定义连接池，可以直接使用 Sea-ORM
    let mut opt = ConnectOptions::new("mysql://user:password@localhost:3306/mydb");
    opt.max_connections(10)
        .min_connections(2)
        .connect_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(1800))
        .acquire_timeout(Duration::from_secs(10));

    // 创建分区管理器（内部使用上述配置）
    let manager = MySQLPartitionManager::new_with_options(
        "mysql://user:password@localhost:3306/mydb",
        config,
        opt
    ).await?;

    Ok(())
}
```

### 连接池统计信息

```rust
use oxcache::database::mysql::MySQLPartitionManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = PartitionConfig::default();
    let manager = MySQLPartitionManager::new(
        "mysql://user:password@localhost:3306/mydb",
        config
    ).await?;

    // 获取连接池统计信息
    let stats = manager.pool_stats().await;
    println!("活跃连接: {}", stats.active_connections);
    println!("空闲连接: {}", stats.idle_connections);
    println!("最大连接数: {}", stats.max_connections);
    println!("连接获取时间: {:.2}ms", stats.connection_acquire_ms);

    Ok(())
}
```

## 连接字符串

### MySQL 连接字符串

```
# 基础格式
mysql://user:password@host:port/database

# 完整示例
mysql://root:password123@localhost:3306/myapp_db

# 带参数
mysql://user:password@localhost:3306/db?charset=utf8mb4&parseTime=true

# Unix Socket
mysql://user:password@/path/to/socket/dbname

# SSL 连接
mysql://user:password@localhost:3306/db?sslmode=require
```

### PostgreSQL 连接字符串

```
# 基础格式
postgresql://user:password@host:port/database

# 完整示例
postgresql://postgres:password123@localhost:5432/myapp_db

# 带参数
postgresql://user:password@localhost:5432/db?sslmode=require&application_name=myapp

# Unix Socket
postgresql://user:password@/var/run/postgresql/.s.PGSQL.5432/dbname
```

### SQLite 连接字符串

```
# 文件路径
sqlite:///path/to/database.db

# 内存数据库
sqlite::memory:

# 相对路径
sqlite:./data.db

# 只读模式
sqlite:///path/to/database.db?mode=ro
```

## 高级用法

### 健康检查

```rust
use oxcache::database::mysql::MySQLPartitionManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = PartitionConfig::default();
    let manager = MySQLPartitionManager::new(
        "mysql://user:password@localhost:3306/mydb",
        config
    ).await?;

    // 检查连接健康状态
    if manager.health_check().await {
        println!("数据库连接正常");
    } else {
        println!("数据库连接异常");
    }

    Ok(())
}
```

### 重新连接

```rust
use oxcache::database::mysql::MySQLPartitionManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = PartitionConfig::default();
    let mut manager = MySQLPartitionManager::new(
        "mysql://user:password@localhost:3306/mydb",
        config
    ).await?;

    // 检测到连接失败时重新连接
    if !manager.health_check().await {
        println!("尝试重新连接...");
        manager.reconnect("mysql://user:password@localhost:3306/mydb").await?;
        println!("重新连接成功");
    }

    Ok(())
}
```

### 分区信息查询

```rust
use oxcache::database::partition::PartitionManager;

async fn print_partition_info<T: PartitionManager>(
    manager: &T,
    table_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let partitions = manager.get_partitions(table_name).await?;

    println!("表 {} 的分区信息:", table_name);
    println!("  总分区数: {}", partitions.len());

    for partition in partitions {
        println!("  - {}:", partition.name);
        println!("    起始时间: {}", partition.start_date.format("%Y-%m-%d %H:%M:%S"));
        println!("    结束时间: {}", partition.end_date.format("%Y-%m-%d %H:%M:%S"));
        println!("    已创建: {}", if partition.created { "是" } else { "否" });
    }

    Ok(())
}
```

## 最佳实践

### ✅ 推荐做法

1. **合理配置分区策略**：
   - 根据数据保留时间设置 `retention_months`
   - 提前预创建分区避免写入失败
   - 定期清理过期分区释放空间

2. **监控分区状态**：
   - 定期检查分区是否创建成功
   - 监控分区数量和大小
   - 关注连接池健康状态

3. **错误处理**：
   - 妥善处理分区创建失败
   - 实现重连机制应对网络问题
   - 记录分区操作日志

4. **性能优化**：
   - 为分区字段创建索引
   - 合理设置连接池大小
   - 使用批量操作提升效率

### ❌ 避免做法

1. **不要忽略分区配置**：
   - 不要忘记预创建分区
   - 不要让分区数量无限增长

2. **不要硬编码连接字符串**：
   - 使用环境变量或配置文件
   - 避免在代码中暴露敏感信息

3. **不要忽略错误处理**：
   - 不要忽略分区创建失败
   - 不要忽略连接异常

4. **不要过度分区**：
   - 避免创建过多的小分区
   - 根据实际数据量选择分区粒度

## 性能优化

### 分区表设计

```rust
// 1. 选择合适的分区字段（通常是时间字段）
let schema = r#"
    CREATE TABLE events (
        id BIGINT AUTO_INCREMENT PRIMARY KEY,
        event_type VARCHAR(100) NOT NULL,
        data JSON,
        created_at DATE NOT NULL,  -- 分区字段
        INDEX idx_created_at (created_at)
    )
"#;

// 2. 为分区字段创建索引
// MySQL 和 PostgreSQL 会自动在分区字段上创建索引

// 3. 合理设置分区粒度
// - 数据量大：按月分区
// - 数据量小：按季度或按年分区
let config = PartitionConfig {
    enabled: true,
    strategy: PartitionStrategy::Monthly,  // 按月分区
    retention_months: 12,
    precreate_months: 3,
};
```

### 连接池优化

```rust
use sea_orm::ConnectOptions;
use std::time::Duration;

// 根据实际负载调整连接池参数
let mut opt = ConnectOptions::new("mysql://user:password@localhost:3306/mydb");
opt.max_connections(20)      // 根据并发量调整
    .min_connections(5)       // 保持最小连接数
    .connect_timeout(Duration::from_secs(5))
    .idle_timeout(Duration::from_secs(600))
    .max_lifetime(Duration::from_secs(1800))
    .acquire_timeout(Duration::from_secs(10));
```

### 分区管理优化

```rust
// 1. 批量预创建分区（减少频繁创建的开销）
manager.precreate_partitions("logs", 6).await?;

// 2. 定期清理过期分区（避免分区数量过多）
let cutoff = Utc::now() - Duration::days(90);
manager.cleanup_old_partitions("logs", cutoff).await?;

// 3. 使用连接池统计信息监控性能
let stats = manager.pool_stats().await;
if stats.active_connections > stats.max_connections * 8 / 10 {
    println!("警告：连接池使用率超过 80%");
}
```

## 完整示例

```rust
use oxcache::database::mysql::MySQLPartitionManager;
use oxcache::database::partition::{PartitionConfig, PartitionStrategy};
use chrono::{Utc, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MySQL 分区管理器完整示例 ===\n");

    // 1. 配置分区策略
    println!("1. 配置分区策略...");
    let config = PartitionConfig {
        enabled: true,
        strategy: PartitionStrategy::Monthly,
        retention_months: 12,
        precreate_months: 3,
    };
    println!("   ✅ 分区配置: 按月分区，保留12个月\n");

    // 2. 创建分区管理器
    println!("2. 创建 MySQL 分区管理器...");
    let manager = MySQLPartitionManager::new(
        "mysql://root:password@localhost:3306/myapp",
        config
    ).await?;
    println!("   ✅ 分区管理器创建成功\n");

    // 3. 初始化分区表
    println!("3. 初始化分区表...");
    let schema = r#"
        CREATE TABLE IF NOT EXISTS user_logs (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            user_id BIGINT NOT NULL,
            action VARCHAR(100) NOT NULL,
            metadata JSON,
            created_at DATE NOT NULL,
            INDEX idx_user_id (user_id),
            INDEX idx_created_at (created_at)
        )
    "#;
    manager.initialize_table("user_logs", schema).await?;
    println!("   ✅ 表初始化成功\n");

    // 4. 预创建未来分区
    println!("4. 预创建未来分区...");
    manager.precreate_partitions("user_logs", 3).await?;
    println!("   ✅ 预创建成功\n");

    // 5. 获取所有分区
    println!("5. 获取所有分区...");
    let partitions = manager.get_partitions("user_logs").await?;
    println!("   当前分区数: {}", partitions.len());
    for partition in &partitions {
        println!("   - {} ({} ~ {})",
            partition.name,
            partition.start_date.format("%Y-%m-%d"),
            partition.end_date.format("%Y-%m-%d")
        );
    }
    println!();

    // 6. 清理过期分区
    println!("6. 清理过期分区...");
    let cutoff_date = Utc::now() - Duration::days(180);
    let dropped_count = manager.cleanup_old_partitions("user_logs", cutoff_date).await?;
    println!("   ✅ 清理了 {} 个过期分区\n", dropped_count);

    // 7. 健康检查
    println!("7. 健康检查...");
    if manager.health_check().await {
        println!("   ✅ 数据库连接正常\n");
    } else {
        println!("   ❌ 数据库连接异常\n");
    }

    // 8. 获取连接池统计
    println!("8. 连接池统计...");
    let stats = manager.pool_stats().await;
    println!("   活跃连接: {}", stats.active_connections);
    println!("   空闲连接: {}", stats.idle_connections);
    println!("   最大连接数: {}", stats.max_connections);
    println!("   连接获取时间: {:.2}ms\n", stats.connection_acquire_ms);

    println!("=== 示例完成 ===");

    Ok(())
}
```

## 故障排除

### 问题：分区创建失败

**原因**：
- 表结构不包含日期字段
- 分区字段类型不正确
- 数据库权限不足

**解决方案**：
1. 确保表包含 `DATE` 或 `TIMESTAMP` 字段
2. 检查分区字段类型是否正确
3. 验证数据库用户权限

```rust
// 正确的表结构示例
let schema = r#"
    CREATE TABLE logs (
        id BIGINT AUTO_INCREMENT PRIMARY KEY,
        message TEXT,
        created_at DATE NOT NULL  -- 必须是日期类型
    )
"#;
```

### 问题：分区未自动创建

**原因**：
- 没有调用 `precreate_partitions`
- 分区配置未启用
- 时间计算错误

**解决方案**：
1. 调用 `precreate_partitions` 预创建分区
2. 检查 `PartitionConfig.enabled` 是否为 `true`
3. 验证时间计算逻辑

```rust
// 确保启用分区
let config = PartitionConfig {
    enabled: true,  // 必须为 true
    strategy: PartitionStrategy::Monthly,
    retention_months: 12,
    precreate_months: 3,
};

// 预创建分区
manager.precreate_partitions("logs", 3).await?;
```

### 问题：连接失败

**原因**：
- 连接字符串错误
- 数据库服务未启动
- 网络不通
- 用户名或密码错误

**解决方案**：
1. 检查连接字符串格式
2. 确认数据库服务运行
3. 检查网络连接
4. 验证用户名和密码

```rust
// 使用健康检查验证连接
if !manager.health_check().await {
    eprintln!("数据库连接失败");
    // 尝试重新连接
    manager.reconnect("mysql://user:password@localhost:3306/mydb").await?;
}
```

### 问题：分区清理失败

**原因**：
- 分区正在被使用
- 权限不足
- 分区名称错误

**解决方案**：
1. 确保分区没有被锁定
2. 检查数据库用户权限
3. 验证分区名称

```rust
// 使用正确的截止时间
let cutoff = Utc::now() - Duration::days(90);
let dropped = manager.cleanup_old_partitions("logs", cutoff).await?;
println!("清理了 {} 个分区", dropped);
```

## 相关文档

- [用户指南](USER_GUIDE.md)
- [架构文档](ARCHITECTURE.md)
- [API 参考](API_REFERENCE.md)
- [Sea-ORM 文档](https://www.sea-ql.org/SeaORM/)

## 示例代码

- `examples/src/05_database/` - 数据库分区示例
- `tests/database_partitioning_tests.rs` - 分区测试
- `src/database/` - 数据库分区实现

## API 参考

### MySQLPartitionManager

```rust
pub struct MySQLPartitionManager {
    config: PartitionConfig,
    connection: Arc<DatabaseConnection>,
    pool_stats: Arc<Mutex<PoolStats>>,
}

impl MySQLPartitionManager {
    pub async fn new(connection_string: &str, config: PartitionConfig) -> Result<Self>;
    pub async fn health_check(&self) -> bool;
    pub async fn ping(&self) -> Result<()>;
    pub async fn pool_stats(&self) -> PoolStats;
    pub async fn reconnect(&mut self, connection_string: &str) -> Result<()>;
}
```

### PostgresPartitionManager

```rust
pub struct PostgresPartitionManager {
    config: PartitionConfig,
    connection: Arc<DatabaseConnection>,
    pool_stats: Arc<Mutex<PoolStats>>,
}

impl PostgresPartitionManager {
    pub async fn new(connection_string: &str, config: PartitionConfig) -> Result<Self>;
    pub async fn health_check(&self) -> bool;
    pub async fn ping(&self) -> Result<()>;
    pub async fn get_pool_stats(&self) -> PoolStats;
    pub async fn reconnect(&mut self, connection_string: &str) -> Result<()>;
}
```

### SQLitePartitionManager

```rust
pub struct SQLitePartitionManager {
    config: PartitionConfig,
    connection: Arc<DatabaseConnection>,
}

impl SQLitePartitionManager {
    pub async fn new(connection_string: &str, config: PartitionConfig) -> Result<Self>;
    pub fn new_sync(connection_string: &str, config: PartitionConfig) -> Result<Self>;
}
```

### PartitionManager Trait

```rust
#[async_trait]
pub trait PartitionManager: Send + Sync {
    async fn initialize_table(&self, table_name: &str, schema: &str) -> Result<()>;
    async fn create_partition(&self, partition: &PartitionInfo) -> Result<()>;
    async fn get_partitions(&self, table_name: &str) -> Result<Vec<PartitionInfo>>;
    async fn drop_partition(&self, table_name: &str, partition_name: &str) -> Result<()>;
    async fn ensure_partition_exists(&self, date: DateTime<Utc>, table_name: &str) -> Result<String>;
    async fn precreate_partitions(&self, table_name: &str, months_ahead: u32) -> Result<()>;
    async fn cleanup_old_partitions(&self, table_name: &str, cutoff_date: DateTime<Utc>) -> Result<u32>;
    fn get_config(&self) -> PartitionConfig;
}
```

### PartitionConfig

```rust
pub struct PartitionConfig {
    pub enabled: bool,
    pub strategy: PartitionStrategy,
    pub retention_months: u32,
    pub precreate_months: u32,
}

impl Default for PartitionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strategy: PartitionStrategy::Monthly,
            retention_months: 12,
            precreate_months: 3,
        }
    }
}
```