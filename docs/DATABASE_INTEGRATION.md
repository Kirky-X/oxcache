# 数据库集成完整指南

## 概述

Oxcache 提供了完整的数据库集成功能，支持 MySQL、PostgreSQL 和 SQLite 三种主流数据库。通过数据库加载器（Database Loader），可以自动从数据库加载数据并填充到缓存中，实现缓存预热和自动回源。

### 核心特性

- ✅ **多数据库支持**：MySQL、PostgreSQL、SQLite
- ✅ **自动加载**：缓存未命中时自动从数据库加载
- ✅ **连接池管理**：高效的数据库连接池
- ✅ **分区支持**：支持时间分区和哈希分区
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

### MySQL 集成

```rust
use oxcache::{Cache, CacheOps};
use oxcache::database::mysql::MySQLPartitionManager;
use oxcache::client::db_loader::{DbLoader, DbFallbackManager, SqlDbLoader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建 MySQL 分区管理器
    let partition_manager = MySQLPartitionManager::new(
        "mysql://user:password@localhost:3306/mydb"
    ).await?;

    // 创建数据库加载器
    let db_loader = SqlDbLoader::new(|key: &str| async move {
        // 从数据库查询
        let user_id: u64 = key.strip_prefix("user:")
            .ok_or("Invalid key")?
            .parse()?;
        
        // 使用 sea-orm 查询
        let db = Database::connect("mysql://user:password@localhost:3306/mydb").await?;
        let user = User::find_by_id(user_id).one(&db).await?
            .ok_or("User not found")?;
        
        Ok(user)
    });

    // 创建缓存
    let cache: Cache<String, User> = Cache::tiered(10000, "redis://localhost:6379").await?;

    // 设置数据库回源管理器
    let mut fallback_manager = DbFallbackManager::new(cache.clone(), db_loader);
    cache.set_db_fallback_manager(Some(fallback_manager.clone())).await?;

    // 查询用户（自动从数据库加载）
    let user = cache.get("user:123").await?.ok_or("User not found")?;

    println!("用户: {:?}", user);

    Ok(())
}
```

### PostgreSQL 集成

```rust
use oxcache::{Cache, CacheOps};
use oxcache::database::postgresql::PostgreSQLPartitionManager;
use oxcache::client::db_loader::{DbLoader, DbFallbackManager, SqlDbLoader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建 PostgreSQL 分区管理器
    let partition_manager = PostgreSQLPartitionManager::new(
        "postgresql://user:password@localhost:5432/mydb"
    ).await?;

    // 创建数据库加载器
    let db_loader = SqlDbLoader::new(|key: &str| async move {
        // 从数据库查询
        let product_id: u64 = key.strip_prefix("product:")
            .ok_or("Invalid key")?
            .parse()?;
        
        // 使用 sea-orm 查询
        let db = Database::connect("postgresql://user:password@localhost:5432/mydb").await?;
        let product = Product::find_by_id(product_id).one(&db).await?
            .ok_or("Product not found")?;
        
        Ok(product)
    });

    // 创建缓存
    let cache: Cache<String, Product> = Cache::tiered(10000, "redis://localhost:6379").await?;

    // 设置数据库回源管理器
    let mut fallback_manager = DbFallbackManager::new(cache.clone(), db_loader);
    cache.set_db_fallback_manager(Some(fallback_manager.clone())).await?;

    // 查询产品（自动从数据库加载）
    let product = cache.get("product:456").await?.ok_or("Product not found")?;

    println!("产品: {:?}", product);

    Ok(())
}
```

### SQLite 集成

