# oxcache 性能基线（docs/PERFORMANCE.md）

> 本机基线记录。运行环境：WSL2 (linux 6.6)、Rust 1.97.1、debug profile（除非另有说明）。
> CI 阈值门禁待基线稳定后启用（design D4 口径）。

## 序列化格式 L2 传输体积对比（T305）

格式选择（`serde-bincode` / `postcard` feature，`CacheBuilder::serialization_format()`）。
代表负载（`Sample { id: u64, name: String, tags: Vec<String>×3, score: f64 }`）与
数值密集负载（6 个 64 位数值字段）的序列化字节数：

| 负载 | JSON | bincode 1.x | postcard 1.x |
| --- | --- | --- | --- |
| Sample（短字符串混合） | 69 B | 74 B | 32 B |
| NumericHeavy（6×64 位数值） | 84 B | 48 B | — |

结论（以真实负载度量为准）：

- **postcard**（varint）对混合负载最紧凑（约为 JSON 的 46%）；
- **bincode 1.x** 默认 varint 编码，数值密集场景约为 JSON 的 57%，
  但短字符串场景可能略大于 JSON（长度前缀 + 无字段名压缩）；
- JSON 优势是可读性与跨语言互操作；**同一键前缀不得混用格式**（无自描述头）。

复现：`cargo test --lib --features serde-bincode,postcard formats_are_interoperable -- --nocapture`

## 自适应 zstd 压缩（T310）

`CompressingBackend`（`compression` feature）：阈值 256 B、zstd level 3、
可压缩重复负载（MockBackend 存储侧字节数）：

| 原始大小 | 存储大小 | 压缩率 |
| --- | --- | --- |
| 1024 B | 37 B | 3.6% |
| 8192 B | 38 B | 0.5% |
| 65536 B | 38 B | 0.1% |

阈值以下零压缩开销（原样存储）；读取端按魔数自动识别 zstd / 兼容旧 gzip / 透传。
复现：`cargo test --lib --features compression size_comparison -- --nocapture`

## 热路径零分配（T317）

`get_by_str` / `set_by_str` 借用键 API（T317）：
`get` 路径每次调用省去 `K::to_key_string()` 的 String 分配（借用查询零堆分配）；
`set` 路径省去 String 中转（1 次 `Arc<str>` 分配 vs 原来的 String+Arc 两次）。

Criterion 基线（`benches/hot_path_benchmark.rs`，bench profile = release + lto=fat，Moka L1 命中路径）：

| 基准 | owned 键（既有 API） | borrowed 键（T317） | 差异 |
| --- | --- | --- | --- |
| get 命中 | 241.07 ns | 224.88 ns | **-6.7%** |
| set | 819.38 ns | 715.01 ns | **-12.7%** |

复现：`cargo bench --bench hot_path_benchmark`
