// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// This file should FAIL to compile: a constant key template on a method (with a
// `self` receiver) carries no ownership dimension — every tenant would share
// the same constant key.

use oxcache::cached;

struct Tenant;

impl Tenant {
    #[cached(service = "probe12_constant_key_svc", key = "user_profile")]
    async fn profile(&self) -> Result<u64, String> {
        Ok(1)
    }
}

fn main() {}
