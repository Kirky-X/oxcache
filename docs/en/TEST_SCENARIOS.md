# 🧪 Oxcache Test Scenario Matrix

> Feature domain → test target map + E2E gap-filling records.
> Execution policy: always use the project's own test framework `cargo test` (standalone scripts are forbidden as substitutes);
> integration/E2E tests depend on real Docker containers (testcontainers self-start/self-clean / the `tests/real_env/`
> compose cluster); test doubles (mockall etc.) are forbidden; real in-process implementations plus fault-injection
> stubs (chaos-style `FailingBackend`) are the project's existing policy; assertions are pinned to real behavior,
> and when documentation guesses diverge from the implementation the docs are corrected with the evidence noted.
>
> **Snapshot policy**: the data in this document is a measured snapshot taken during that acceptance round; for the
> current version, the test-function counts are governed by
> [Section 7](#7-test-layers-and-common-commands) and CI (features removed after the snapshot, such as `cli`,
> keep their snapshot rows for the record only).

## 1. Test Pyramid Baseline (all-features measured)

| Layer | Entry | Result |
|---|---|---|
| Unit | `cargo test --lib` | 1132 passed / 0 failed / 120 ignored (ignored gap-filling in §4) |
| Integration | `tests/unit.rs` | 327 passed |
| Integration | `tests/integration.rs` | 137 passed + 4 --ignored gap-fills all passed (1 first-round valkey failure was a transient image pull; re-verified 8/8 pass) |
| E2E | `tests/e2e.rs` | 75 passed (72 existing + 3 new, see §3) |
| Security | `tests/security.rs` | 14 passed |
| Macro | `tests/macros.rs` (trybuild) | 10 passed (51.4s including compile-fail snapshot verification) |
| Feature | `tests/feature_test.rs` | 2 passed |
| Chaos | `tests/chaos/backend_failure_test.rs` | 19 passed |
| Bloom | `tests/bloom_filter_integration.rs` | 9 passed |
| Performance | `tests/performance.rs` | 19 passed + 5 --ignored gap-fills all passed |

examples: 37/37 run with rc=0 (the `error_handling`/`custom_backend`/`events` examples print designed error
demonstrations, matching the intended design policy).

## 2. Feature Domain → Target Matrix

