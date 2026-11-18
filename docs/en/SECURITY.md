# 🔒 oxcache Security Document

This document describes oxcache's supported versions, vulnerability reporting process, and supply-chain gates. Every entry has been checked item by item against the repository's existing configuration (`Cargo.toml`, `.github/workflows/`, `.cargo/audit.toml`, `deny.toml`, `.pre-commit-config.yaml`, `src/lib.rs`).

## 📌 Supported Versions and Maintenance Policy

The project is in its 0.x pre-release stage and **maintains only the current minor version line** (`0.5.x`, including the rc pre-releases within that line).

- **Current line**: receives defect and security fixes; fixes ship with the latest release of that line.
- **Older minor lines**: enter EOL as soon as the first release of the next minor line is published; no further fixes of any kind, and no backporting of security patches.
- **No LTS**: before 1.0 there is no cross-minor API compatibility promise; minor-version upgrades may contain breaking changes. Read the [CHANGELOG](CHANGELOG.md) before upgrading.
- **Visible on crates.io ≠ supported**: old versions remain downloadable (unless yanked for a severe defect) but are out of the supported scope.

| Version line | Status | Notes |
|--------------|--------|-------|
| `0.5.x` (current) | ✅ Supported | The only line receiving fixes |
| `≤ 0.4.x` | ❌ EOL | Superseded by the current line, unmaintained; please upgrade |

MSRV is **Rust 1.97.1** (root `Cargo.toml` `[workspace.package] rust-version`), edition 2024.

## 🐛 Vulnerability Reporting Process

**Do not report security vulnerabilities through public GitHub Issues.**

