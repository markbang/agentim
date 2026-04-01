use crate::agent::Agent;
use crate::channel::Channel;
use crate::error::{AgentError, Result};
use crate::session::{MessageRole, Session};
use dashmap::{mapref::entry::Entry, DashMap};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;

struct SessionEntry {
    session: Mutex<Session>,
    processing: AsyncMutex<()>,
}

impl SessionEntry {
    fn new(session: Session) -> Self {
        Self {
            session: Mutex::new(session),
            processing: AsyncMutex::new(()),
        }
    }

    fn snapshot(&self) -> Result<Session> {
        Ok(self
            .session
            .lock()
            .map_err(|_| AgentError::Unknown("session mutex poisoned".to_string()))?
            .clone())
    }

    fn replace(&self, session: Session) -> Result<()> {
        *self
            .session
            .lock()
            .map_err(|_| AgentError::Unknown("session mutex poisoned".to_string()))? = session;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct SessionLookupKey {
    agent_id: String,
    channel_id: String,
    user_id: String,
}

impl SessionLookupKey {
    fn new(agent_id: &str, channel_id: &str, user_id: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            channel_id: channel_id.to_string(),
            user_id: user_id.to_string(),
        }
    }

    fn from_session(session: &Session) -> Self {
        Self::new(&session.agent_id, &session.channel_id, &session.user_id)
    }
}

struct SessionMessageResult {
    response: String,
    channel_id: String,
    reply_target: String,
}

pub struct AgentIM {
    agents: Arc<DashMap<String, Arc<dyn Agent>>>,
    channels: Arc<DashMap<String, Arc<dyn Channel>>>,
    sessions: Arc<DashMap<String, Arc<SessionEntry>>>,
    session_index: Arc<DashMap<SessionLookupKey, String>>,
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
            session_index: Arc::new(DashMap::new()),
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

    fn get_session_entry(&self, id: &str) -> Result<Arc<SessionEntry>> {
        self.sessions
            .get(id)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| AgentError::SessionNotFound(id.to_string()))
    }

    fn insert_session_entry(&self, session: Session) -> String {
        let session_id = session.id.clone();
        let lookup_key = SessionLookupKey::from_session(&session);

        if let Some(previous_id) = self.session_index.insert(lookup_key, session_id.clone()) {
            if previous_id != session_id {
                self.sessions.remove(&previous_id);
            }
        }

        self.sessions
            .insert(session_id.clone(), Arc::new(SessionEntry::new(session)));
        session_id
    }

    pub fn create_session(
        &self,
        agent_id: String,
        channel_id: String,
        user_id: String,
    ) -> Result<String> {
        self.find_or_create_session(&agent_id, &channel_id, &user_id)
    }

    pub fn get_session(&self, id: &str) -> Result<Session> {
        self.get_session_entry(id)?.snapshot()
    }

    pub fn update_session(&self, id: &str, session: Session) -> Result<()> {
        let new_key = SessionLookupKey::from_session(&session);

        if let Some(existing) = self.sessions.get(id) {
            let entry = existing.value().clone();
            let old_key = entry
                .snapshot()
                .map(|snapshot| SessionLookupKey::from_session(&snapshot))?;
            entry.replace(session.clone())?;

            if old_key != new_key {
                self.session_index.remove(&old_key);
            }
            self.session_index.insert(new_key, id.to_string());
            return Ok(());
        }

        self.session_index.insert(new_key, id.to_string());
        self.sessions
            .insert(id.to_string(), Arc::new(SessionEntry::new(session)));
        Ok(())
    }

