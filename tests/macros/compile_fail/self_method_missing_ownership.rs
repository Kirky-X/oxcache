// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// This file should FAIL to compile: a method (with a `self` receiver) that has
// neither `scope` nor an ownership-carrying `key` template produces a default
// key with no ownership dimension — different tenants would share one entry.

use oxcache::cached;

struct Tenant;

impl Tenant {
    #[cached(service = "probe12_missing_ownership_svc")]
    async fn profile(&self, id: u64) -> Result<u64, String> {
        Ok(id)
    }
}

fn main() {}