| Domain | Scenario highlights | Existing target | Gap handling |
|---|---|---|---|
| In-memory backends moka/dashmap | read/write / expiry / TTL / TTI / capacity | lib inline + `tests/unit.rs` + e2e advanced/cache | No gap |
| Redis backend | connection / read-write / pipeline / reconnect | `tests/integration/redis/redis_client_comprehensive_test.rs` | No gap |
| Redis cluster | 6-node formation / sharding / failover | `tests/integration/redis/redis_cluster_test.rs` + real_env compose | No gap |
| Redis sentinel | master-slave switchover / discovery (after failover the same client re-discovers the new master and keeps serving reads/writes) | `tests/integration/redis/redis_sentinel_test.rs` + real_env compose | No gap |
| Dragonfly | compatibility mode / operation surface | `tests/integration/backend/dragonfly_test.rs` (6 tests) | No gap |
| Valkey | explicit mode / transparent reuse | `tests/integration/backend/valkey_test.rs` (8 tests) | No gap |
| Aerospike | container self-managed + access-address injection + full operation surface | `tests/integration/backend/aerospike_test.rs` (7 tests) | 11 lib-inline --ignored tests are redundant same-path coverage and CI never runs them; recorded as an environment boundary (§4.4) |
| Chained cache | ordering / backfill / race read / partial failure | `tests/integration/chain_cache_integration_test.rs` | No gap |
| Distributed lock | reentrancy / TTL / contention / renewal / release | `tests/integration/redis/dist_lock_test.rs` (§4.1 note) + new `tests/e2e/dist_lock_watchdog_e2e.rs` | Watchdog combination-semantics gap → OXL-WD-01/02/03 |
| Event system | event construction / pub-sub / error events | 21 inline in `src/core/events.rs` (component-level) + new `tests/e2e/events_chain_e2e.rs` | publisher→failure→subscriber integration chain gap → OXE-01/02/03 |
| Lua scripts | script loading / EVAL / atomicity | lib inline (8 --ignored gap-fills all passed) | No gap |
| Serialization | JSON/bincode/typed API | lib inline + `tests/unit.rs` | No gap |
| Batch operations | mget/mset/pipeline | lib inline + e2e advanced | No gap |
| Metrics | stats / Prometheus / JSON export | lib inline + `tests/unit.rs` | No gap |
| Compression | zstd/flate2 transparent compression | lib inline | No gap |
| Bloom filter | false-positive rate / scaling / integration | `tests/bloom_filter_integration.rs` | No gap |
| Procedural macros | derive/attribute macros/compile-fail snapshots | `tests/macros.rs` (trybuild) | No gap |
| Security | key injection / redaction / audit logs | `tests/security.rs` | No gap |
| Chaos | backend fault injection / degradation | `tests/chaos/backend_failure_test.rs` (`FailingBackend`) | No gap |
| kit integration | AsyncKit registration/config/build/health/lifecycle | 15 inline in `src/integrations/kit/` (module 7/observer 3/decorator 3/shutdown 2) | Coverage is thick; no e2e needed |
| i18n | multilingual error messages | 36 inline in `src/i18n/mod.rs` | Coverage is thick; no e2e needed |
| cli | FeatureSet boolean switches (cli_available) | lib inline | No standalone e2e necessity |
| promotion | ChainCache backfill concept demo | integration already covers backfill | No gap |
| feature_test | feature-gating existence | `tests/feature_test.rs` | No gap |

Test environment protocol (environment variables): `REDIS_URL` takes priority / `OXCACHE_SKIP_REDIS_TESTS` skips all /
`OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS` (non-TLS gate) /
`REDIS_CLUSTER_AVAILABLE` / `REDIS_SENTINEL_AVAILABLE` / `REDIS_SENTINEL_MASTER_URL`
(default `redis://127.0.0.1:16380`, aligned with the master host port of the `tests/real_env/docker-compose.sentinel.yml`
stack) / `REDIS_SENTINEL_ADDR_MAP` (NAT address mapping for sentinel mode,
`container address=host address,...`; the default is exactly that compose topology: sentinel reports container-internal
addresses, and host-side clients must map them to the published ports) / `REDIS_VERSION_TEST_ENABLED`.

## 3. E2E Gap Filling (newly landed)

### 3.1 `tests/e2e/dist_lock_watchdog_e2e.rs` (scenario IDs: OXL-WD-01/02/03)

Existing coverage reference: `dist_lock_test.rs`'s `test_dist_lock_watchdog_renew` already covers single-holder
renewal (ttl=300ms, `is_held` still true after sleeping 500ms); the other 4 tests all use
`watchdog_enabled(false)`. This file covers the watchdog **combination semantics**:

- **OXL-WD-01** renewal keeps mutual exclusion: A (ttl=1s, watchdog renews every ttl/3≈333ms) holds the lock
  across 2.5s (>2×TTL) and B is still rejected, with the error text containing "already held";
- **OXL-WD-02** release stops renewal and hands over immediately: after A releases, B's acquire succeeds immediately;
- **OXL-WD-03** the watchdog has indeed stopped after release: after TTL expiry `is_held`=false and
  `backend.exists`=false, with no "resurrecting" renewals of the key.

### 3.2 `tests/e2e/events_chain_e2e.rs` (scenario IDs: OXE-01/02/03)

Existing coverage reference: the 21 inline tests in `src/core/events.rs` are component-level semantics; the
`emit_backend_error` call sites are covered by unit tests. The **tests layer previously had zero coverage** of the full chain
publisher config → real backend failure → `publish_error` arrival. Faults are injected by stopping a real Redis container
(real failure path, not a mock):

