// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Redis Sentinel 模式连接管理：master 发现、NAT 地址映射与 failover 跟随。
//!
//! 不使用 `redis::sentinel::SentinelClient`：它会对发现出的 master 地址做
//! 连通性验证，且没有地址改写钩子。容器等 NAT 部署下，sentinel 报告的
//! 节点地址（如容器内网 ip:port）对跨网络边界的客户端不可达，必须映射为
//! 客户端可达地址（如宿主机发布端口），因此这里自行实现最小发现协议
//! （`SENTINEL get-master-addr-by-name`，依次尝试各 sentinel，首个成功
//! 应答者胜出），并对发现地址应用 [`SentinelDiscovery::addr_map`]。

use crate::error::{OxCacheError, OxCacheResult};
use redis::Client;
use redis::aio::ConnectionLike;
use redis::aio::ConnectionManager;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::error::map_redis_error;

/// Sentinel 发现配置：sentinel URL 列表 + 监控的 master 名 + NAT 地址映射。
#[derive(Debug, Clone)]
pub(crate) struct SentinelDiscovery {
    sentinels: Vec<String>,
    master_name: String,
    addr_map: Vec<(String, String)>,
}

impl SentinelDiscovery {
    /// 解析连接字符串为 sentinel URL 列表（逗号分隔）。
    ///
    /// URL 可携带认证信息（用于连接 sentinel 本身）；数据节点的连接由
    /// 发现地址构建，不继承该认证信息。构建期逐个校验 URL 可解析，把
    /// 配置错误显性化。
    pub(crate) fn parse_urls(connection_string: &str) -> OxCacheResult<Vec<String>> {
        let urls: Vec<String> = connection_string
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        if urls.is_empty() {
            return Err(OxCacheError::InvalidInput(
                "Sentinel mode requires at least one sentinel URL in the connection string \
                 (comma-separated list, e.g. redis://host1:26379,redis://host2:26379)"
                    .to_string(),
            ));
        }
        for url in &urls {
            // 仅校验 scheme 前缀，不走 Client::open：其解析在未启用 TLS
            // 特性的构建里会对 rediss:// 误报，覆盖掉真实的配置语义
            if !(url.starts_with("redis://")
                || url.starts_with("rediss://")
                || url.starts_with("redis+unix://")
                || url.starts_with("unix://"))
            {
                return Err(OxCacheError::InvalidInput(format!(
                    "Invalid sentinel URL '{url}': expected redis://, rediss://, \
                     redis+unix:// or unix:// scheme"
                )));
            }
        }
        Ok(urls)
    }

    pub(crate) fn new(
        connection_string: &str,
        master_name: &str,
        addr_map: Vec<(String, String)>,
    ) -> OxCacheResult<Self> {
        Ok(Self {
            sentinels: Self::parse_urls(connection_string)?,
            master_name: master_name.to_string(),
            addr_map,
        })
    }

    /// 数据节点连接 scheme：跟随首个 sentinel URL（redis:// 或 rediss://）。
    fn data_scheme(&self) -> &'static str {
        if self
            .sentinels
            .first()
            .is_some_and(|u| u.starts_with("rediss://"))
        {
            "rediss"
        } else {
            "redis"
        }
    }

    /// 将 sentinel 视野内的节点地址映射为客户端可达地址并构建连接 URL。
    ///
    /// 映射表按 `ip:port` 精确匹配；未命中的地址原样使用。
    fn build_master_url(&self, ip: &str, port: u16, database: Option<u16>) -> String {
        let key = format!("{ip}:{port}");
        let (host, port) = match self.addr_map.iter().find(|(from, _)| *from == key) {
            Some((_, to)) => match to.rsplit_once(':') {
                Some((h, p)) => (h.to_string(), p.to_string()),
                None => (to.clone(), port.to_string()),
            },
            None => (ip.to_string(), port.to_string()),
        };
        // sentinel 报告 IPv6 字面量时需加方括号
        let host = if host.contains(':') {
            format!("[{host}]")
        } else {
            host
        };
        let mut url = format!("{}://{}:{}", self.data_scheme(), host, port);
        if let Some(db) = database {
            url.push('/');
            url.push_str(&db.to_string());
        }
        url
    }

    /// 依次询问各 sentinel 解析 master 地址，首个成功应答者胜出。
    pub(crate) async fn resolve_master(
        &self,
        connect_timeout: Duration,
        database: Option<u16>,
    ) -> OxCacheResult<String> {
        let mut last_err = String::from("no sentinel attempted");
        for url in &self.sentinels {
            let client = match Client::open(url.as_str()) {
                Ok(c) => c,
                Err(e) => {
                    last_err = format!("{url}: {}", map_redis_error(e));
                    continue;
                }
            };
            let mut conn = match tokio::time::timeout(
                connect_timeout,
                client.get_multiplexed_async_connection(),
            )
            .await
            {
                Ok(Ok(c)) => c,
                Ok(Err(e)) => {
                    last_err = format!("{url}: {e}");
                    continue;
                }
                Err(_) => {
                    last_err = format!("{url}: connect timeout");
                    continue;
                }
            };
            match redis::cmd("SENTINEL")
                .arg("get-master-addr-by-name")
                .arg(&self.master_name)
                .query_async::<Option<(String, u16)>>(&mut conn)
                .await
            {
                Ok(Some((ip, port))) => {
                    return Ok(self.build_master_url(&ip, port, database));
                }
                Ok(None) => {
                    last_err = format!("{url}: master '{}' unknown", self.master_name);
                }
                Err(e) => {
                    last_err = format!("{url}: {e}");
                }
            }
        }
        Err(OxCacheError::Connection(format!(
            "Sentinel could not resolve master '{}': no sentinel answered successfully \
             (last error: {last_err})",
            self.master_name
        )))
    }
}