1. **Primary channel**: submit the report through the private [GitHub Security Advisories](https://github.com/Kirky-X/oxcache/security/policy) disclosure channel (repository page **Security → Report a vulnerability**).
2. **Fallback channel**: only if the primary channel is unavailable, email [Kirky-X@outlook.com](mailto:Kirky-X@outlook.com) (handler: Kirky.X).

Reports should include, wherever possible:

- Affected versions and feature combinations (e.g. `--features full`, `redis`/`valkey`/`dragonfly`/`aerospike` backend selection, `serde-bincode` and other serialization features)
- Reproduction steps or a minimal example (including the L1/L2/L3 tier and key shape)
- Impact assessment (CWE classification if determinable)
- Feasible temporary mitigations

### ⏱️ Response Commitments

| Stage | Timeline |
|-------|----------|
| Acknowledgement | within 48 hours |
| Initial assessment (accept / reject / need more info) | within 7 days |
| Fix for an accepted vulnerability | released as soon as possible depending on severity |
| Coordinated disclosure | date agreed with the reporter once the fixed version is available |

Before the fixed version is released, the reporter may get a chance to preview and verify the patch; the form of acknowledgement (anonymity possible) is negotiated with the reporter.

## 🔗 Supply Chain and Gates

The following gates are actually configured and enforced in the repository:

| Gate | Location | Content |
|------|----------|---------|
| Format and Lint | `ci.yml` · `fmt` / `clippy` | `cargo fmt --check`; `cargo clippy -- -D warnings` (zero warnings) |
| Compile and feature matrix | `ci.yml` · `check` / `feature-redis-tier` / `feature-minimal`; `feature-matrix.yml` | Compile checks including the Redis tier and minimal feature combinations |
| Tests | `ci.yml` · `test` / `test-critical-combinations` | Full test suite and critical feature-combination tests |
| Dependency vulnerability audit | `ci.yml` · `security`; `dependency-check.yml` | `cargo audit` (reads `.cargo/audit.toml`), `cargo deny check` |
| Static security analysis | `codeql.yml` | CodeQL Rust analysis |
| Coverage gate | `ci.yml` · `coverage` | `cargo llvm-cov --features full --workspace --fail-under-lines 90` |
| Local hooks | `.pre-commit-config.yaml` | `detect-private-key`, `detect-secrets`, `typos`, `cargo fmt` / `check` / `clippy` (local hooks) |
| Dependency and license governance | `deny.toml`, `.cargo/audit.toml` | License / bans / advisory checks and audit exemptions |
| Dependency updates | `.github/dependabot.yml` | Weekly tracking of the cargo / github-actions ecosystems |
| Unsafe-code constraint | `src/lib.rs` | `#![deny(unsafe_code)]` |

### Known Exemptions (Transparent Disclosure)

`.cargo/audit.toml` and `deny.toml` record the following single RustSec exemption with the same policy, for a form that is **reachable in the dependency tree but not exploitable / without an upgrade path**:

- `RUSTSEC-2026-0440` (astral-tokio-tar marked unmaintained across all versions): pulled in dev-only via `testcontainers` 0.28.0 (the latest crates.io release; the advisory states "No safe upgrade is available"); it only packages test-container image layers, never entering the lib dependency surface or published artifacts; the exemption will be removed once testcontainers migrates to tar-codec upstream.

Exemptions do not constitute security behavior changes; dependency failure and degradation semantics are described in the [🏗️ Architecture documentation](ARCHITECTURE.md).

## ✅ Consumer-Side Security Recommendations

- In production, enable backends with the minimal feature set to avoid exposing unused connection surfaces; connection strings and credentials always come from environment variables or a secret service, never as plaintext in configuration files.
- When cache keys are built from untrusted input, apply length and character-set constraints before insertion (`validate_redis_key`, `redact_value` / `redact_connection_string` — see the `example_security` / `example_security_validation` examples).
- Watch the output of `cargo audit` and `cargo deny`, and keep dependencies and the toolchain up to date.

## 📚 Related Documents

- [🤝 Contributing Guide](CONTRIBUTING.md) — development environment and submission process
- [📜 Changelog](CHANGELOG.md) — security fix records
- [📖 User Guide](USER_GUIDE.md) — configuration and secure usage
- [🏠 README](../../README.md) — capability list and built-in security notes

## 🛡️ In-Library Defense Overview

> This section was migrated from the README security chapter: a quick overview of the multiple layers of defense built into the library.

| Defense layer | Mechanism |
|---------------|-----------|
| Key validation | `validate_redis_key`: rejects empty keys, keys over 512 KB, and keys containing `\r` / `\n` / `\0`; scans for SQL-injection and path-traversal patterns |
| Lua sandbox | `validate_lua_script`: 10 KB limit, 100-key limit, dangerous commands judged by the **invoked command name** (`FLUSHALL` / `CONFIG` / `SHUTDOWN` etc.; whitespace and string-concatenation variants are equivalent), commands whose name cannot be statically determined are rejected, comment and string preprocessing against bypass |
| SCAN limits | `validate_scan_pattern` (256 chars, max 10 wildcards) + `clamp_scan_count` (clamped to 1-1000) |
| TLS enforcement | `RedisBackend` requires `rediss://` by default unless the `OXCACHE_ALLOW_INSECURE_REDIS` development escape hatch is set explicitly |
| Redaction | `redact_connection_string` / `redact_value` / `redact_cache_key` / `Redacted` wrapper; keys in telemetry spans, audit events and `CacheEvent` are redacted by default (whole key masked when the key text matches the sensitive-word list; identifier-shaped segments — e-mail, numeric IDs, UUIDs, long hex digests — masked as `<id>`; audit events and cache events share one format) |
| Memory safety | Crate-root `#![deny(unsafe_code)]` |
| Value protection | `encrypt` (XChaCha20-Poly1305 with the key bound as AAD) and `integrity` (HMAC-SHA256 with the tag bound to key name + value) decorators |
| Namespace boundary | `namespace-guard` decorator rejects keys/patterns outside the declared namespace with `AccessDenied`; `clear()` is rejected under `Isolated`; per-namespace quotas bound entries and bytes |
| Deserialization DoS guard | `MAX_JSON_DEPTH` (64 levels) depth limit + `MAX_JSON_SIZE` (5 MB) deserialization size cap + 64 MiB decompressed-output cap (decompression-bomb guard) + stack-based recursion (`serde_stacker`) |
| Supply chain | Every third-party GitHub Action pinned by commit SHA in CI; `cargo deny check` (advisories / licenses / duplicate deps, see `deny.toml`) and `cargo audit` always on |

---

Maintainer: Kirky.X.
