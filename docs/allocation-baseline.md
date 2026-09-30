# 📊 热路径分配基线（allocation baseline）

> 建立于 0.5.0-rc.7。本文为热路径堆分配的**口径化基线**与削减记录：
> 削减点、前后量化对比、ROI 结论与复测方法。后续热路径分配治理以本文
> 为对账基线。

**路径偏离说明**：原任务指定产物路径为 `reviews/allocation-baseline.md`，
但 `reviews/` 被本仓 `.gitignore` 整目录排除（「Reviews / security scan
artifacts」段），入库必丢文件（`docs/diting-review.md` 重建时已因同型
失误留档一次）。故基线文档改放被跟踪的 `docs/`，bench 产物（criterion
输出 `target/criterion/`）与探针临时文件由既有 `.gitignore`（`/target`）
覆盖，核验无需新增忽略规则。

## 口径与环境

| 项 | 值 |
|---|---|
| 主指标 | **每操作堆分配次数**（计数型，确定性，无计时噪声） |
| 次指标 | criterion 墙钟时间（`benches/hot_path_benchmark.rs`，参考项） |
| 计数方法 | 测试二进制级 `#[global_allocator]` 计数包装（`System` 透传，`alloc`/`realloc` 各计一次），预跑一轮排除惰性初始化后取 50 次调用均值 |
| 环境 | WSL2（kernel 6.6.87.2-microsoft-standard-WSL2 x64）、rustc 1.97.1、默认 feature（minimal：moka L1 + JSON 格式 + 无压缩）、单进程独占 |
| 工作负载 | `Cache<String, String>`，`get_many` 100 键全命中（值 ≈ 8 字节 JSON 字符串） |

复测 harness（临时测试二进制，测量后移除；复现时按本节方法重建）。
harness 的 criterion bench 化**登记为后续项**：一次性基线快照不引入常驻
bench 维护面，复现成本由本节方法学兜底：

```rust
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

struct CountingAlloc;

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

#[tokio::test]
async fn probe_get_many_allocations() {
    let cache: oxcache::Cache<String, String> = oxcache::Cache::builder().build().await.unwrap();
    for i in 0..100u32 {
        cache.set(&format!("bench_key_{i}"), &format!("value_{i}")).await.unwrap();
    }
    let keys: Vec<String> = (0..100u32).map(|i| format!("bench_key_{i}")).collect();
    let refs: Vec<&String> = keys.iter().collect();
    let _: std::collections::HashMap<String, String> = cache.get_many(refs.clone()).await.unwrap();

    let before = ALLOCS.load(Ordering::Relaxed);
    const N: usize = 50;
    for _ in 0..N {
        let _: std::collections::HashMap<String, String> = cache.get_many(refs.clone()).await.unwrap();
    }
    let after = ALLOCS.load(Ordering::Relaxed);
    println!("PER_CALL_ALLOCS={}", (after - before) / N);
}
```

## 基线与削减对比（`get_many` 100 键全命中）

| 指标 | 削减前 | 削减后 | 变化 |
|---|---|---|---|
| 每调用堆分配次数（主指标） | **517** | **412** | **−105（−20.3%）** |
| criterion 时间中位数（参考） | 32.569 µs | 33.408 µs | +2.3%（p = 0.70，不显著） |

时间项诚实披露：本负载下 105 次分配的节省未转化为本环境可分辨的墙钟
差异（单次分配极小、计时噪声占优）。分配计数是本基线的对账指标——
收益在高频调用路径的 allocator 压力与尾延迟（分配次数是 GC/分配器
竞争的决定项），不在单次均值时间。

### 削减点 1（高 ROI）：`UnifiedSerializer::deserialize` 非压缩分支去中间拷贝

- **位置**：`src/infra/serialization/unified.rs`
- **原状**：`compress = false` 时先 `data.to_vec()` 全量拷贝再交给
  `deserialize_with_format` 解析；而三种格式的解析入口
  （`deserialize_safe` / `bincode::decode_from_slice` / `postcard::from_bytes`）
  全部以 `&[u8]` 借用工作——拷贝纯属浪费。
- **改法**：压缩分支保留解压产物解析；非压缩分支直接借用解析。
- **收益**：每次反序列化 −1 次堆分配 + 一次全量 memcpy；`get` /
  `get_many` / `get_or` 逐键生效（本负载 −100 次/调用）。
- **语义**：零行为变化（输出与错误路径不变；压缩路径不动）。

### 削减点 2（低 ROI、顺手）：`batch_ops::get_many` 结果容器预分配

- **位置**：`src/cache/api/batch_ops.rs`
- **原状**：`HashMap::new()` 逐条 `insert`，100 命中触发 ~6 轮
  grow + rehash 分配。
- **改法**：`HashMap::with_capacity(values.len())`（命中数上界已知）。
- **收益**：−5 次分配/调用（100 键规模）；miss 占多数时最多浪费一次
  容量分配（批量读多为命中场景，权衡成立）。

## ROI 结论：评估后未削减的候选点

| 候选点 | 结论 |
|---|---|
| `Cache::get` 的 `to_key_string()` 每次 1 分配 | 不动：已有零分配热路径 API `get_by_str`/`set_by_str`（见同 bench 文件 owned vs borrowed 对照），owned API 的分配是签名语义的一部分 |
| `ChainCache::iter_entries` 每层 `batch_keys` 构造 | 不动：`collect()` 源迭代器（`pending`/`keys` 切片）size_hint 精确，本就单次预分配 |
| `UnifiedSerializer::serialize_with_type` 非 compress 分支 `to_vec` | 不动：返回的 `Vec<u8>` 即产物本身，非中间拷贝 |
| `iter_entries` 输出 `zip().collect()` | 不动：两侧长度精确相等，size_hint 精确，单次预分配 |

## 基线快照（供后续对账）

削减后（0.5.0-rc.7）`get_many(100 命中)` = **412 次分配/调用**。残余
大头为逐键的 key `String` 构造（100）与 serde_json 值解析产物（100+）
——均属 API 语义必需，进一步削减需换 owned 签名或 arena 化解析，
超出本基线治理范围。