/// Sentinel 模式的数据连接：持有当前 master 连接，可恢复错误后重新发现
/// master 并重建连接（failover 跟随）。
///
/// 连接句柄经 `RwLock` 共享：读锁仅克隆句柄（`ConnectionManager` 内部为
/// Arc，克隆廉价且共享同一条多路复用连接），写锁只覆盖连接替换、不跨
/// 网络 I/O 持有，稳态并发语义与 Single 模式一致。
#[derive(Clone)]
pub(crate) struct SentinelConnection {
    shared: Arc<RwLock<ConnectionManager>>,
    discovery: Arc<SentinelDiscovery>,
    connection_timeout: Duration,
    database: Option<u16>,
}

impl SentinelConnection {
    /// 发现 master 并建立初始连接；返回连接与指向 master 的 Client。
    pub(crate) async fn connect(
        discovery: SentinelDiscovery,
        connection_timeout: Duration,
        database: Option<u16>,
    ) -> OxCacheResult<(Self, Client)> {
        let discovery = Arc::new(discovery);
        let master_url = discovery
            .resolve_master(connection_timeout, database)
            .await?;
        let client = Client::open(master_url.as_str()).map_err(map_redis_error)?;
        let manager = Self::connect_manager(&client, connection_timeout).await?;
        Ok((
            Self {
                shared: Arc::new(RwLock::new(manager)),
                discovery,
                connection_timeout,
                database,
            },
            client,
        ))
    }

    async fn connect_manager(
        client: &Client,
        connection_timeout: Duration,
    ) -> OxCacheResult<ConnectionManager> {
        match tokio::time::timeout(connection_timeout, client.get_connection_manager()).await {
            Ok(Ok(mgr)) => Ok(mgr),
            Ok(Err(e)) => Err(OxCacheError::Connection(format!(
                "Failed to connect to the master resolved by sentinel: {e}"
            ))),
            Err(_) => Err(OxCacheError::Connection(
                "Connection timeout - Redis master resolved by sentinel unavailable".to_string(),
            )),
        }
    }

    /// 配置的数据库索引（`ConnectionLike::get_db` 的同步语义来源）。
    pub(crate) fn db(&self) -> i64 {
        self.database.map(i64::from).unwrap_or(0)
    }

    async fn current(&self) -> ConnectionManager {
        self.shared.read().await.clone()
    }

    /// 重新发现 master 并替换连接。发现失败时以扩展错误显性化原始触发
    /// 错误与失败原因，不静默吞掉。
    async fn rediscover(&self, trigger: &str) -> Result<(), redis::RedisError> {
        let master_url = self
            .discovery
            .resolve_master(self.connection_timeout, self.database)
            .await;
        let manager = match master_url {
            Ok(url) => match Client::open(url.as_str()) {
                Ok(client) => Self::connect_manager(&client, self.connection_timeout).await,
                Err(e) => Err(map_redis_error(e)),
            },
            Err(e) => Err(e),
        };
        match manager {
            Ok(new_manager) => {
                *self.shared.write().await = new_manager;
                Ok(())
            }
            Err(e) => Err(redis::make_extension_error(
                "OXCACHE_SENTINEL".to_string(),
                Some(format!("master rediscovery after '{trigger}' failed: {e}")),
            )),
        }
    }

    /// 执行单命令：可恢复错误触发一次「重新发现 + 重试」。
    pub(crate) async fn exec_cmd(&self, cmd: &redis::Cmd) -> redis::RedisResult<redis::Value> {
        let mut conn = self.current().await;
        match conn.req_packed_command(cmd).await {
            Ok(value) => Ok(value),
            Err(e) if Self::is_recoverable(&e) => {
                self.rediscover(&e.to_string()).await?;
                let mut conn = self.current().await;
                conn.req_packed_command(cmd).await
            }
            Err(e) => Err(e),
        }
    }