- **OXE-01** set: L2 failure → partial-success semantics (`Ok(())`) + event arrival;
- **OXE-02** get: an L1 hit does not touch L2 (no new events by contrast); L1 miss + L2 failure →
  degrades to `Ok(None)` + event arrival;
- **OXE-03** delete: L2 failure → partial-success semantics + event arrival.

Event contract assertions: each of the three failure paths produces exactly one `publish_error`, formatted
`"backend {name}: {error}"`, with key `Some(original key)`.

## 4. Real-Behavior Corrections and Findings

1. **Container lifecycle discovery (important, fixed)**: testcontainers 0.28's `ContainerAsync` deletes the container
   on drop. The existing pattern in `dist_lock_test.rs` locked the container inside `setup()`'s local scope; after
   return, the first connection operation always hit `broken pipe`, silently swallowed by the existing `ok_or_skip!`
   macro, so that file's assertions were effectively not executed in this environment (appearing "green"). Fix: `setup`
   returns the container handle and the test body holds it until the end, `ok_or_skip!` removed, container gating moved
   to `container_or_skip`/`backend_skip` (fail-closed under `OXCACHE_TEST_STRICT`),
   assertions fail explicitly via `expect`; the new e2e does the same (see the
   `dist_lock_watchdog_e2e.rs::setup` doc comment).
2. **`read_from_chain` degradation semantics corrected**: when L1 clearly misses and L2 fails, the result is
   `Ok(None)` (an explicit miss on some backend does not fail the whole read) rather than `Err`; `Err` propagates
   only when all backends fail. The OXE-02 assertions are pinned to this real behavior.
3. **valkey transient failure**: the one first-round failure in `valkey_test.rs` was a race on the first valkey image
   pull (`expect` on connection failure); standalone re-verification passed 8/8; recorded as a flaky observation, not a code defect.
4. **aerospike lib --ignored environment boundary**: the 11 lib-inline --ignored tests and
   `aerospike_test.rs` (7 non-ignored, real container + injection + full operation surface) are redundant same-path
   coverage; the combination of the client's `wait_till_stabilized` breaking early on first-round tend nodes=0 plus
   `fail_if_not_connected=true` cannot reliably reproduce container readiness inside the lib process, and CI never
   runs `--ignored` (the default `--lib` scope 1132/0 meets the bar). Recorded as a known environment boundary, not a code defect.
5. **real_env stack config resolution when started with the repo root as project dir (fixed)**: compose's
   relative mount `./configs/*.conf` resolves against the project dir; workflow scripts cannot `cd`, so when started
   with the repo root as project dir the mounts land in the root `configs/` — the missing path is created as an empty
   directory by docker, sentinel has no template to copy (`cp: -r not specified` exit) and replicas lose `replicaof`
   and disconnect. Fix: the root `configs` directory now carries the same content as `tests/real_env/configs` (both
   tracked in git), so both the repo root and `tests/real_env` start directories resolve.

## 5. Tiered Feature Combination Matrix (CI policy replica)

Combination set (`.github/workflows/ci.yml` "Test (critical combinations)", currently 9 combinations):
`minimal,macros` / `minimal,bloom` / `minimal,compression` / `redis-tier,macros` /
`redis-tier,bloom` / `redis-tier,compression` / `redis-tier,batch` / `redis-tier,lua` /
`redis-tier,bloom,macros`; the three base tiers `minimal` / `redis-tier` / `full` (and `adaptive-ttl`)
are covered by separate Test / Clippy jobs. At snapshot acceptance there were 10 combinations (including `core,cli`;
after the `cli` feature was removed in rc.4 that combination was withdrawn; `core` in the old combination is the
compatibility alias of `redis-tier`, recorded here under the canonical name).
Policy: `cargo test --features "<combination>" --workspace`
(on top of the default minimal, same as CI). Execution results were recorded in the acceptance ledger (`reviews/acceptance-report.md`; the `reviews/` directory is gitignored and not retained in the repository).

