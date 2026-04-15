use crate::error::{AgentError, Result};
use crate::session::Session;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn load_sessions(&self) -> Result<Vec<Session>>;
    async fn save_sessions(&self, sessions: Vec<Session>) -> Result<()>;
}

#[derive(Clone)]
pub struct FileSessionStore {
    path: PathBuf,
    backup_count: usize,
}

impl FileSessionStore {
    pub fn new(path: impl Into<PathBuf>, backup_count: usize) -> Self {
        Self {
            path: path.into(),
            backup_count,
        }
    }

    fn backup_path(path: &std::path::Path, index: usize) -> std::path::PathBuf {
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "sessions".to_string());
        path.with_file_name(format!("{}.bak.{}", file_name, index))
    }

    fn save_sync(path: PathBuf, backup_count: usize, sessions: Vec<Session>) -> Result<()> {
        let content = serde_json::to_string_pretty(&sessions)?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        if backup_count > 0 && path.exists() {
            for index in (1..=backup_count).rev() {
                let from = if index == 1 {
                    path.clone()
                } else {
                    Self::backup_path(&path, index - 1)
                };
                let to = Self::backup_path(&path, index);
                if from.exists() {
                    if to.exists() {
                        std::fs::remove_file(&to)?;
                    }
                    std::fs::rename(&from, &to)?;
                }
            }
        }

        let temp_path = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&temp_path, content)?;
        std::fs::rename(&temp_path, &path)?;
        Ok(())
    }
}

#[async_trait]
impl SessionStore for FileSessionStore {
    async fn load_sessions(&self) -> Result<Vec<Session>> {
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || {
            if !path.exists() {
                return Ok(Vec::new());
            }
            let content = std::fs::read_to_string(path)?;
            let sessions = serde_json::from_str::<Vec<Session>>(&content)?;
            Ok(sessions)
        })
        .await
        .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?
    }

    async fn save_sessions(&self, sessions: Vec<Session>) -> Result<()> {
        let path = self.path.clone();
        let backup_count = self.backup_count;
        tokio::task::spawn_blocking(move || Self::save_sync(path, backup_count, sessions))
            .await
            .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?
    }
}

#[derive(Clone, Default)]
pub struct MemorySessionStore {
    sessions: Arc<dashmap::DashMap<String, Session>>,
}

#[async_trait]
impl SessionStore for MemorySessionStore {
    async fn load_sessions(&self) -> Result<Vec<Session>> {
        Ok(self
            .sessions
            .iter()
            .map(|entry| entry.value().clone())
            .collect())
    }

    async fn save_sessions(&self, sessions: Vec<Session>) -> Result<()> {
        self.sessions.clear();
        for session in sessions {
            self.sessions.insert(session.id.clone(), session);
        }
        Ok(())
    }
}

#[cfg(feature = "redis-store")]
#[derive(Clone)]
pub struct RedisSessionStore {
    client: redis::Client,
    key: String,
}

#[cfg(feature = "redis-store")]
impl RedisSessionStore {
    pub fn new(redis_url: &str, key: impl Into<String>) -> Result<Self> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| AgentError::ConfigError(format!("invalid redis url: {}", e)))?;
        Ok(Self {
            client,
            key: key.into(),
        })
    }
}

#[cfg(feature = "redis-store")]
#[async_trait]
impl SessionStore for RedisSessionStore {
    async fn load_sessions(&self) -> Result<Vec<Session>> {
        let client = self.client.clone();
        let key = self.key.clone();
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?;
        let value: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?;
        match value {
            Some(raw) => Ok(serde_json::from_str::<Vec<Session>>(&raw)?),
            None => Ok(Vec::new()),
        }
    }

    async fn save_sessions(&self, sessions: Vec<Session>) -> Result<()> {
        let client = self.client.clone();
        let key = self.key.clone();
        let payload = serde_json::to_string(&sessions)?;
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?;
        redis::cmd("SET")
            .arg(&key)
            .arg(payload)
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| AgentError::IoError(std::io::Error::other(e.to_string())))?;
        Ok(())
    }
}