    /// 执行命令序列（管道）：可恢复错误触发一次「重新发现 + 整体重试」。
    pub(crate) async fn exec_pipeline(
        &self,
        pipeline: &redis::Pipeline,
        offset: usize,
        count: usize,
    ) -> redis::RedisResult<Vec<redis::Value>> {
        let mut conn = self.current().await;
        match conn.req_packed_commands(pipeline, offset, count).await {
            Ok(values) => Ok(values),
            Err(e) if Self::is_recoverable(&e) => {
                self.rediscover(&e.to_string()).await?;
                let mut conn = self.current().await;
                conn.req_packed_commands(pipeline, offset, count).await
            }
            Err(e) => Err(e),
        }
    }

    /// 可恢复错误判定：failover 会引发的连接失效与主从角色类错误，值得
    /// 重新发现一次；应用层语义错误（类型不匹配、NOPERM 等）不重试。
    fn is_recoverable(err: &redis::RedisError) -> bool {
        use redis::ErrorKind as K;
        use redis::ServerErrorKind as S;
        if err.is_connection_dropped() || err.is_io_error() || err.is_timeout() {
            return true;
        }
        matches!(
            err.kind(),
            K::Server(S::ReadOnly)
                | K::Server(S::MasterDown)
                | K::Server(S::ClusterDown)
                | K::Server(S::TryAgain)
                | K::Server(S::BusyLoading)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_urls_splits_and_trims() {
        let urls = SentinelDiscovery::parse_urls(
            "redis://127.0.0.1:26382 , redis://127.0.0.1:26383,,redis://127.0.0.1:26384",
        )
        .unwrap();
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "redis://127.0.0.1:26382");
    }

    #[test]
    fn test_parse_urls_rejects_empty() {
        assert!(SentinelDiscovery::parse_urls(" , ").is_err());
    }

    #[test]
    fn test_parse_urls_rejects_invalid() {
        assert!(SentinelDiscovery::parse_urls("not-a-url").is_err());
    }

    #[test]
    fn test_build_master_url_applies_addr_map_and_db() {
        let discovery = SentinelDiscovery::new(
            "redis://127.0.0.1:26382",
            "mymaster",
            vec![("172.26.0.2:6379".to_string(), "127.0.0.1:16380".to_string())],
        )
        .unwrap();

        // 命中映射表：容器地址 → 宿主机发布端口
        assert_eq!(
            discovery.build_master_url("172.26.0.2", 6379, Some(3)),
            "redis://127.0.0.1:16380/3"
        );
        // 未命中：原样使用
        assert_eq!(
            discovery.build_master_url("172.26.0.3", 6379, None),
            "redis://172.26.0.3:6379"
        );
    }

    #[test]
    fn test_build_master_url_follows_sentinel_tls_scheme() {
        let discovery =
            SentinelDiscovery::new("rediss://127.0.0.1:26382", "mymaster", vec![]).unwrap();
        assert_eq!(
            discovery.build_master_url("172.26.0.2", 6379, None),
            "rediss://172.26.0.2:6379"
        );
    }

    #[test]
    fn test_build_master_url_brackets_ipv6() {
        let discovery =
            SentinelDiscovery::new("redis://127.0.0.1:26382", "mymaster", vec![]).unwrap();
        assert_eq!(
            discovery.build_master_url("::1", 6379, None),
            "redis://[::1]:6379"
        );
    }

    #[test]
    fn test_is_recoverable_classifies_failover_errors() {
        let readonly = redis::RedisError::from((
            redis::ErrorKind::Server(redis::ServerErrorKind::ReadOnly),
            "READONLY",
        ));
        assert!(SentinelConnection::is_recoverable(&readonly));

        let dropped = redis::RedisError::from((redis::ErrorKind::Io, "connection dropped"));
        assert!(SentinelConnection::is_recoverable(&dropped));

        // 应用层语义错误不触发重新发现
        let noperm = redis::RedisError::from((
            redis::ErrorKind::Server(redis::ServerErrorKind::NoPerm),
            "NOPERM",
        ));
        assert!(!SentinelConnection::is_recoverable(&noperm));
    }

    /// 全部 sentinel 不可达：逐个尝试后返回 Connection 错误，错误信息必须
    /// 携带 master 名与最后一个失败原因（不静默吞掉根因）。
    #[tokio::test]
    async fn resolve_master_reports_last_error_when_all_sentinels_unreachable() {
        let discovery = SentinelDiscovery::new(
            "redis://127.0.0.1:1,redis://127.0.0.1:2",
            "mymaster",
            vec![],
        )
        .unwrap();
        let err = discovery
            .resolve_master(Duration::from_millis(300), None)
            .await
            .expect_err("an unreachable sentinel must return Err");
        let msg = err.to_string();
        assert!(
            msg.contains("mymaster"),
            "the error should name the master: {msg}"
        );
        assert!(
            msg.contains("127.0.0.1:2"),
            "the error should carry the last failure cause (the later sentinel): {msg}"
        );
    }

    /// 首个 sentinel URL 在 client 解析期即失败时跳过并继续尝试下一个，
    /// 全部失败后仍以统一错误形态收敛。
    #[tokio::test]
    async fn resolve_master_skips_unparsable_sentinel_url() {
        let discovery =
            SentinelDiscovery::new("redis://,redis://127.0.0.1:1", "mymaster", vec![]).unwrap();
        let err = discovery
            .resolve_master(Duration::from_millis(300), None)
            .await
            .expect_err("an unreachable sentinel must return Err");
        assert!(err.to_string().contains("mymaster"));
    }

    /// master 发现失败时 connect 不得伪造连接：直接透出发现错误。
    #[tokio::test]
    async fn connect_fails_when_master_unresolvable() {
        let discovery = SentinelDiscovery::new("redis://127.0.0.1:1", "mymaster", vec![]).unwrap();
        match SentinelConnection::connect(discovery, Duration::from_millis(300), None).await {
            Ok(_) => panic!("a discovery failure must not return a connection"),
            Err(OxCacheError::Connection(msg)) => {
                assert!(
                    msg.contains("mymaster"),
                    "the error should name the master: {msg}"
                );
            }
            Err(other) => panic!("should return a Connection error, got {other:?}"),
        }
    }

    /// 假 sentinel 端点（进程内 RESP2 服务器，见 `crate::test_support`）上的
    /// 完整发现 → 连接 → 命令执行：`resolve_master` 成功路径（含 addr_map 与
    /// database 拼接）、`connect`/`connect_manager`、`db()`、`exec_cmd` 与
    /// `exec_pipeline` 的稳态路径。
    #[tokio::test]
    async fn sentinel_discovers_and_executes_against_in_process_endpoint() {
        const PORT: u16 = 26390;
        assert!(
            crate::test_support::ensure_server(PORT),
            "the fake server should be ready on {PORT}"
        );
        let discovery = SentinelDiscovery::new(
            &format!("redis://127.0.0.1:{PORT}"),
            "mymaster",
            // 发现地址经映射表改写为客户端可达地址（NAT 部署口径）
            vec![(format!("127.0.0.1:{PORT}"), format!("localhost:{PORT}"))],
        )
        .unwrap();

        let url = discovery
            .resolve_master(Duration::from_secs(2), Some(3))
            .await
            .expect("the fake sentinel should resolve the master");
        assert_eq!(url, format!("redis://localhost:{PORT}/3"));

        let (conn, _client) =
            SentinelConnection::connect(discovery, Duration::from_secs(2), Some(3))
                .await
                .expect("should connect to the fake master endpoint");
        assert_eq!(conn.db(), 3);

        let pong = conn.exec_cmd(&redis::cmd("PING")).await.unwrap();
        assert_eq!(pong, redis::Value::SimpleString("PONG".to_string()));

        let mut pipeline = redis::Pipeline::new();
        pipeline.cmd("SET").arg("sentinel:k").arg("v");
        pipeline.cmd("GET").arg("sentinel:k");
        let values = conn.exec_pipeline(&pipeline, 0, 2).await.unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[1], redis::Value::BulkString(b"v".to_vec()));
    }

