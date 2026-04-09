use crate::agent::Agent;
use crate::channel::Channel;
use crate::error::{AgentError, Result};
use crate::session::Session;
use dashmap::DashMap;
use std::sync::Arc;

pub struct AgentIM {
    agents: Arc<DashMap<String, Arc<dyn Agent>>>,
    channels: Arc<DashMap<String, Arc<dyn Channel>>>,
    sessions: Arc<DashMap<String, Session>>,
}

#[derive(Clone, Copy, Debug)]
pub struct MessageHandlingOptions {
    pub max_messages: Option<usize>,
    pub context_message_limit: usize,
    pub agent_timeout_ms: Option<u64>,
}

impl Default for MessageHandlingOptions {
    fn default() -> Self {
        Self {
            max_messages: None,
            context_message_limit: 10,
            agent_timeout_ms: None,
        }
    }
}

impl AgentIM {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(DashMap::new()),
            channels: Arc::new(DashMap::new()),
            sessions: Arc::new(DashMap::new()),
        }
    }

    pub fn register_agent(&self, id: String, agent: Arc<dyn Agent>) -> Result<()> {
        self.agents.insert(id, agent);
        Ok(())
    }

    pub fn register_channel(&self, id: String, channel: Arc<dyn Channel>) -> Result<()> {
        self.channels.insert(id, channel);
        Ok(())
    }

    pub fn get_agent(&self, id: &str) -> Result<Arc<dyn Agent>> {
        self.agents
            .get(id)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| AgentError::AgentNotFound(id.to_string()))
    }

    pub fn get_channel(&self, id: &str) -> Result<Arc<dyn Channel>> {
        self.channels
            .get(id)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| AgentError::ChannelNotFound(id.to_string()))
    }

    pub fn create_session(
        &self,
        agent_id: String,
        channel_id: String,
        user_id: String,
    ) -> Result<String> {
        // Verify agent and channel exist
        self.get_agent(&agent_id)?;
        self.get_channel(&channel_id)?;

        let session = Session::new(agent_id, channel_id, user_id);
        let session_id = session.id.clone();
        self.sessions.insert(session_id.clone(), session);
        Ok(session_id)
    }

    pub fn get_session(&self, id: &str) -> Result<Session> {
        self.sessions
            .get(id)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| AgentError::SessionNotFound(id.to_string()))
    }

    pub fn update_session(&self, id: &str, session: Session) -> Result<()> {
        self.sessions.insert(id.to_string(), session);
        Ok(())
    }

    pub fn list_sessions(&self) -> Vec<Session> {
        self.sessions
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub fn list_agents(&self) -> Vec<String> {
        self.agents
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    pub fn list_channels(&self) -> Vec<String> {
        self.channels
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    pub fn save_sessions_to_path(&self, path: &str) -> Result<()> {
        self.save_sessions_to_path_with_rotation(path, 0)
    }

    fn backup_path(path: &std::path::Path, index: usize) -> std::path::PathBuf {
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "sessions".to_string());
        path.with_file_name(format!("{}.bak.{}", file_name, index))
    }

    pub fn save_sessions_to_path_with_rotation(
        &self,
        path: &str,
        backup_count: usize,
    ) -> Result<()> {
        let sessions = self.list_sessions();
        let content = serde_json::to_string_pretty(&sessions)?;
        let path = std::path::Path::new(path);

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        if backup_count > 0 && path.exists() {
            for index in (1..=backup_count).rev() {
                let from = if index == 1 {
                    path.to_path_buf()
                } else {
                    Self::backup_path(path, index - 1)
                };
                let to = Self::backup_path(path, index);

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
        std::fs::rename(&temp_path, path)?;
        Ok(())
    }

    fn load_sessions_from_specific_path(&self, path: &std::path::Path) -> Result<usize> {
        let content = std::fs::read_to_string(path)?;
        let sessions: Vec<Session> = serde_json::from_str(&content)?;
        let count = sessions.len();

        for session in &sessions {
            self.get_agent(&session.agent_id)?;
            self.get_channel(&session.channel_id)?;
        }

        for session in sessions {
            self.sessions.insert(session.id.clone(), session);
        }

        Ok(count)
    }

    pub fn load_sessions_from_path(&self, path: &str) -> Result<usize> {
        let path = std::path::Path::new(path);
        if !path.exists() {
            return Ok(0);
        }

        self.load_sessions_from_specific_path(path)
    }

    pub fn load_sessions_from_path_with_fallback(
        &self,
        path: &str,
        backup_count: usize,
    ) -> Result<(usize, String)> {
        let path = std::path::Path::new(path);
        let mut candidates = vec![path.to_path_buf()];
        for index in 1..=backup_count {
            candidates.push(Self::backup_path(path, index));
        }

        let existing = candidates
            .into_iter()
            .filter(|candidate| candidate.exists())
            .collect::<Vec<_>>();
        if existing.is_empty() {
            return Ok((0, path.display().to_string()));
        }

        let mut last_error = None;
        for candidate in existing {
            match self.load_sessions_from_specific_path(&candidate) {
                Ok(count) => return Ok((count, candidate.display().to_string())),
                Err(err) => last_error = Some(err),
            }
        }

        Err(last_error
            .unwrap_or_else(|| AgentError::Unknown("No valid session snapshot found".to_string())))
    }

    pub async fn send_to_agent(&self, session_id: &str, user_message: String) -> Result<String> {
        self.send_to_agent_with_context_limit(session_id, user_message, 10)
            .await
    }

    pub async fn send_to_agent_with_context_limit(
        &self,
        session_id: &str,
        user_message: String,
        context_message_limit: usize,
    ) -> Result<String> {
        self.send_to_agent_with_context_limit_and_timeout(
            session_id,
            user_message,
            context_message_limit,
            None,
        )
        .await
    }

    pub async fn send_to_agent_with_context_limit_and_timeout(
        &self,
        session_id: &str,
        user_message: String,
        context_message_limit: usize,
        agent_timeout_ms: Option<u64>,
    ) -> Result<String> {
        let mut session = self.get_session(session_id)?;
        let agent = self.get_agent(&session.agent_id)?;

        // Add user message to session
        session.add_message(crate::session::MessageRole::User, user_message);

        // Get context for agent
        let context = session.get_context(context_message_limit);

        // Send to agent
        let response = if let Some(timeout_ms) = agent_timeout_ms {
            tokio::time::timeout(
                std::time::Duration::from_millis(timeout_ms),
                agent.send_message(context),
            )
            .await
            .map_err(|_| {
                AgentError::TimeoutError(format!("agent request exceeded {}ms", timeout_ms))
            })??
        } else {
            agent.send_message(context).await?
        };

        // Add agent response to session
        session.add_message(crate::session::MessageRole::Assistant, response.clone());

        // Update session
        self.update_session(session_id, session)?;

        Ok(response)
    }

    pub async fn send_to_channel(&self, session_id: &str, message: String) -> Result<()> {
        let session = self.get_session(session_id)?;
        let channel = self.get_channel(&session.channel_id)?;
        let reply_target = session
            .metadata
            .get("reply_target")
            .cloned()
            .unwrap_or_else(|| session.user_id.clone());

        channel.send_message(&reply_target, &message).await?;
        Ok(())
    }

    pub fn trim_session_history(&self, session_id: &str, max_messages: usize) -> Result<()> {
        let mut session = self.get_session(session_id)?;
        session.trim_history(max_messages);
        self.update_session(session_id, session)
    }

    pub async fn handle_incoming_message(
        &self,
        agent_id: &str,
        channel_id: &str,
        user_id: &str,
        reply_target: Option<&str>,
        user_message: String,
    ) -> Result<String> {
        self.handle_incoming_message_with_limit(
            agent_id,
            channel_id,
            user_id,
            reply_target,
            user_message,
            None,
        )
        .await
    }

    pub async fn handle_incoming_message_with_limit(
        &self,
        agent_id: &str,
        channel_id: &str,
        user_id: &str,
        reply_target: Option<&str>,
        user_message: String,
        max_messages: Option<usize>,
    ) -> Result<String> {
        self.handle_incoming_message_with_limits(
            agent_id,
            channel_id,
            user_id,
            reply_target,
            user_message,
            max_messages,
            10,
        )
        .await
    }

    pub async fn handle_incoming_message_with_options(
        &self,
        agent_id: &str,
        channel_id: &str,
        user_id: &str,
        reply_target: Option<&str>,
        user_message: String,
        options: MessageHandlingOptions,
    ) -> Result<String> {
        self.handle_incoming_message_with_runtime_limits(
            agent_id,
            channel_id,
            user_id,
            reply_target,
            user_message,
            options.max_messages,
            options.context_message_limit,
            options.agent_timeout_ms,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn handle_incoming_message_with_limits(
        &self,
        agent_id: &str,
        channel_id: &str,
        user_id: &str,
        reply_target: Option<&str>,
        user_message: String,
        max_messages: Option<usize>,
        context_message_limit: usize,
    ) -> Result<String> {
        self.handle_incoming_message_with_runtime_limits(
            agent_id,
            channel_id,
            user_id,
            reply_target,
            user_message,
            max_messages,
            context_message_limit,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn handle_incoming_message_with_runtime_limits(
        &self,
        agent_id: &str,
        channel_id: &str,
        user_id: &str,
        reply_target: Option<&str>,
        user_message: String,
        max_messages: Option<usize>,
        context_message_limit: usize,
        agent_timeout_ms: Option<u64>,
    ) -> Result<String> {
        let session_id = self.find_or_create_session(agent_id, channel_id, user_id)?;

        if let Some(reply_target) = reply_target {
            let mut session = self.get_session(&session_id)?;
            session
                .metadata
                .insert("reply_target".to_string(), reply_target.to_string());
            self.update_session(&session_id, session)?;
        }

        let response = self
            .send_to_agent_with_context_limit_and_timeout(
                &session_id,
                user_message,
                context_message_limit,
                agent_timeout_ms,
            )
            .await?;

        if let Some(max_messages) = max_messages {
            self.trim_session_history(&session_id, max_messages)?;
        }

        self.send_to_channel(&session_id, response.clone()).await?;
        Ok(response)
    }

    pub async fn health_check(&self) -> Result<()> {
        for agent_ref in self.agents.iter() {
            agent_ref.value().health_check().await?;
        }

        for channel_ref in self.channels.iter() {
            channel_ref.value().health_check().await?;
        }

        Ok(())
    }

    pub fn find_or_create_session(
        &self,
        agent_id: &str,
        channel_id: &str,
        user_id: &str,
    ) -> Result<String> {
        // Look for existing session
        for session_ref in self.sessions.iter() {
            let session = session_ref.value();
            if session.agent_id == agent_id
                && session.channel_id == channel_id
                && session.user_id == user_id
            {
                return Ok(session.id.clone());
            }
        }

        // Create new session if not found
        self.create_session(
            agent_id.to_string(),
            channel_id.to_string(),
            user_id.to_string(),
        )
    }

    pub fn delete_session(&self, id: &str) -> Result<()> {
        self.sessions
            .remove(id)
            .ok_or_else(|| AgentError::SessionNotFound(id.to_string()))?;
        Ok(())
    }

    /// Remove sessions that have not been updated for `max_idle_seconds`.
    /// Returns the number of sessions removed.
    pub fn cleanup_stale_sessions(&self, max_idle_seconds: u64) -> usize {
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(max_idle_seconds as i64);
        let stale_ids: Vec<String> = self
            .sessions
            .iter()
            .filter(|entry| entry.value().updated_at < cutoff)
            .map(|entry| entry.key().clone())
            .collect();
        let count = stale_ids.len();
        for id in stale_ids {
            self.sessions.remove(&id);
        }
        count
    }
}

impl Default for AgentIM {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AgentIM {
    fn clone(&self) -> Self {
        Self {
            agents: self.agents.clone(),
            channels: self.channels.clone(),
            sessions: self.sessions.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::channel::{Channel, ChannelMessage};
    use crate::config::{AgentType, ChannelType};
    use crate::session::Message;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    struct MockAgent {
        mock_id: String,
    }

    impl MockAgent {
        fn new(id: &str) -> Self {
            Self {
                mock_id: id.to_string(),
            }
        }
    }

    #[async_trait]
    impl Agent for MockAgent {
        fn agent_type(&self) -> AgentType {
            AgentType::OpenAI
        }

        fn id(&self) -> &str {
            &self.mock_id
        }

        async fn send_message(&self, messages: Vec<Message>) -> Result<String> {
            let last = messages
                .last()
                .map(|msg| msg.content.clone())
                .unwrap_or_default();
            Ok(format!("echo:{}", last))
        }

        async fn health_check(&self) -> Result<()> {
            Ok(())
        }
    }

    struct MockChannel {
        mock_id: String,
        sent_messages: Arc<Mutex<Vec<(String, String)>>>,
    }

    impl MockChannel {
        fn new(id: &str, sent_messages: Arc<Mutex<Vec<(String, String)>>>) -> Self {
            Self {
                mock_id: id.to_string(),
                sent_messages,
            }
        }
    }

    #[async_trait]
    impl Channel for MockChannel {
        fn channel_type(&self) -> ChannelType {
            ChannelType::Discord
        }

        fn id(&self) -> &str {
            &self.mock_id
        }

        async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
            self.sent_messages
                .lock()
                .unwrap()
                .push((user_id.to_string(), content.to_string()));
            Ok(())
        }

        async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
            Ok(None)
        }

        async fn health_check(&self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_agentim_creation() {
        let agentim = AgentIM::new();
        assert_eq!(agentim.list_agents().len(), 0);
        assert_eq!(agentim.list_channels().len(), 0);
    }

    #[test]
    fn test_register_agent() {
        let agentim = AgentIM::new();
        let agent = Arc::new(MockAgent::new("agent1"));
        agentim.register_agent("agent1".to_string(), agent).unwrap();
        assert_eq!(agentim.list_agents().len(), 1);
    }

    #[test]
    fn test_create_session() {
        let agentim = AgentIM::new();
        let agent = Arc::new(MockAgent::new("agent1"));
        let channel = Arc::new(MockChannel::new("ch1", Arc::new(Mutex::new(Vec::new()))));

        agentim.register_agent("agent1".to_string(), agent).unwrap();
        agentim
            .register_channel("ch1".to_string(), channel)
            .unwrap();

        let session_id = agentim
            .create_session("agent1".to_string(), "ch1".to_string(), "user1".to_string())
            .unwrap();

        let session = agentim.get_session(&session_id).unwrap();
        assert_eq!(session.agent_id, "agent1");
        assert_eq!(session.channel_id, "ch1");
    }

    #[tokio::test]
    async fn test_handle_incoming_message_autocreates_session_and_uses_reply_target() {
        let agentim = AgentIM::new();
        let sent_messages = Arc::new(Mutex::new(Vec::new()));

        agentim
            .register_agent(
                "default-agent".to_string(),
                Arc::new(MockAgent::new("default-agent")),
            )
            .unwrap();
        agentim
            .register_channel(
                "discord-bot".to_string(),
                Arc::new(MockChannel::new("discord-bot", sent_messages.clone())),
            )
            .unwrap();

        let response = agentim
            .handle_incoming_message(
                "default-agent",
                "discord-bot",
                "user-1",
                Some("channel-42"),
                "ping".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(response, "echo:ping");

        let sessions = agentim.list_sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].user_id, "user-1");
        assert_eq!(
            sessions[0].metadata.get("reply_target"),
            Some(&"channel-42".to_string())
        );

        let sent = sent_messages.lock().unwrap();
        assert_eq!(
            sent.as_slice(),
            &[("channel-42".to_string(), "echo:ping".to_string())]
        );
    }
}
