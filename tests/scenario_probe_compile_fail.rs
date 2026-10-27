// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 临时执行探针（E2E 场景 12）：带 `self` 接收者缺归属维度编译期报错 ——
//! trybuild compile_fail 快照校验。
//!
//! 依据：`docs/UNRELEASED.md:15-23`、`macros/src/lib.rs` 内联测试
//! （receiver_parameter_requires_scope_or_key / constant_key_template_on_method_rejected）。

#![cfg(feature = "macros")]

/// 方法（带 `self` 接收者）无 `scope` 且无 `key` → 编译期报错
#[test]
fn probe12_compile_fail_self_method_missing_ownership() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/macros/compile_fail/self_method_missing_ownership.rs");
}

/// 方法上的常量键模板（与 `self` 无关）→ 编译期报错
#[test]
fn probe12_compile_fail_constant_key_template_on_method() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/macros/compile_fail/self_method_constant_key.rs");
}