    /// 假 sentinel 上的未知 master 名（nil 应答）与错误应答：分别走
    /// `Ok(None)` 与 `Err` 分支，逐条记录失败原因后以统一错误收敛。
    #[tokio::test]
    async fn resolve_master_reports_unknown_master_and_error_reply() {
        const PORT: u16 = 26391;
        assert!(
            crate::test_support::ensure_server(PORT),
            "the fake server should be ready on {PORT}"
        );
        let sentinel_url = format!("redis://127.0.0.1:{PORT}");

        // nil 应答：master 名未知
        let unknown = SentinelDiscovery::new(&sentinel_url, "ghostmaster", vec![]).unwrap();
        let err = unknown
            .resolve_master(Duration::from_secs(2), None)
            .await
            .expect_err("an unknown master must error");
        assert!(
            err.to_string().contains("ghostmaster"),
            "the error should name the master: {err}"
        );

        // 错误应答：sentinel 拒绝服务，同样不得伪装成功
        let failing = SentinelDiscovery::new(&sentinel_url, "errormaster", vec![]).unwrap();
        let err = failing
            .resolve_master(Duration::from_secs(2), None)
            .await
            .expect_err("a sentinel error reply must error");
        let msg = err.to_string();
        assert!(
            msg.contains("errormaster") && msg.contains(&format!("127.0.0.1:{PORT}")),
            "the error should name both the master and the failed sentinel: {msg}"
        );
    }
}
