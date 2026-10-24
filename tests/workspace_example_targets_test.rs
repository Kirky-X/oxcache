// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Workspace 清单守卫：example 目标名不得跨包重复。
//!
//! 同一 workspace 的各包共享 target 目录，同名 example 目标落到同一个输出
//! 文件（`target/debug/examples/<name>.exe`）。cargo 对此只发 "output filename
//! collision" 警告，两个目标并行链接时后写者抢不到输出文件（Windows 上
//! LNK1104），`cargo test --workspace` 整体以 exit 101 失败。
//!
//! 本测试把该约束钉死：同一 workspace 内各包的 `[[example]]` 目标名必须唯一。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 读取根清单 `[workspace] members` 列出的成员
fn workspace_members(root_manifest: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(root_manifest).expect("root manifest readable");
    let mut array = String::new();
    let mut collecting = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if !collecting {
            if !trimmed.starts_with("members") {
                continue;
            }
            let Some(open) = trimmed.find('[') else {
                continue;
            };
            collecting = true;
            array.push_str(&trimmed[open..]);
        } else {
            array.push_str(trimmed);
        }
        if array.contains(']') {
            break;
        }
    }
    array
        .trim_matches(|c| c == '[' || c == ']')
        .split(',')
        .filter_map(|item| {
            let item = item.trim();
            item.strip_prefix('"')?
                .strip_suffix('"')
                .map(str::to_string)
        })
        .collect()
}

/// 读取清单中的 `[[example]]` 目标名
fn example_target_names(manifest: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(manifest).expect("manifest readable");
    let mut names = Vec::new();
    let mut in_example = false;
    for line in text.lines() {
        let line = line.trim();
        if line == "[[example]]" {
            in_example = true;
            continue;
        }
        if line.starts_with('[') {
            in_example = false;
            continue;
        }
        if !in_example {
            continue;
        }
        if let Some(rest) = line.strip_prefix("name") {
            let rest = rest.trim_start().trim_start_matches('=').trim();
            if let Some(name) = rest.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
                names.push(name.to_string());
            }
        }
    }
    names
}

/// 同一 workspace 内 example 目标名唯一
#[test]
fn workspace_example_target_names_are_unique() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root_manifest = base.join("Cargo.toml");
    let mut owners: HashMap<String, String> = HashMap::new();
    for member in workspace_members(&root_manifest) {
        let manifest = base.join(&member).join("Cargo.toml");
        for name in example_target_names(&manifest) {
            if let Some(previous) = owners.insert(name.clone(), member.clone()) {
                panic!(
                    "example target '{name}' is registered twice ({previous} and {member}): \
                     same-workspace targets share one output file, the later link fails \
                     (Windows LNK1104) and `cargo test --workspace` exits 101"
                );
            }
        }
    }
    assert!(
        !owners.is_empty(),
        "no example target found in the workspace manifests"
    );
}
