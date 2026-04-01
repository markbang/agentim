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
                self.update_history_summary(&self.messages.iter().cloned().collect::<Vec<_>>());
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
        let compact_messages = removed_messages
            .iter()
            .filter(|message| message.role != MessageRole::System)
            .cloned()
            .collect::<Vec<_>>();

        let mut new_fragments = Vec::new();
        let mut index = 0;
        while index < compact_messages.len() {
            let current = &compact_messages[index];
            let current_content = current.content.replace('\n', " ");

            if current.role == MessageRole::User && index + 1 < compact_messages.len() {
                let next = &compact_messages[index + 1];
                if next.role == MessageRole::Assistant {
                    new_fragments.push(format!(
                        "[turn] {} => {}",
                        current_content,
                        next.content.replace('\n', " ")
                    ));
                    index += 2;
                    continue;
                }
            }

            new_fragments.push(format!("[{}] {}", current.role, current_content));
            index += 1;
        }

        if new_fragments.is_empty() {
            return;
        }

        let mut summary_fragments = Vec::new();
        let mut omitted_fragments = 0usize;

        if let Some(existing_summary) = self.metadata.get("history_summary") {
            for fragment in existing_summary
                .split(" | ")
                .filter(|fragment| !fragment.is_empty())
            {
                if summary_fragments.is_empty() {
                    if let Some(existing_omitted) = Self::parse_summary_omitted_fragment(fragment) {
                        omitted_fragments += existing_omitted;
                        continue;
                    }
                }

                summary_fragments.push(fragment.to_string());
            }
        }

        summary_fragments.extend(new_fragments);

        let max_chars = 600;
        while !summary_fragments.is_empty()
            && Self::render_history_summary(&summary_fragments, omitted_fragments)
                .chars()
                .count()
                > max_chars
        {
            summary_fragments.remove(0);
            omitted_fragments += 1;
        }

        let summary = Self::render_history_summary(&summary_fragments, omitted_fragments);
        self.metadata.insert("history_summary".to_string(), summary);
    }

    fn parse_summary_omitted_fragment(fragment: &str) -> Option<usize> {
        fragment
            .strip_prefix("[summary] ")?
            .strip_suffix(" older fragment(s) omitted")?
            .parse::<usize>()
            .ok()
    }

    fn render_history_summary(fragments: &[String], omitted_fragments: usize) -> String {
        let mut parts = Vec::new();
        if omitted_fragments > 0 {
            parts.push(format!(
                "[summary] {} older fragment(s) omitted",
                omitted_fragments
            ));
        }
        parts.extend(fragments.iter().cloned());
        parts.join(" | ")
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
        assert!(context[0].content.contains("[turn] u1 => a1"));
        assert_eq!(context[1].content, "u2");
        assert_eq!(context[2].content, "a2");
    }

    #[test]
    fn test_trim_history_truncates_summary_on_fragment_boundaries() {
        let mut session = Session::new(
            "agent1".to_string(),
            "channel1".to_string(),
            "user1".to_string(),
        );

        for index in 0..10 {
            let content = format!("turn-{index}-abcdefghijklmnopqrstuvwxyz0123456789");
            session.add_message(MessageRole::User, content.clone());
            session.add_message(MessageRole::Assistant, format!("reply-{content}"));
        }

        session.trim_history(2);
        let summary = session.metadata.get("history_summary").cloned().unwrap();
        let fragments = summary.split(" | ").collect::<Vec<_>>();

        assert!(summary.starts_with("[summary] "));
        assert!(fragments.len() > 1);
        assert!(fragments[0].contains("older fragment(s) omitted"));
        assert!(fragments[1..]
            .iter()
            .all(|fragment| fragment.starts_with("[turn] ")
                || fragment.starts_with("[user] ")
                || fragment.starts_with("[assistant] ")));
        assert!(!summary.starts_with("..."));
    }
}
