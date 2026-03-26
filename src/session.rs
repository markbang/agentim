use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
            MessageRole::System => write!(f, "system"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub agent_id: String,
    pub channel_id: String,
    pub user_id: String,
    pub messages: VecDeque<Message>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: std::collections::HashMap<String, String>,
}

impl Session {
    pub fn new(agent_id: String, channel_id: String, user_id: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            agent_id,
            channel_id,
            user_id,
            messages: VecDeque::new(),
            created_at: now,
            updated_at: now,
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn add_message(&mut self, role: MessageRole, content: String) {
        let message = Message {
            id: Uuid::new_v4().to_string(),
            role,
            content,
            timestamp: Utc::now(),
        };
        self.messages.push_back(message);
        self.updated_at = Utc::now();
    }

    pub fn get_context(&self, max_messages: usize) -> Vec<Message> {
        self.messages
            .iter()
            .rev()
            .take(max_messages)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    pub fn clear_history(&mut self) {
        self.messages.clear();
        self.updated_at = Utc::now();
    }

    pub fn trim_history(&mut self, max_messages: usize) {
        while self.messages.len() > max_messages {
            self.messages.pop_front();
        }
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = Session::new(
            "agent1".to_string(),
            "channel1".to_string(),
            "user1".to_string(),
        );
        assert_eq!(session.agent_id, "agent1");
        assert_eq!(session.messages.len(), 0);
    }

    #[test]
    fn test_add_message() {
        let mut session = Session::new(
            "agent1".to_string(),
            "channel1".to_string(),
            "user1".to_string(),
        );
        session.add_message(MessageRole::User, "Hello".to_string());
        assert_eq!(session.messages.len(), 1);
    }
}
