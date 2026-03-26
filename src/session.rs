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
        if max_messages == 0 {
            return Vec::new();
        }

        let history_summary = self.metadata.get("history_summary").cloned();
        let recent_budget = if history_summary.is_some() {
            max_messages.saturating_sub(1)
        } else {
            max_messages
        };

        let recent_messages = self
            .messages
            .iter()
            .rev()
            .take(recent_budget)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();

        if let Some(summary) = history_summary {
            let mut context = vec![Message {
                id: "history-summary".to_string(),
                role: MessageRole::System,
                content: format!("Earlier context summary: {}", summary),
                timestamp: self.updated_at,
            }];
            context.extend(recent_messages);
            context
        } else {
            recent_messages
        }
    }

    pub fn clear_history(&mut self) {
        self.messages.clear();
        self.updated_at = Utc::now();
    }

    pub fn trim_history(&mut self, max_messages: usize) {
        if max_messages == 0 {
            if !self.messages.is_empty() {
                self.update_history_summary(
                    &self.messages.iter().cloned().collect::<Vec<_>>(),
                );
            }
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

        let removed_messages = self
            .messages
            .iter()
            .filter(|message| !keep_ids.contains(&message.id))
            .cloned()
            .collect::<Vec<_>>();
        self.update_history_summary(&removed_messages);

        self.messages = self
            .messages
            .iter()
            .filter(|message| keep_ids.contains(&message.id))
            .cloned()
            .collect();
        self.updated_at = Utc::now();
    }

    fn update_history_summary(&mut self, removed_messages: &[Message]) {
        let new_fragments = removed_messages
            .iter()
            .filter(|message| message.role != MessageRole::System)
            .map(|message| {
                format!(
                    "[{}] {}",
                    message.role,
                    message.content.replace('\n', " ")
                )
            })
            .collect::<Vec<_>>();

        if new_fragments.is_empty() {
            return;
        }

        let mut summary = self
            .metadata
            .get("history_summary")
            .cloned()
            .unwrap_or_default();
        if !summary.is_empty() {
            summary.push_str(" | ");
        }
        summary.push_str(&new_fragments.join(" | "));

        let max_chars = 600;
        if summary.chars().count() > max_chars {
            let tail = summary
                .chars()
                .rev()
                .take(max_chars - 3)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<String>();
            summary = format!("...{}", tail);
        }

        self.metadata.insert("history_summary".to_string(), summary);
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

    #[test]
    fn test_trim_history_creates_summary_for_removed_messages() {
        let mut session = Session::new(
            "agent1".to_string(),
            "channel1".to_string(),
            "user1".to_string(),
        );
        session.add_message(MessageRole::User, "u1".to_string());
        session.add_message(MessageRole::Assistant, "a1".to_string());
        session.add_message(MessageRole::User, "u2".to_string());
        session.add_message(MessageRole::Assistant, "a2".to_string());

        session.trim_history(2);
        let context = session.get_context(3);

        assert_eq!(context[0].role, MessageRole::System);
        assert!(context[0].content.starts_with("Earlier context summary:"));
        assert!(context[0].content.contains("u1"));
        assert_eq!(context[1].content, "u2");
        assert_eq!(context[2].content, "a2");
    }
}