```rust
use oxcache::{Cache, CacheOps};
use oxcache::database::sqlite::SQLitePartitionManager;
use oxcache::client::db_loader::{DbLoader, DbFallbackManager, SqlDbLoader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建 SQLite 分区管理器
    let partition_manager = SQLitePartitionManager::new(
        "sqlite:///path/to/database.db"
    ).await?;

    // 创建数据库加载器
    let db_loader = SqlDbLoader::new(|key: &str| async move {
        // 从数据库查询
        let config_name = key.strip_prefix("config:")
            .ok_or("Invalid key")?;
        
        // 使用 sea-orm 查询
        let db = Database::connect("sqlite:///path/to/database.db").await?;
        let config = Config::find_by_name(config_name).one(&db).await?
            .ok_or("Config not found")?;
        
        Ok(config)
    });

    // 创建缓存
    let cache: Cache<String, Config> = Cache::memory().await?;

    // 设置数据库回源管理器
    let mut fallback_manager = DbFallbackManager::new(cache.clone(), db_loader);
    cache.set_db_fallback_manager(Some(fallback_manager.clone())).await?;

    // 查询配置（自动从数据库加载）
    let config = cache.get("config:theme").await?.ok_or("Config not found")?;

    println!("配置: {:?}", config);

    Ok(())
}
```

### 使用 #[cached] 宏

```rust
use oxcache::cached;

// 带数据库加载的缓存函数
#[cached(service = "user_cache", ttl = 3600)]
async fn get_user(user_id: u64) -> Result<User, String> {
    // 使用 sea-orm 查询
    let db = Database::connect("mysql://user:password@localhost:3306/mydb").await
        .map_err(|e| e.to_string())?;
    
    let user = User::find_by_id(user_id).one(&db).await
        .map_err(|e| e.to_string())?
        .ok_or("User not found".to_string())?;
    
    Ok(user)
}
```

## 数据库分区

### 时间分区

```rust
use oxcache::database::partition::{PartitionConfig, TimeUnit};

// 按月分区
let config = PartitionConfig::time_based(TimeUnit::Month);

// 按季度分区
let config = PartitionConfig::time_based(TimeUnit::Quarter);

// 按年分区
let config = PartitionConfig::time_based(TimeUnit::Year);

// 应用分区配置
let loader = MySqlLoader::with_partition(
    "mysql://user:password@localhost:3306/mydb",
    config
).await?;
```

### 哈希分区

```rust
use oxcache::database::partition::PartitionConfig;

// 按哈希分区（4 个分片）
let config = PartitionConfig::hash_based(4);

// 按哈希分区（8 个分片）
let config = PartitionConfig::hash_based(8);

// 应用分区配置
let loader = PostgresLoader::with_partition(
    "postgresql://user:password@localhost:5432/mydb",
    config
).await?;
```

### 自定义分区

```rust
use oxcache::database::partition::{PartitionStrategy, PartitionConfig};

// 自定义分区策略
struct CustomPartitionStrategy;

impl PartitionStrategy for CustomPartitionStrategy {
    fn get_partition(&self, key: &str) -> usize {
        // 自定义分区逻辑
        if key.starts_with("user:") {
            0
        } else if key.starts_with("product:") {
            1
        } else {
            2
        }
    }
}

let config = PartitionConfig::custom(CustomPartitionStrategy);
```

## 连接池配置

### 基础配置

```rust
use oxcache::database::mysql::MySqlLoader;
use sqlx::mysql::MySqlPoolOptions;

// 创建自定义连接池
let pool = MySqlPoolOptions::new()
    .max_connections(20)      // 最大连接数
    .min_connections(5)       // 最小连接数
    .connect_timeout(Duration::from_secs(30))  // 连接超时
    .idle_timeout(Duration::from_secs(600))    // 空闲超时
    .max_lifetime(Duration::from_secs(1800))   // 最大生命周期
    .connect("mysql://user:password@localhost:3306/mydb")
    .await?;

// 使用连接池创建加载器
let loader = MySqlLoader::from_pool(pool);
```

### 高级配置

```rust
use sqlx::mysql::MySqlPoolOptions;

let pool = MySqlPoolOptions::new()
    .max_connections(20)
    .min_connections(5)
    .connect_timeout(Duration::from_secs(30))
    .idle_timeout(Duration::from_secs(600))
    .max_lifetime(Duration::from_secs(1800))
    .test_before_acquire(true)      // 获取连接前测试
    .acquire_timeout(Duration::from_secs(10))   // 获取超时
    .wait_timeout(Duration::from_secs(5))       // 等待超时
    .after_connect(|conn, _meta| Box::pin(async move {
        // 连接后执行初始化 SQL
        sqlx::query("SET time_zone = '+08:00'")
            .execute(&mut *conn)
            .await?;
        Ok(())
    }))
    .connect("mysql://user:password@localhost:3306/mydb")
    .await?;
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

### 批量加载

```rust
use oxcache::client::db_loader::DatabaseCacheLoader;

