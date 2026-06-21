use crate::error::{CacheError, Result};
use std::fmt;

#[cfg(feature = "singleflight")]
use std::collections::HashMap;
#[cfg(feature = "singleflight")]
use std::sync::Arc;
#[cfg(feature = "singleflight")]
use tokio::sync::broadcast;
#[cfg(feature = "singleflight")]
use tokio::sync::Mutex;

#[cfg(feature = "singleflight")]
type InflightMap = Arc<Mutex<HashMap<String, broadcast::Sender<Arc<Vec<u8>>>>>>;

#[cfg(feature = "singleflight")]
pub struct SingleFlight {
    inflight: InflightMap,
}

#[cfg(feature = "singleflight")]
#[allow(dead_code)]
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
        let rx = {
            let mut map = self.inflight.lock().await;
            if let Some(tx) = map.get(key) {
                Some(tx.subscribe())
            } else {
                let (tx, _) = broadcast::channel(1);
                map.insert(key.to_string(), tx);
                None
            }
        };

        match rx {
            None => {
                let result = work().await;
                {
                    let mut map = self.inflight.lock().await;
                    if let Some(tx) = map.remove(key) {
                        if let Ok(ref val) = result {
                            let _ = tx.send(Arc::new(val.clone()));
                        }
                    }
                }
                result
            }
            Some(mut rx) => match rx.recv().await {
                Ok(shared) => Ok((*shared).clone()),
                Err(_) => Err(CacheError::Internal("SingleFlight: sender dropped".into())),
            },
        }
    }

    pub async fn active_calls(&self) -> usize {
        self.inflight.lock().await.len()
    }

    pub async fn reset(&self) {
        self.inflight.lock().await.clear();
    }
}

#[cfg(feature = "singleflight")]
impl Default for SingleFlight {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "singleflight")]
impl fmt::Debug for SingleFlight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SingleFlight")
            .field("active_calls", &self.inflight.try_lock().map(|m| m.len()).unwrap_or(0))
            .finish()
    }
}

#[cfg(not(feature = "singleflight"))]
pub struct SingleFlight {
    _phantom: std::marker::PhantomData<()>,
}

#[cfg(not(feature = "singleflight"))]
impl SingleFlight {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
    pub async fn call<F, Fut>(&self, _key: &str, work: F) -> Result<Vec<u8>>
    where
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<Vec<u8>>> + Send,
    {
        work().await
    }
    pub async fn active_calls(&self) -> usize {
        0
    }
    pub async fn reset(&self) {}
}

#[cfg(not(feature = "singleflight"))]
impl Default for SingleFlight {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "singleflight"))]
impl fmt::Debug for SingleFlight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SingleFlight").finish()
    }
}
