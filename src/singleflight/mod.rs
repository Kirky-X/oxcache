//! Singleflight 模块 - 请求去重机制

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::sync::broadcast;

use crate::error::{CacheError, Result};

type SharedResult = Arc<Result<Vec<u8>>>;

pub struct SingleFlight {
    inflight: Arc<Mutex<HashMap<String, broadcast::Sender<SharedResult>>>>,
}

impl SingleFlight {
    pub fn new() -> Self {
        Self {
            inflight: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn call<F, Fut>(&self, key: &str, work: F) -> Result<Vec<u8>>
    where
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<Vec<u8>>> + Send,
    {
        let (tx_option, mut rx) = {
            let mut map = self.inflight.lock().await;

            if let Some(sender) = map.get(key) {
                (None, sender.subscribe())
            } else {
                let (tx, rx) = broadcast::channel(1);
                map.insert(key.to_string(), tx.clone());
                (Some(tx), rx)
            }
        };

        match tx_option {
            Some(tx) => {
                let result = work().await;
                let shared = Arc::new(result.clone());
                let _ = tx.send(shared);

                {
                    let mut map = self.inflight.lock().await;
                    map.remove(key);
                }

                result
            }
            None => {
                match rx.recv().await {
                    Ok(shared) => (*shared).clone(),
                    Err(_) => Err(CacheError::Internal("SingleFlight: sender dropped".into())),
                }
            }
        }
    }

    pub async fn active_calls(&self) -> usize {
        self.inflight.lock().await.len()
    }

    pub async fn reset(&self) {
        self.inflight.lock().await.clear();
    }
}

impl Default for SingleFlight {
    fn default() -> Self { Self::new() }
}

impl fmt::Debug for SingleFlight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SingleFlight")
            .field("active_calls", &self.inflight.try_lock().map(|m| m.len()).unwrap_or(0))
            .finish()
    }
}

#[cfg(not(feature = "singleflight"))]
pub struct SingleFlight { _phantom: std::marker::PhantomData<()> }

#[cfg(not(feature = "singleflight"))]
impl SingleFlight {
    pub fn new() -> Self { Self { _phantom: std::marker::PhantomData } }
    pub async fn call<F, Fut>(&self, _key: &str, work: F) -> Result<Vec<u8>>
    where F: FnOnce() -> Fut + Send, Fut: std::future::Future<Output = Result<Vec<u8>>> + Send {
        work().await
    }
    pub async fn active_calls(&self) -> usize { 0 }
    pub async fn reset(&self) {}
}

#[cfg(not(feature = "singleflight"))]
impl Default for SingleFlight { fn default() -> Self { Self::new() } }

#[cfg(not(feature = "singleflight"))]
impl fmt::Debug for SingleFlight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.debug_struct("SingleFlight").finish() }
}
