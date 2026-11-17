[English](README.md) | 中文

<div align="center">

<img src="docs/assets/oxcache.png" alt="Oxcache Logo" width="240">

[![Rust 2024](https://img.shields.io/badge/Rust-2024-edded?logo=rust&style=for-the-badge)](https://www.rust-lang.org/) [![Rustc 1.97.1+](https://img.shields.io/badge/Rustc-1.97.1+-orange.svg?style=for-the-badge)](https://www.rust-lang.org/) [![crates.io](https://img.shields.io/crates/v/oxcache?style=for-the-badge)](https://crates.io/crates/oxcache) [![docs.rs](https://img.shields.io/docsrs/oxcache?style=for-the-badge)](https://docs.rs/oxcache) [![downloads](https://img.shields.io/crates/d/oxcache?style=for-the-badge)](https://crates.io/crates/oxcache) [![CI](https://img.shields.io/github/actions/workflow/status/Kirky-X/oxcache/ci.yml?branch=main&label=CI&style=for-the-badge)](https://github.com/Kirky-X/oxcache/actions/workflows/ci.yml) [![coverage](https://img.shields.io/codecov/c/github/Kirky-X/oxcache?style=for-the-badge)](https://codecov.io/gh/Kirky-X/oxcache) [![license](https://img.shields.io/crates/l/oxcache?style=for-the-badge)](LICENSE)

**Rust 多级缓存库：L1 内存 + L2 分布式 + 可选 L3 磁盘（redb）**

[✨ 功能特性](#-功能特性) • [🚀 快速开始](#-快速开始) • [📚 文档](#-文档) • [💻 示例](#-示例) • [🤝 参与贡献](#-参与贡献)

</div>

---

## ✨ 功能特性

<table style="width:100%; border-collapse: collapse">
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🚀 <b>多级缓存</b><br><span style="color:#64748B">L1（Moka / DashMap）与 L2（Redis / Valkey / Dragonfly / Aerospike）经 <code>ChainCache</code> 按分数组链，非最高分命中可异步回填</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">⚡ <b>零侵入宏</b><br><span style="color:#64748B"><code>#[cached]</code> 一行接入，支持 <code>service</code> / <code>ttl</code> / <code>key</code> / <code>key_prefix</code> / <code>scope</code> / <code>sync</code> / <code>single_flight</code> / <code>strict</code> / <code>condition</code> / <code>skip</code></span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🔄 <b>同步 API</b><br><span style="color:#64748B"><code>sync_mode(true)</code> 后 <code>get_sync</code> / <code>set_sync</code> / <code>get_or_sync</code> 与异步 API 在同一 <code>Cache&lt;K, V&gt;</code> 上共存</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">⏱️ <b>全后端 per-entry TTL</b><br><span style="color:#64748B"><code>ttl</code> / <code>expire</code> 在 Moka / DashMap / Redis / Valkey（经 RedisBackend）/ Dragonfly / Aerospike / Disk / Mock / Chain / Bloom 后端语义一致</span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🌸 <b>穿透防护</b><br><span style="color:#64748B">单飞去重（64 分片）、空值哨兵、TTL 抖动、布隆过滤器负查询短路</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">🔐 <b>安全内建</b><br><span style="color:#64748B">键 / Lua / SCAN 三层输入校验、连接串脱敏、值级加密与完整性装饰器</span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">📈 <b>可观测性</b><br><span style="color:#64748B">延迟直方图与操作计数、Prometheus / JSON 导出、<code>telemetry</code> tracing 遥测、审计事件流</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">🧬 <b>可插拔序列化</b><br><span style="color:#64748B">JSON 默认，<code>serde-bincode</code> / <code>postcard</code> 二进制格式可选，深度限制防嵌套 DoS</span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🗜️ <b>自适应压缩</b><br><span style="color:#64748B"><code>CompressingBackend</code> 按阈值触发 zstd，读取按魔数识别并兼容旧 gzip</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">🔑 <b>分布式协调</b><br><span style="color:#64748B">Redis 分布式锁（watchdog 续期 / 可重入）、RedLock 多节点多数派锁、跨实例失效总线</span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🧯 <b>故障韧性</b><br><span style="color:#64748B">ChainCache 单链路容错、<code>degradation</code> 三态自动降级与恢复、健康检查、优雅关闭</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">🧪 <b>工程化质量</b><br><span style="color:#64748B">2000+ 测试函数（截至 0.5.1）、混沌与安全测试、三平台 CI 矩阵、覆盖率门禁</span></td>
</tr>
</table>

> `lock` / `red-lock` / `invalidation` / `encrypt` / `integrity` / `versioning` / `config-confers` / `degradation` / `audit` / `stale` / `offload` / `disk` / `adaptive-ttl` / `warmup` 等进阶能力的逐项说明见 [API 参考](docs/cn/API_REFERENCE.md)。

---

## 🚀 快速开始

### 📦 安装

```bash
cargo add oxcache            # 默认 minimal：L1 内存缓存
cargo add oxcache --features full   # 全量：L1 + L2 + 宏 + 压缩 + 批量 + Lua + 锁
```

**最低要求**：Rust 1.97.1+（edition 2024）。默认 `minimal` 特性仅启用 L1 内存缓存；使用 L2（Redis / Valkey / Dragonfly / Aerospike）需本地可达的对应服务，Redis 相关测试依赖 Docker（testcontainers 自动拉起容器）。

### 💡 最小可运行示例

```rust
use oxcache::Cache;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: u64,
    name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 默认 Moka L1 内存后端
    let cache: Cache<String, User> = Cache::builder().build().await?;

    // 写入
    cache
        .set(&"user:1".to_string(), &User { id: 1, name: "Alice".into() })
        .await?;

    // 读取
    if let Some(user) = cache.get(&"user:1".to_string()).await? {
        println!("命中: {}", user.name);
    }

    // 删除
    cache.delete(&"user:1".to_string()).await?;
    assert!(cache.get(&"user:1".to_string()).await?.is_none());
    Ok(())
}
```

- `#[cached]` 宏用法与核心概念见[用户指南](docs/cn/USER_GUIDE.md)；更多安装方式见[用户指南的安装章节](docs/cn/USER_GUIDE.md#安装)；wasm32 实验性支持见[用户指南的 wasm32 章节](docs/cn/USER_GUIDE.md#-wasm32-支持实验性)；完整示例清单见[用户指南的示例总览](docs/cn/USER_GUIDE.md#-示例总览)。

---

## 🎨 特性标志

层级预设（`default = ["minimal"]`，仅 L1）：

```toml
oxcache = { version = "0.5.1", features = ["minimal"] }   # 仅 L1（默认）
oxcache = { version = "0.5.1", features = ["redis-tier"] } # L1 + L2 Redis（旧名 core 为兼容别名）
oxcache = { version = "0.5.1", features = ["full"] }      # 真全量：全部具名能力特性
```

完整特性标志表（逐项说明与默认值）见 [API 参考](docs/cn/API_REFERENCE.md#-特性标志)。

---

## 📚 文档

| 文档 | 说明 |
|------|------|
| [📖 用户指南](docs/cn/USER_GUIDE.md) | 从安装到进阶的完整使用教程 |
| [📘 API 参考](docs/cn/API_REFERENCE.md) | 全部公开 API 的详细说明与错误码表 |
| [🏗️ 架构文档](docs/cn/ARCHITECTURE.md) | 设计理念、模块职责、数据流与故障处理 |
| [📊 性能基线](docs/cn/PERFORMANCE.md) | 序列化体积、压缩率、热路径基准与复现命令 |
| [🧪 测试场景矩阵](docs/cn/TEST_SCENARIOS.md) | 功能域到测试落点的映射与执行口径 |
| [🤝 贡献指南](docs/cn/CONTRIBUTING.md) | 开发环境、TDD 工作流与 PR 流程 |
| [📋 更新日志](docs/cn/CHANGELOG.md) | 每个版本的变更记录 |
| [📦 在线 API 文档](https://docs.rs/oxcache) | docs.rs 自动生成的最新文档 |
| [📦 crates.io](https://crates.io/crates/oxcache) | 发布页面 |

---

## 💻 示例

`examples/` 目录包含 **37 个示例**（默认特性下全部可运行）；分组清单与运行命令见[用户指南的示例总览](docs/cn/USER_GUIDE.md#-示例总览)。同步 API 的完整方法表与运行时要求见 [API 参考](docs/cn/API_REFERENCE.md#-同步-api)。

---

## 🏗️ 架构

Oxcache 采用「统一接口 + 可插拔后端」的分层设计：应用只面对 `Cache<K, V>` 一个类型安全入口，L1/L2 后端可经 `ChainCache` 按分数组链并按需回填。分层架构图、模块职责、数据流、TTL 行为对照表与布隆过滤器穿透防护见[架构文档](docs/cn/ARCHITECTURE.md)。

---

## 🧪 测试

2000+ 测试函数（截至 0.5.1）。测试层级、函数计数与常用命令见[测试场景矩阵](docs/cn/TEST_SCENARIOS.md#7-测试层级与常用命令)；Redis 相关测试依赖本机 Docker（testcontainers 自动拉起容器）。

---

## 📊 性能

热路径借用键 API（`get_by_str` / `set_by_str`）实测 get **-6.7%**、set **-12.7%**；postcard 混合负载传输体积约为 JSON 的 **46%**。基准与复现命令见[性能基线](docs/cn/PERFORMANCE.md)。

---

## 🔒 安全

Oxcache 在库层面内建多层防御，防线总览见[安全文档](docs/cn/SECURITY.md#-库内防线总览)。**漏洞报告**：请勿通过公开 Issue 报告安全漏洞，请使用 GitHub [Security Advisories](https://github.com/Kirky-X/oxcache/security/policy) 私密披露通道；完整政策见[安全文档](docs/cn/SECURITY.md)。

---

## 🗺️ 开发路线图

当前路线图见 [UNRELEASED.md 的开发路线图](docs/cn/UNRELEASED.md#开发路线图)。

---

## 🤝 参与贡献

欢迎提交 Pull Request 和 Issue！开发环境、TDD 工作流、Git Hooks 与 PR 流程见[贡献指南](docs/cn/CONTRIBUTING.md)。

---

## 📋 更新日志

完整版本历史见 [CHANGELOG.md](docs/cn/CHANGELOG.md)，未发布变更见 [UNRELEASED.md](docs/cn/UNRELEASED.md)。

---

## 📄 许可证

本项目基于 [MIT](LICENSE) 许可证发布。

Copyright (c) 2025-2026 Kirky.X🌠

---

## 🙏 致谢

Oxcache 构建在众多优秀的开源项目之上：

- [Moka](https://github.com/moka-rs/moka) 与 [DashMap](https://github.com/xacrimon/dashmap) ，L1 内存缓存后端
- [redis-rs](https://github.com/redis-rs/redis-rs) ，Redis / Valkey / Dragonfly / Sentinel / Cluster 客户端
- [aerospike-client-rust](https://github.com/aerospike/aerospike-client-rust) ，Aerospike 后端
- [Tokio](https://github.com/tokio-rs/tokio) 与 [Serde](https://github.com/serde-rs/serde) ，异步运行时与序列化生态
- [trait-kit](https://github.com/Kirky-X/trait-kit) ，AsyncKit 集成（生命周期 / 健康检查 / 关闭协调）
- [testcontainers-rs](https://github.com/testcontainers/testcontainers-rs) 与 [Criterion](https://github.com/bheisler/criterion.rs) ，集成测试与基准测试基础设施

---

## 📞 联系与支持

- **Issue 反馈**：[GitHub Issues](https://github.com/Kirky-X/oxcache/issues)（提供 Bug 报告 / 功能建议 / 问题咨询三类模板）
- **安全漏洞**：请勿通过公开 Issue 报告安全漏洞，请使用 GitHub Security Advisories 私密披露通道
- **维护者**：Kirky.X

---

## ⭐ Star 历史

[![Star History Chart](https://api.star-history.com/svg?repos=Kirky-X/oxcache&type=Date)](https://star-history.com/#Kirky-X/oxcache&Date)

### 💝 支持本项目

如果这个项目对你有帮助，请考虑给它一个 ⭐️！

**Made with love by Kirky.X**
