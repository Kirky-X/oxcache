//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Configuration validation result types extracted from custom_tiered.rs

use crate::core::types::{BackendType, CacheLayer};

/// Type alias for Layer compatibility
pub type Layer = CacheLayer;

/// 配置验证结果
#[derive(Debug, Clone, Default)]
pub struct ConfigValidationResult {
    valid_layers: Vec<(Layer, BackendType)>,
    invalid_layers: Vec<(Layer, BackendType, String)>,
    fixes: Vec<ConfigFix>,
}

impl ConfigValidationResult {
    pub fn new() -> Self {
        Self {
            valid_layers: Vec::new(),
            invalid_layers: Vec::new(),
            fixes: Vec::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.invalid_layers.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.fixes.is_empty()
    }

    pub fn get_fixes(&self) -> &[ConfigFix] {
        &self.fixes
    }

    pub fn get_validation_report(&self) -> String {
        let mut report = String::new();

        if self.is_valid() {
            report.push_str("✅ Configuration is valid\n");
        } else {
            report.push_str("❌ Configuration has issues:\n");
            for (layer, backend, error) in &self.invalid_layers {
                report.push_str(&format!("  - Layer {}: {} - {}\n", layer, backend, error));
            }
        }

        if !self.fixes.is_empty() {
            report.push_str("\n🔧 Suggested fixes:\n");
            for fix in &self.fixes {
                report.push_str(&format!(
                    "  - {}: '{}' → '{}' (reason: {})\n",
                    fix.layer, fix.from_backend, fix.to_backend, fix.reason
                ));
            }
        }

        report
    }
}

/// 配置修复建议
#[derive(Debug, Clone)]
pub struct ConfigFix {
    pub layer: Layer,
    pub from_backend: BackendType,
    pub to_backend: BackendType,
    pub reason: String,
}

/// 固定配置结果
#[derive(Debug, Clone)]
pub struct FixedConfigResult {
    pub is_valid: bool,
    pub l1_backend: Option<BackendType>,
    pub l2_backend: Option<BackendType>,
    pub l3_backend: Option<BackendType>,
    pub warnings: Vec<String>,
}

impl From<ConfigValidationResult> for FixedConfigResult {
    fn from(val: ConfigValidationResult) -> Self {
        let mut warnings = Vec::new();

        for fix in &val.fixes {
            warnings.push(format!(
                "Auto-fixed {} from '{}' to '{}'",
                fix.layer, fix.from_backend, fix.to_backend
            ));
        }

        let l1_backend = val
            .valid_layers
            .iter()
            .find(|(l, _)| *l == Layer::L1)
            .map(|(_, b)| b.clone());
        let l2_backend = val
            .valid_layers
            .iter()
            .find(|(l, _)| *l == Layer::L2)
            .map(|(_, b)| b.clone());
        let l3_backend = val
            .valid_layers
            .iter()
            .find(|(l, _)| *l == Layer::L3)
            .map(|(_, b)| b.clone());

        Self {
            is_valid: val.is_valid(),
            l1_backend,
            l2_backend,
            l3_backend,
            warnings,
        }
    }
}

impl FixedConfigResult {
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    /// 获取配置报告
    pub fn get_report(&self) -> String {
        let mut report = String::new();

        if !self.is_valid {
            report.push_str("Invalid configuration:\n");
        }

        for warning in &self.warnings {
            report.push_str(&format!("  - {}\n", warning));
        }

        report
    }
}
