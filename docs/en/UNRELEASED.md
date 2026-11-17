# Unreleased Changes

> This file records changes that have not yet been released; see [CHANGELOG.md](CHANGELOG.md) for the released version history.

## Unreleased

## Development Roadmap

> This section was migrated from the README roadmap chapter.

| Status | Item | Notes |
|:------:|------|-------|
| ✅ | **0.5.0 stable release** | Landed: 0.5.0 published on 2026-11-14 and 0.5.1 on 2026-11-15 (tags pushed; `release.yml` published to crates.io) |
| 📋 | **Downstream version propagation** | dbnexus, inklog, limiteron, and sdforge sync their oxcache dependency requirement to 0.5 (path + version dual declaration) |
| ✅ | **Valkey integration test environment gating** | 8 Valkey and 6 Dragonfly integration tests gated via `container_or_skip`: skipped with a reason when Docker is absent, and CI sets `OXCACHE_TEST_STRICT=1` to fail closed; semantics in tests/README.md "Container availability gating" section |
| ✅ | **Follow-up on archived review findings** | All 5 items closed (2 Medium + 1 Low fixed in the original round; the remaining items closed in the 2026-09-30 backlog-fix round) |
