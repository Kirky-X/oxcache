//! 模拟L2后端，用于测试降级策略而不依赖外部Redis

use oxcache::config::L2Config;
use oxcache::error::{CacheError, Result};

#[derive(Clone)]
pub struct FailingL2Backend {
    pub command_timeout_ms: u64,
}

impl FailingL2Backend {
    pub async fn ping(&self) -> Result<()> {
        Err(CacheError::BackendError("模拟连接失败".to_string()))
    }

    pub fn command_timeout_ms(&self) -> u64 {
        self.command_timeout_ms
    }
}

pub async fn create_failing_l2_backend(config: &L2Config) -> Result<FailingL2Backend> {
    Ok(FailingL2Backend {
        command_timeout_ms: config.command_timeout_ms,
    })
}