## 6. Static Gates

| Gate | Command | Requirement |
|---|---|---|
| Format | `cargo fmt --all -- --check` | zero diff |
| Lint | `cargo clippy --all-targets --all-features --workspace -- -D warnings` | zero warnings |
| Docs | `cargo doc --no-deps --all-features` (`-D warnings`) | zero warnings |
| Supply chain | `cargo deny check` / `cargo audit` | no high severity |
| MSRV | `rust-version = "1.97.1"` (workspace.package) | compiles with toolchain 1.97.1 |

## 📚 Related Documents

- [📖 User Guide](USER_GUIDE.md): usage-perspective explanation of feature behavior
- [🤝 Contributing Guide](CONTRIBUTING.md): day-to-day execution of the TDD workflow and static gates
- 🔒 Security document: the security mechanisms covered by `tests/security.rs`
- [Section 7: Test layers and common commands](#7-test-layers-and-common-commands): current-version test-function counts and common commands

## 7. Test Layers and Common Commands

> This section was migrated from the README testing chapter: a workspace grep-policy snapshot of test-function counts plus the CI-identical common commands.

The test suite is organized as described in [`tests/README.md`](../../tests/README.md); the scenario matrix is in Section 2 above.

| Layer | Entry point | Coverage | Test functions¹ |
|-------|-------------|----------|-----------------|
| Library unit tests | `--lib` | `#[cfg(test)]` tests inside `src/` | 1671 |
| Unit tests | `--test unit` | Backend interfaces, CacheBuilder, serialization, metrics, log redaction, etc. | 332 |
| Integration tests | `--test integration` | Batch writes, chained cache, degradation & recovery, TTL, Redis Cluster / Sentinel (incl. failover re-discovery), distributed locks, etc. | 153 |
| End-to-end tests | `--test e2e` | Basic operations, `#[cached]` macro, real-world scenarios, advanced scenarios | 65 |
| Macro tests | `--test macros` | `sync` / `skip_cache_write` modes and trybuild compile-fail cases | 24 |
| Security tests | `--test security` | Security coverage and security validation | 20 |
| Chaos tests | `--test chaos` | Backend failure injection, network failures, random failures | 19 |
| Performance tests | `--test performance` | Memory leak detection, Miri memory safety, pipeline performance | 19 |
| Feature gating | `--test feature_test`; `--features "full,bloom" --test bloom_filter_integration` | Narrow feature combinations, bloom filter integration | 2 + 7 |

> ¹ `#[test]` / `#[tokio::test]` function counts via grep (`grep -rEo '#\[(tokio::)?test\]' --include='*.rs' src tests | wc -l`), worktree counts as of **0.5.1** (re-verified 2026-11-16); 2415 in total (`src/` 1673 + `tests/` 742). Per-layer rows are grep counts per test directory (641 in total); the remaining 101 come from 22 top-level test files not listed above (`tests/*_extra_test.rs` supplementary files, `scenario_probe_*.rs` probes and top-level `*_test.rs` cases).

### Common Commands (same as CI)

```bash
# The CI test matrix runs at minimal / redis-tier / full and more tiers
cargo test --features full --workspace

# Run by test binary
cargo test --features full --lib
cargo test --features full --test integration
cargo test --features full --test e2e

# Narrow feature-combination checks (CI feature-redis-tier / feature-minimal jobs)
cargo check -p oxcache --no-default-features --features redis-tier
cargo check -p oxcache --no-default-features --features minimal

# Skip tests that require Redis
cargo test --features full -- --skip redis

# Coverage (CI and pre-push gate: line coverage >= 90%)
cargo llvm-cov --features full --workspace --fail-under-lines 90
```

> Redis-related tests start a `redis:7-alpine` container automatically via testcontainers and require a local Docker environment; integration / E2E tests forbid test doubles and use real in-process implementations plus chaos-style fault-injection stubs (policy in Section 2).