// 批量加载缓存
let keys = vec![
    "user:1".to_string(),
    "user:2".to_string(),
    "user:3".to_string(),
];

let results = db_loader.batch_get_or_load(keys, |keys| async move {
    let user_ids: Vec<u64> = keys.iter()
        .filter_map(|k| k.strip_prefix("user:"))
        .filter_map(|s| s.parse().ok())
        .collect();
    
    let users = sqlx::query_as::<_, User>(
        "SELECT id, name, email FROM users WHERE id IN (?)"
    )
    .bind(&user_ids)
    .fetch_all(&loader.pool)
    .await?;
    
    Ok(users)
}).await?;

for result in results {
    println!("{:?}", result);
}
```

### 预加载缓存

```rust
use oxcache::client::db_loader::DatabaseCacheLoader;

// 预加载热门数据
async fn preload_hot_data(
    db_loader: &DatabaseCacheLoader<User>,
    loader: &MySqlLoader,
) -> Result<(), Box<dyn std::error::Error>> {
    // 查询热门用户
    let hot_users = sqlx::query_as::<_, User>(
        "SELECT id, name, email FROM users 
         WHERE created_at > DATE_SUB(NOW(), INTERVAL 7 DAY)
         ORDER BY view_count DESC LIMIT 1000"
    )
    .fetch_all(&loader.pool)
    .await?;
    
    // 预加载到缓存
    for user in hot_users {
        let key = format!("user:{}", user.id);
        db_loader.cache.set(&key, &user, Some(3600)).await?;
    }
    
    println!("预加载了 {} 个热门用户", hot_users.len());
    
    Ok(())
}
```

### 故障恢复

```rust
use oxcache::client::db_loader::DatabaseCacheLoader;

// 带故障恢复的查询
async fn get_user_with_fallback(
    db_loader: &DatabaseCacheLoader<User>,
    user_id: u64,
) -> Result<Option<User>, Box<dyn std::error::Error>> {
    let key = format!("user:{}", user_id);
    
    // 尝试从数据库加载
    match db_loader.get_or_load(&key, |key| async move {
        let user_id: u64 = key.strip_prefix("user:")?
            .parse()?;
        
        let user = sqlx::query_as::<_, User>(
            "SELECT id, name, email FROM users WHERE id = ?"
        )
        .bind(user_id)
        .fetch_one(&loader.pool)
        .await?;
        
        Ok(user)
    }).await {
        Ok(user) => Ok(Some(user)),
        Err(e) => {
            // 数据库故障，返回默认值
            eprintln!("数据库查询失败: {}", e);
            Ok(None)
        }
    }
}
```

### 监控与统计

```rust
use oxcache::database::DatabaseStats;

// 获取数据库统计信息
let stats = loader.get_stats().await?;

println!("数据库统计：");
println!("  活跃连接: {}", stats.active_connections);
println!("  空闲连接: {}", stats.idle_connections);
println!("  总查询数: {}", stats.total_queries);
println!("  成功查询: {}", stats.successful_queries);
println!("  失败查询: {}", stats.failed_queries);
println!("  平均查询时间: {:?}", stats.avg_query_time);
```

## 最佳实践

### ✅ 推荐做法

1. **使用连接池**：合理配置连接池大小，避免连接泄漏
2. **错误处理**：妥善处理数据库错误，实现故障恢复
3. **索引优化**：为查询字段创建合适的索引
4. **批量操作**：使用批量查询减少数据库往返
5. **监控统计**：定期检查数据库统计信息

### ❌ 避免做法

1. **N+1 查询**：避免在循环中执行数据库查询
2. **连接泄漏**：不要忘记关闭数据库连接
3. **过度查询**：不要查询不需要的字段
4. **忽略错误**：不要忽略数据库错误
5. **硬编码 SQL**：不要在代码中硬编码 SQL 语句

## 性能优化

### 查询优化

```rust
// 使用索引
let user = sqlx::query_as::<_, User>(
    "SELECT id, name, email FROM users WHERE id = ?"
)
.bind(user_id)
.fetch_one(&pool)
.await?;

