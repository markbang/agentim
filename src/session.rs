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
        if max_messages == 0 {
            self.messages.clear();
            self.updated_at = Utc::now();
            return;
        }

        if self.messages.len() <= max_messages {
            self.updated_at = Utc::now();
            return;
        }

        let system_ids = self
            .messages
            .iter()
            .filter(|message| message.role == MessageRole::System)
            .map(|message| message.id.clone())
            .collect::<Vec<_>>();

        let keep_ids = if system_ids.len() >= max_messages {
            system_ids
                .into_iter()
                .rev()
                .take(max_messages)
                .collect::<Vec<_>>()
        } else {
            let non_system_budget = max_messages - system_ids.len();
            let mut ids = system_ids;
            ids.extend(
                self.messages
                    .iter()
                    .rev()
                    .filter(|message| message.role != MessageRole::System)
                    .take(non_system_budget)
                    .map(|message| message.id.clone()),
            );
            ids
        };

        self.messages = self
            .messages
            .iter()
            .filter(|message| keep_ids.contains(&message.id))
            .cloned()
            .collect();
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

    #[test]
    fn test_trim_history_preserves_system_messages_when_possible() {
        let mut session = Session::new(
            "agent1".to_string(),
            "channel1".to_string(),
            "user1".to_string(),
        );
        session.add_message(MessageRole::System, "system".to_string());
        session.add_message(MessageRole::User, "u1".to_string());
        session.add_message(MessageRole::Assistant, "a1".to_string());
        session.add_message(MessageRole::User, "u2".to_string());
        session.add_message(MessageRole::Assistant, "a2".to_string());

        session.trim_history(3);

        assert_eq!(session.messages.len(), 3);
        assert_eq!(session.messages[0].role, MessageRole::System);
        assert_eq!(session.messages[1].content, "u2");
        assert_eq!(session.messages[2].content, "a2");
    }
}