    pub fn list_sessions(&self) -> Vec<Session> {
        self.sessions
            .iter()
            .filter_map(|entry| entry.value().snapshot().ok())
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
            self.insert_session_entry(session);
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

    async fn process_session_message(
        &self,
        session_id: &str,
        reply_target: Option<&str>,
        user_message: String,
        options: MessageHandlingOptions,
    ) -> Result<SessionMessageResult> {
        let entry = self.get_session_entry(session_id)?;
        let _processing_guard = entry.processing.lock().await;

        let mut session = entry.snapshot()?;
        if let Some(reply_target) = reply_target {
            session
                .metadata
                .insert("reply_target".to_string(), reply_target.to_string());
        }

        let agent = self.get_agent(&session.agent_id)?;
        session.add_message(MessageRole::User, user_message);
        let context = session.get_context(options.context_message_limit);

        let response = if let Some(timeout_ms) = options.agent_timeout_ms {
            tokio::time::timeout(
                std::time::Duration::from_millis(timeout_ms),
                agent.send_message(&mut session, context),
            )
            .await
            .map_err(|_| {
                AgentError::TimeoutError(format!("agent request exceeded {}ms", timeout_ms))
            })??
        } else {
            agent.send_message(&mut session, context).await?
        };

        session.add_message(MessageRole::Assistant, response.clone());
        if let Some(max_messages) = options.max_messages {
            session.trim_history(max_messages);
        }

        let result = SessionMessageResult {
            channel_id: session.channel_id.clone(),
            reply_target: session
                .metadata
                .get("reply_target")
                .cloned()
                .unwrap_or_else(|| session.user_id.clone()),
            response,
        };
        entry.replace(session)?;
        Ok(result)
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
        Ok(self
            .process_session_message(
                session_id,
                None,
                user_message,
                MessageHandlingOptions {
                    max_messages: None,
                    context_message_limit,
                    agent_timeout_ms,
                },
            )
            .await?
            .response)
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
        let entry = self.get_session_entry(session_id)?;
        let mut session = entry.snapshot()?;
        session.trim_history(max_messages);
        entry.replace(session)
    }

    pub async fn handle_incoming_message(
        &self,
        agent_id: &str,
        channel_id: &str,
        user_id: &str,
        reply_target: Option<&str>,
        user_message: String,
    ) -> Result<String> {
        self.handle_incoming_message_with_options(
            agent_id,
            channel_id,
            user_id,
            reply_target,
            user_message,
            MessageHandlingOptions::default(),
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
        self.handle_incoming_message_with_options(
            agent_id,
            channel_id,
            user_id,
            reply_target,
            user_message,
            MessageHandlingOptions {
                max_messages,
                ..MessageHandlingOptions::default()
            },
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
        let session_id = self.find_or_create_session(agent_id, channel_id, user_id)?;
        let result = self
            .process_session_message(&session_id, reply_target, user_message, options)
            .await?;

        let channel = self.get_channel(&result.channel_id)?;
        channel
            .send_message(&result.reply_target, &result.response)
            .await?;
        Ok(result.response)
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
        self.get_agent(agent_id)?;
        self.get_channel(channel_id)?;

        let lookup_key = SessionLookupKey::new(agent_id, channel_id, user_id);
        match self.session_index.entry(lookup_key) {
            Entry::Occupied(existing) => Ok(existing.get().clone()),
            Entry::Vacant(vacant) => {
                let session = Session::new(
                    agent_id.to_string(),
                    channel_id.to_string(),
                    user_id.to_string(),
                );
                let session_id = session.id.clone();
                self.sessions
                    .insert(session_id.clone(), Arc::new(SessionEntry::new(session)));
                vacant.insert(session_id.clone());
                Ok(session_id)
            }
        }
    }

    pub fn delete_session(&self, id: &str) -> Result<()> {
        let (_, entry) = self
            .sessions
            .remove(id)
            .ok_or_else(|| AgentError::SessionNotFound(id.to_string()))?;
        let lookup_key = SessionLookupKey::from_session(&entry.snapshot()?);
        self.session_index.remove(&lookup_key);
        Ok(())
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
            session_index: self.session_index.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Agent, ClaudeAgent};
    use crate::channel::{Channel, ChannelMessage, TelegramChannel};
    use crate::config::{AgentType, ChannelType};
    use crate::session::{Message, Session};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_agentim_creation() {
        let agentim = AgentIM::new();
        assert_eq!(agentim.list_agents().len(), 0);
        assert_eq!(agentim.list_channels().len(), 0);
    }

    #[test]
    fn test_register_agent() {
        let agentim = AgentIM::new();
        let agent = Arc::new(ClaudeAgent::new("claude1".to_string(), None));
        agentim
            .register_agent("claude1".to_string(), agent)
            .unwrap();
        assert_eq!(agentim.list_agents().len(), 1);
    }

    #[test]
    fn test_create_session() {
        let agentim = AgentIM::new();
        let agent = Arc::new(ClaudeAgent::new("claude1".to_string(), None));
        let channel = Arc::new(TelegramChannel::new("tg1".to_string()));

        agentim
            .register_agent("claude1".to_string(), agent)
            .unwrap();
        agentim
            .register_channel("tg1".to_string(), channel)
            .unwrap();

        let session_id = agentim
            .create_session(
                "claude1".to_string(),
                "tg1".to_string(),
                "user1".to_string(),
            )
            .unwrap();

        let session = agentim.get_session(&session_id).unwrap();
        assert_eq!(session.agent_id, "claude1");
        assert_eq!(session.channel_id, "tg1");
    }

    struct MockAgent;

    #[async_trait]
    impl Agent for MockAgent {
        fn agent_type(&self) -> AgentType {
            AgentType::Claude
        }

        fn id(&self) -> &str {
            "mock-agent"
        }

        async fn send_message(
            &self,
            _session: &mut Session,
            messages: Vec<Message>,
        ) -> Result<String> {
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

    struct SlowMockAgent {
        call_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Agent for SlowMockAgent {
        fn agent_type(&self) -> AgentType {
            AgentType::Claude
        }

        fn id(&self) -> &str {
            "slow-mock-agent"
        }

        async fn send_message(
            &self,
            _session: &mut Session,
            messages: Vec<Message>,
        ) -> Result<String> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            let last = messages
                .last()
                .map(|msg| msg.content.clone())
                .unwrap_or_default();
            Ok(format!("slow:{}", last))
        }

        async fn health_check(&self) -> Result<()> {
            Ok(())
        }
    }

    struct MockChannel {
        sent_messages: Arc<Mutex<Vec<(String, String)>>>,
    }

    #[async_trait]
    impl Channel for MockChannel {
        fn channel_type(&self) -> ChannelType {
            ChannelType::Discord
        }

        fn id(&self) -> &str {
            "mock-channel"
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

    #[tokio::test]
    async fn test_handle_incoming_message_autocreates_session_and_uses_reply_target() {
        let agentim = AgentIM::new();
        let sent_messages = Arc::new(Mutex::new(Vec::new()));

        agentim
            .register_agent("default-agent".to_string(), Arc::new(MockAgent))
            .unwrap();
        agentim
            .register_channel(
                "discord-bot".to_string(),
                Arc::new(MockChannel {
                    sent_messages: sent_messages.clone(),
                }),
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_concurrent_messages_share_a_single_session_and_preserve_history() {
        let agentim = AgentIM::new();
        let sent_messages = Arc::new(Mutex::new(Vec::new()));
        let call_count = Arc::new(AtomicUsize::new(0));

        agentim
            .register_agent(
                "default-agent".to_string(),
                Arc::new(SlowMockAgent {
                    call_count: call_count.clone(),
                }),
            )
            .unwrap();
        agentim
            .register_channel(
                "discord-bot".to_string(),
                Arc::new(MockChannel {
                    sent_messages: sent_messages.clone(),
                }),
            )
            .unwrap();

        let first = agentim.handle_incoming_message(
            "default-agent",
            "discord-bot",
            "user-1",
            Some("channel-42"),
            "first".to_string(),
        );
        let second = agentim.handle_incoming_message(
            "default-agent",
            "discord-bot",
            "user-1",
            Some("channel-42"),
            "second".to_string(),
        );

        let (first, second) = tokio::join!(first, second);
        assert!(matches!(
            first.as_deref(),
            Ok("slow:first") | Ok("slow:second")
        ));
        assert!(matches!(
            second.as_deref(),
            Ok("slow:first") | Ok("slow:second")
        ));

        let sessions = agentim.list_sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(call_count.load(Ordering::SeqCst), 2);

        let session = &sessions[0];
        let contents = session
            .messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>();
        assert_eq!(contents.len(), 4);
        assert!(contents.contains(&"first"));
        assert!(contents.contains(&"second"));
        assert!(contents.contains(&"slow:first"));
        assert!(contents.contains(&"slow:second"));

        let sent = sent_messages.lock().unwrap();
        assert_eq!(sent.len(), 2);
        assert!(sent.contains(&("channel-42".to_string(), "slow:first".to_string())));
        assert!(sent.contains(&("channel-42".to_string(), "slow:second".to_string())));
    }
}
