//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Path validation configuration and utilities

use crate::error::{CacheError, Result};
use std::path::{Path, PathBuf};

/// 路径验证配置
#[derive(Debug, Clone)]
pub struct PathValidationConfig {
    /// 允许的基础目录
    pub allowed_base_dirs: Vec<PathBuf>,
    /// 是否允许符号链接
    pub allow_symbolic_links: bool,
    /// 最大路径长度
    pub max_path_length: usize,
}

impl Default for PathValidationConfig {
    fn default() -> Self {
        Self {
            allowed_base_dirs: Vec::new(),
            allow_symbolic_links: false,
            max_path_length: 4096,
        }
    }
}

impl PathValidationConfig {
    /// 创建新配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加允许的基础目录
    pub fn add_allowed_base_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.allowed_base_dirs.push(dir.into());
        self
    }

    /// 允许符号链接
    pub fn allow_symbolic_links(mut self, allowed: bool) -> Self {
        self.allow_symbolic_links = allowed;
        self
    }

    /// 设置最大路径长度
    pub fn with_max_path_length(mut self, length: usize) -> Self {
        self.max_path_length = length;
        self
    }

    /// 验证路径安全性
    pub fn validate(&self, path: &str) -> Result<PathBuf> {
        if path.len() > self.max_path_length {
            return Err(CacheError::InvalidInput(format!(
                "Path exceeds maximum length of {} characters",
                self.max_path_length
            )));
        }

        let path = Path::new(path);

        if !path.is_absolute() {
            return Err(CacheError::InvalidInput("Only absolute paths are allowed".to_string()));
        }

        let normalized = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                let mut buf = PathBuf::new();
                for component in path.components() {
                    match component {
                        std::path::Component::Normal(part) => buf.push(part),
                        std::path::Component::CurDir => {}
                        std::path::Component::ParentDir if !buf.pop() => {
                            return Err(CacheError::InvalidInput("Path traversal attempt detected".to_string()));
                        }
                        _ => {}
                    }
                }
                buf
            }
        };

        if !self.allowed_base_dirs.is_empty() {
            let mut within_allowed = false;
            for base_dir in &self.allowed_base_dirs {
                if let Ok(base_canonical) = base_dir.canonicalize() {
                    if normalized.starts_with(&base_canonical) {
                        within_allowed = true;
                        break;
                    }
                }
            }
            if !within_allowed {
                return Err(CacheError::InvalidInput(format!(
                    "Path is not within allowed directories: {}",
                    normalized.display()
                )));
            }
        }

        if !self.allow_symbolic_links {
            if let Some(file_name) = normalized.file_name() {
                if file_name.to_string_lossy().starts_with('.') {}
            }
        }

        validate_path_chars(path)?;

        Ok(normalized)
    }
}

fn validate_path_chars(path: &Path) -> Result<()> {
    let invalid_chars = ['\0', '\n', '\r', '\t'];
    let path_str = path.to_string_lossy();

    for ch in invalid_chars {
        if path_str.contains(ch) {
            return Err(CacheError::InvalidInput(format!(
                "Path contains invalid character: {:?}",
                ch
            )));
        }
    }

    Ok(())
}
