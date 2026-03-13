use crate::agent::Agent;
use crate::config::AgentType;
use crate::error::Result;
use crate::session::Message;
use async_trait::async_trait;
use std::io::{self, Write};

pub struct CliAgent {
    id: String,
}

impl CliAgent {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Agent for CliAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Claude // 使用Claude作为默认类型
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, messages: Vec<Message>) -> Result<String> {
        // Display context to user
        println!("\n📋 Message History:");
        for (i, msg) in messages.iter().enumerate() {
            let role = match msg.role {
                crate::session::MessageRole::User => "👤 User",
                crate::session::MessageRole::Assistant => "🤖 Assistant",
                crate::session::MessageRole::System => "⚙️ System",
            };
            println!("  {}. {}: {}", i + 1, role, msg.content);
        }

        // Prompt user for response
        println!("\n💬 Enter your response (or 'skip' to skip):");
        print!("> ");
        io::stdout().flush()?;

        let mut response = String::new();
        io::stdin().read_line(&mut response)?;
        let response = response.trim().to_string();

        if response.to_lowercase() == "skip" {
            return Ok("[CLI Agent: Message skipped]".to_string());
        }

        Ok(response)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

