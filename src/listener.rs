use crate::error::Result;
use crate::lease::LeaseStore;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListenerCheckpoint {
    pub cursor: Option<String>,
}

#[derive(Clone)]
pub struct ListenerRuntimeConfig {
    pub max_backoff_ms: u64,
    pub lease_store: Option<Arc<dyn LeaseStore>>,
    pub lease_key: Option<String>,
    pub lease_owner: String,
    pub lease_ttl_seconds: u64,
}

impl Default for ListenerRuntimeConfig {
    fn default() -> Self {
        Self {
            max_backoff_ms: 30_000,
            lease_store: None,
            lease_key: None,
            lease_owner: format!("agentim-{}", uuid::Uuid::new_v4()),
            lease_ttl_seconds: 60,
        }
    }
}

#[async_trait]
pub trait InboundListener: Send + Sync {
    fn id(&self) -> &str;
    async fn initialize(&self) -> Result<()>;
    async fn load_checkpoint(&self) -> Result<ListenerCheckpoint>;
    async fn run_once(&self, checkpoint: &mut ListenerCheckpoint) -> Result<bool>;
    async fn persist_checkpoint(&self, checkpoint: &ListenerCheckpoint) -> Result<()>;
}

pub async fn run_listener_supervisor(
    listener: Arc<dyn InboundListener>,
    runtime: ListenerRuntimeConfig,
) -> Result<()> {
    listener.initialize().await?;
    let mut checkpoint = listener.load_checkpoint().await?;
    let mut backoff_ms = 1000u64;

    loop {
        if let (Some(store), Some(key)) = (&runtime.lease_store, &runtime.lease_key) {
            let acquired = store
                .try_acquire(key, &runtime.lease_owner, runtime.lease_ttl_seconds)
                .await?;
            if !acquired {
                tracing::debug!(listener = %listener.id(), lease_key = %key, "listener lease not acquired; sleeping");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
        }

        match listener.run_once(&mut checkpoint).await {
            Ok(processed_any) => {
                listener.persist_checkpoint(&checkpoint).await?;
                if let (Some(store), Some(key)) = (&runtime.lease_store, &runtime.lease_key) {
                    let _ = store
                        .renew(key, &runtime.lease_owner, runtime.lease_ttl_seconds)
                        .await?;
                }
                if processed_any {
                    backoff_ms = 1000;
                }
            }
            Err(err) => {
                tracing::warn!(listener = %listener.id(), error = %err, backoff_ms, "listener cycle failed; retrying");
                tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(runtime.max_backoff_ms.max(1000));
                listener.initialize().await?;
            }
        }
    }
}

pub fn default_checkpoint_path(state_file: Option<&str>, listener_id: &str) -> Option<PathBuf> {
    state_file.map(|path| PathBuf::from(format!("{}.{}.checkpoint.json", path, listener_id)))
}

pub async fn load_checkpoint_from_path<T>(path: Option<PathBuf>) -> Result<T>
where
    T: DeserializeOwned + Default + Send + 'static,
{
    let Some(path) = path else {
        return Ok(T::default());
    };

    tokio::task::spawn_blocking(move || {
        if !path.exists() {
            return Ok(T::default());
        }
        let content = std::fs::read_to_string(&path)?;
        match serde_json::from_str::<T>(&content) {
            Ok(parsed) => Ok(parsed),
            Err(err) => {
                let corrupt_path =
                    path.with_extension(format!("corrupt.{}.json", chrono::Utc::now().timestamp()));
                let _ = std::fs::rename(&path, &corrupt_path);
                tracing::warn!(
                    path = %path.display(),
                    corrupt_path = %corrupt_path.display(),
                    error = %err,
                    "listener checkpoint corrupted; falling back to default checkpoint"
                );
                Ok(T::default())
            }
        }
    })
    .await
    .map_err(|e| crate::error::AgentError::IoError(std::io::Error::other(e.to_string())))?
}

pub async fn save_checkpoint_to_path<T>(path: Option<PathBuf>, checkpoint: &T) -> Result<()>
where
    T: Serialize + Send + Sync + Clone + 'static,
{
    let Some(path) = path else {
        return Ok(());
    };
    let checkpoint = checkpoint.clone();
    tokio::task::spawn_blocking(move || {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(&path, serde_json::to_string_pretty(&checkpoint)?)?;
        Ok(())
    })
    .await
    .map_err(|e| crate::error::AgentError::IoError(std::io::Error::other(e.to_string())))?
}
