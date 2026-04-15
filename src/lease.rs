use crate::error::{AgentError, Result};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[async_trait]
pub trait LeaseStore: Send + Sync {
    async fn try_acquire(&self, key: &str, owner: &str, ttl_seconds: u64) -> Result<bool>;
    async fn renew(&self, key: &str, owner: &str, ttl_seconds: u64) -> Result<bool>;
    async fn release(&self, key: &str, owner: &str) -> Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LeaseRecord {
    owner: String,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct FileLeaseStore {
    base_dir: PathBuf,
}

impl FileLeaseStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    pub fn from_state_file(path: &str) -> Self {
        let path = PathBuf::from(path);
        let base_dir = path
            .parent()
            .map(|parent| parent.join(".agentim-leases"))
            .unwrap_or_else(|| PathBuf::from(".agentim-leases"));
        Self { base_dir }
    }

    fn lease_path(&self, key: &str) -> PathBuf {
        self.base_dir.join(format!("{}.lease.json", key))
    }

    fn write_record(path: &PathBuf, record: &LeaseRecord) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, serde_json::to_string_pretty(record)?)?;
        Ok(())
    }

    fn read_record(path: &PathBuf) -> Result<Option<LeaseRecord>> {
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(path)?;
        let record = serde_json::from_str::<LeaseRecord>(&content)?;
        Ok(Some(record))
    }
}

#[async_trait]
impl LeaseStore for FileLeaseStore {
    async fn try_acquire(&self, key: &str, owner: &str, ttl_seconds: u64) -> Result<bool> {
        let path = self.lease_path(key);
        let owner = owner.to_string();
        tokio::task::spawn_blocking(move || {
            let now = Utc::now();
            let expires_at = now + Duration::seconds(ttl_seconds as i64);
            match Self::read_record(&path)? {
                None => {
                    Self::write_record(&path, &LeaseRecord { owner, expires_at })?;
                    Ok(true)
                }
                Some(record) if record.owner == owner || record.expires_at <= now => {
                    Self::write_record(&path, &LeaseRecord { owner, expires_at })?;
                    Ok(true)
                }
                Some(_) => Ok(false),
            }
        })
        .await
        .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?
    }

    async fn renew(&self, key: &str, owner: &str, ttl_seconds: u64) -> Result<bool> {
        let path = self.lease_path(key);
        let owner = owner.to_string();
        tokio::task::spawn_blocking(move || {
            let now = Utc::now();
            let expires_at = now + Duration::seconds(ttl_seconds as i64);
            match Self::read_record(&path)? {
                Some(record) if record.owner == owner => {
                    Self::write_record(&path, &LeaseRecord { owner, expires_at })?;
                    Ok(true)
                }
                Some(record) if record.expires_at <= now => {
                    Self::write_record(&path, &LeaseRecord { owner, expires_at })?;
                    Ok(true)
                }
                Some(_) => Ok(false),
                None => Ok(false),
            }
        })
        .await
        .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?
    }

    async fn release(&self, key: &str, owner: &str) -> Result<()> {
        let path = self.lease_path(key);
        let owner = owner.to_string();
        tokio::task::spawn_blocking(move || {
            if let Some(record) = Self::read_record(&path)? {
                if record.owner == owner && path.exists() {
                    std::fs::remove_file(path)?;
                }
            }
            Ok(())
        })
        .await
        .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?
    }
}

#[cfg(feature = "redis-store")]
#[derive(Clone)]
pub struct RedisLeaseStore {
    client: redis::Client,
    key_prefix: String,
}

#[cfg(feature = "redis-store")]
impl RedisLeaseStore {
    pub fn new(redis_url: &str, key_prefix: impl Into<String>) -> Result<Self> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| AgentError::ConfigError(format!("invalid redis url: {}", e)))?;
        Ok(Self {
            client,
            key_prefix: key_prefix.into(),
        })
    }

    fn namespaced_key(&self, key: &str) -> String {
        format!("{}:{}", self.key_prefix, key)
    }
}

#[cfg(feature = "redis-store")]
#[async_trait]
impl LeaseStore for RedisLeaseStore {
    async fn try_acquire(&self, key: &str, owner: &str, ttl_seconds: u64) -> Result<bool> {
        let key = self.namespaced_key(key);
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?;
        let result: Option<String> = redis::cmd("SET")
            .arg(&key)
            .arg(owner)
            .arg("NX")
            .arg("EX")
            .arg(ttl_seconds)
            .query_async(&mut conn)
            .await
            .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?;
        if result.is_some() {
            return Ok(true);
        }

        let current: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?;
        if current.as_deref() == Some(owner) {
            let _: () = redis::cmd("EXPIRE")
                .arg(&key)
                .arg(ttl_seconds)
                .query_async(&mut conn)
                .await
                .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?;
            return Ok(true);
        }
        Ok(false)
    }

    async fn renew(&self, key: &str, owner: &str, ttl_seconds: u64) -> Result<bool> {
        let key = self.namespaced_key(key);
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?;
        let current: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?;
        if current.as_deref() != Some(owner) {
            return Ok(false);
        }
        let _: () = redis::cmd("EXPIRE")
            .arg(&key)
            .arg(ttl_seconds)
            .query_async(&mut conn)
            .await
            .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?;
        Ok(true)
    }

    async fn release(&self, key: &str, owner: &str) -> Result<()> {
        let key = self.namespaced_key(key);
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?;
        let current: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?;
        if current.as_deref() == Some(owner) {
            let _: () = redis::cmd("DEL")
                .arg(&key)
                .query_async(&mut conn)
                .await
                .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?;
        }
        Ok(())
    }
}
