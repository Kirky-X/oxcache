// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// This file should FAIL to compile: `skip` references `nope`, which is not
// a parameter of `bad_skip_fn` (only `a` exists). The macro must emit a
// `compile_error!` naming the unknown parameter.

use oxcache::cached;

#[cached(service = "skip_unknown_test", skip(nope))]
fn bad_skip_fn(a: u64) -> Result<u64, String> {
    Ok(a)
}

fn main() {}
