use crate::config::{CacheWarmupConfig, WarmupDataSource};
use crate::error::Result;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

pub struct WarmupManager {
    service_name: String,
    config: CacheWarmupConfig,
    warmup_status: Arc<RwLock<HashMap<String, WarmupStatus>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WarmupStatus {
    Pending,
    InProgress { progress: usize, total: usize },
    Completed { loaded: usize, failed: usize },
    Failed { error: String },
}

pub struct WarmupResult {
    pub loaded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub success: bool,
}

impl WarmupResult {
    pub fn skipped() -> Self {
        Self {
            loaded: 0,
            failed: 0,
            skipped: 1,
            success: true,
        }
    }

    pub fn failed(_error: String) -> Self {
        Self {
            loaded: 0,
            failed: 0,
            skipped: 0,
            success: false,
        }
    }
}

impl WarmupManager {
    pub fn new(service_name: String, config: CacheWarmupConfig) -> Self {
        Self {
            service_name,
            config,
            warmup_status: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn run_warmup<F, Fut>(&self, load_fn: F) -> Result<WarmupResult>
    where
        F: Fn(Vec<String>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<HashMap<String, Vec<u8>>>> + Send,
    {
        info!(
            "Starting cache warmup for service: {}, enabled: {}",
            self.service_name, self.config.enabled
        );

        if !self.config.enabled {
            info!("Cache warmup is disabled, skipping");
            return Ok(WarmupResult::skipped());
        }

        let timeout = tokio::time::Duration::from_secs(self.config.timeout_seconds);
        let result = tokio::time::timeout(timeout, self.warmup_inner(load_fn)).await;

        match result {
            Ok(Ok(result)) => {
                info!(
                    "Cache warmup completed: loaded={}, failed={}, skipped={}",
                    result.loaded, result.failed, result.skipped
                );
                Ok(result)
            }
            Ok(Err(e)) => {
                warn!("Cache warmup failed: {}", e);
                Ok(WarmupResult::failed(e.to_string()))
            }
            Err(_) => {
                warn!(
                    "Cache warmup timed out after {} seconds",
                    self.config.timeout_seconds
                );
                Ok(WarmupResult::failed("timeout".to_string()))
            }
        }
    }

    async fn warmup_inner<F, Fut>(&self, load_fn: F) -> Result<WarmupResult>
    where
        F: Fn(Vec<String>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<HashMap<String, Vec<u8>>>> + Send,
    {
        let mut total_loaded = 0usize;
        let mut total_failed = 0usize;
        let mut total_skipped = 0usize;

        for source in &self.config.data_sources {
            info!("Loading keys from source: {:?}", source);

            let keys: Vec<String> = match source {
                WarmupDataSource::Static { keys } => keys.clone(),
                WarmupDataSource::RedisList { .. } => {
                    warn!("RedisList warmup source requires custom implementation");
                    total_skipped = total_skipped.saturating_add(1);
                    continue;
                }
                WarmupDataSource::Database { .. } => {
                    warn!("Database warmup source requires custom implementation");
                    total_skipped = total_skipped.saturating_add(1);
                    continue;
                }
                WarmupDataSource::Api { .. } => {
                    warn!("API warmup source requires custom implementation");
                    total_skipped = total_skipped.saturating_add(1);
                    continue;
                }
            };

            let keys_count = keys.len();
            debug!("Loaded {} keys from source", keys_count);

            let batch_size = self.config.batch_size;
            let interval_ms = self.config.batch_interval_ms;

            for chunk in keys.chunks(batch_size) {
                let chunk_keys: Vec<String> = chunk.to_vec();

                match load_fn(chunk_keys.clone()).await {
                    Ok(data_map) => {
                        let loaded = data_map.len();
                        let failed = chunk_keys.len().saturating_sub(loaded);
                        total_loaded = total_loaded.saturating_add(loaded);
                        total_failed = total_failed.saturating_add(failed);
                    }
                    Err(e) => {
                        warn!("Failed to load data batch: {}", e);
                        total_failed = total_failed.saturating_add(chunk_keys.len());
                    }
                }

                if interval_ms > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)).await;
                }
            }
        }

        Ok(WarmupResult {
            loaded: total_loaded,
            failed: total_failed,
            skipped: total_skipped,
            success: total_failed == 0,
        })
    }

    pub async fn get_status(&self, source_type: &str) -> WarmupStatus {
        let status_map = self.warmup_status.read().await;
        status_map
            .get(source_type)
            .cloned()
            .unwrap_or(WarmupStatus::Pending)
    }
}
