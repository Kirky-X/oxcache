// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// This file should FAIL to compile: `skip` cannot be combined with an
// explicit `key` template (the template fully determines the cache key, so
// honoring skip would be impossible and ignoring it would conceal a
// configuration mistake).

use oxcache::cached;

#[cached(service = "skip_key_conflict_test", key = "u_{a}", skip(a))]
fn bad_skip_key_fn(a: u64) -> Result<u64, String> {
    Ok(a)
}

fn main() {}
