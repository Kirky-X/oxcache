// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// This file should FAIL to compile because `123` is not a valid
// `Meta` argument for `#[cached]`. After T001 the macro must emit a
// `compile_error!` with a span pointing at the bad argument, instead
// of panicking inside `parser.parse(...).expect(...)`.

use oxcache::cached;

#[cached(service = "invalid_arg_test", 123)]
fn bad_fn() -> Result<(), ()> {
    Ok(())
}

fn main() {}