// 只查询需要的字段
let user_name = sqlx::query_as::<_, (String,)>(
    "SELECT name FROM users WHERE id = ?"
)
.bind(user_id)
.fetch_one(&pool)
.await?;
```

### 缓存策略

```rust
// 热数据：长 TTL
cache.set(&key, &value, Some(3600)).await?;

// 冷数据：短 TTL
cache.set(&key, &value, Some(300)).await?;

// 静态数据：永久缓存
cache.set(&key, &value, None).await?;
```

## 完整示例

```rust
use oxcache::{Cache, CacheOps};
use oxcache::database::mysql::MySQLPartitionManager;
use oxcache::client::db_loader::{DbLoader, DbFallbackManager, SqlDbLoader};
use serde::{Deserialize, Serialize};
use sea_orm::{Database, EntityTrait};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 数据库集成完整示例 ===\n");
    
    // 1. 创建 MySQL 分区管理器
    println!("1. 创建 MySQL 分区管理器...");
    let partition_manager = MySQLPartitionManager::new(
        "mysql://root:password@localhost:3306/myapp"
    ).await?;
    println!("   ✅ 分区管理器创建成功\n");
    
    // 2. 创建缓存
    println!("2. 创建双层缓存...");
    let cache: Cache<String, User> = Cache::tiered(10000, "redis://localhost:6379").await?;
    println!("   ✅ 缓存创建成功\n");
    
    // 3. 创建数据库加载器
    println!("3. 创建数据库加载器...");
    let db_loader = SqlDbLoader::new(|key: &str| async move {
        let user_id: u64 = key.strip_prefix("user:")?
            .parse()?;
        
        println!("   📡 从数据库查询用户: {}", user_id);
        
        let db = Database::connect("mysql://root:password@localhost:3306/myapp").await?;
        let user = User::find_by_id(user_id).one(&db).await?
            .ok_or("User not found")?;
        
        println!("   ✅ 数据库查询成功");
        
        Ok(user)
    });
    println!("   ✅ 加载器创建成功\n");
    
    // 4. 设置数据库回源管理器
    println!("4. 设置数据库回源管理器...");
    let mut fallback_manager = DbFallbackManager::new(cache.clone(), db_loader);
    cache.set_db_fallback_manager(Some(fallback_manager)).await?;
    println!("   ✅ 回源管理器设置成功\n");
    
    // 5. 查询用户（自动从数据库加载）
    println!("5. 查询用户...");
    let user_id = 123;
    let key = format!("user:{}", user_id);
    
    let user = cache.get(&key).await?.ok_or("User not found")?;
    println!("   用户: {} ({})", user.name, user.email);
    println!();
    
    // 6. 再次查询（从缓存读取）
    println!("6. 再次查询相同用户...");
    let user2 = cache.get(&key).await?.ok_or("User not found")?;
    println!("   💾 从缓存读取: {} ({})", user2.name, user2.email);
    println!();
    
    // 7. 分区操作
    println!("7. 分区操作...");
    let partitions = partition_manager.list_partitions().await?;
    println!("   现有分区: {:?}", partitions);
    
    Ok(())
}
```

## 故障排除

### 问题：连接池耗尽

**原因**：
- 连接未正确释放
- 并发请求过多
- 连接池配置过小

**解决方案**：
1. 检查连接是否正确释放
2. 增加连接池大小
3. 使用连接超时设置

### 问题：查询超时

**原因**：
- 数据库响应慢
- 查询语句复杂
- 网络延迟高

**解决方案**：
1. 优化查询语句
2. 添加索引
3. 增加超时时间

### 问题：连接失败

**原因**：
- 连接字符串错误
- 数据库服务未启动
- 网络不通

**解决方案**：
1. 检查连接字符串
2. 确认数据库服务运行
3. 检查网络连接

## 相关文档

- [用户指南](USER_GUIDE.md)
- [架构文档](ARCHITECTURE.md)
- [API 参考](API_REFERENCE.md)
- [Sea-ORM 文档](https://www.sea-ql.org/SeaORM/)

## 示例代码

- `examples/src/05_database/` - 数据库集成示例
- `tests/database_test.rs` - 数据库测试
- `src/database/` - 数据库实现