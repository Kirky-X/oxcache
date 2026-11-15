# ⚡ Oxcache Performance Baseline

> Local baseline record. Environment: WSL2 (linux 6.6), Rust 1.97.1, debug profile (unless stated otherwise).
> CI threshold gates will be enabled once the baseline stabilizes (design D4 policy).

For the architecture-level order-of-magnitude reference (L1/L2 throughput and latency estimates), see the benchmark chapter of the [🏗️ Architecture documentation](ARCHITECTURE.md); this document records reproducible measured data.

## 📦 Serialization Format L2 Transfer Size Comparison

Format selection (`serde-bincode` / `postcard` feature, `CacheBuilder::serialization_format()`).
Serialized byte counts for the representative payload (`Sample { id: u64, name: String, tags: Vec<String>×3, score: f64 }`) and
the numeric-heavy payload (six 64-bit numeric fields):

| Payload | JSON | bincode 2 | postcard 1.x |
| --- | --- | --- | --- |
| Sample (short-string mixed) | 69 B | 32 B | 32 B |
| NumericHeavy (6×64-bit numerics) | 84 B | 40 B | — |

Conclusions (measured on real payloads):

- **postcard** and **bincode 2** (`config::standard()`, varint encoding) are equally the most compact for the mixed payload
  (32 B, about 46% of JSON);
- **bincode 2** for the numeric-heavy case is about 48% of JSON (40 B / 84 B);
- JSON's advantages are readability and cross-language interop; **formats must not be mixed under the same key prefix**
  (no self-describing header).

Reproduce:

```bash
cargo test --lib --features serde-bincode,postcard formats_are_interoperable -- --nocapture
```

## 🗜️ Adaptive zstd Compression

`CompressingBackend` (`compression` feature): default threshold 1024 B (this benchmark passes 256 B explicitly
to cover small payloads), zstd level 3, compressible repetitive payload (bytes stored by MockBackend):

| Raw size | Stored size | Compression ratio |
| --- | --- | --- |
| 1024 B | 37 B | 3.6% |
| 8192 B | 38 B | 0.5% |
| 65536 B | 38 B | 0.1% |

Zero compression overhead below the threshold (stored as-is); the read side auto-detects zstd by magic bytes / stays compatible with legacy gzip / passes through.

Reproduce:

```bash
cargo test --lib --features compression size_comparison -- --nocapture
```

## 🔥 Hot-Path Zero Allocation

The `get_by_str` / `set_by_str` borrowed-key APIs:

- The `get` path saves the `String` allocation of `K::to_key_string()` per call (borrowed query, zero heap allocation);
- The `set` path saves the `String` intermediary (1 `Arc<str>` allocation instead of the previous String + Arc pair).

Criterion baseline (`benches/hot_path_benchmark.rs`, bench profile = release + lto=fat, Moka L1 hit path):

| Benchmark | Owned key (existing API) | Borrowed key (borrowed API) | Delta |
| --- | --- | --- | --- |
| get hit | 241.07 ns | 224.88 ns | **-6.7%** |
| set | 819.38 ns | 715.01 ns | **-12.7%** |

Reproduce:

```bash
cargo bench --bench hot_path_benchmark
```
