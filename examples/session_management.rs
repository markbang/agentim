use agentim::{AgentIM, agent::ClaudeAgent, channel::TelegramChannel, session::MessageRole};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let agentim = AgentIM::new();

    // Setup agent and channel
    let claude = Arc::new(ClaudeAgent::new(
        "claude-demo".to_string(),
        "sk-ant-test-key".to_string(),
        Some("claude-3-5-sonnet-20241022".to_string()),
        None,
    ));

    let telegram = Arc::new(TelegramChannel::new(
        "tg-demo".to_string(),
        "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11".to_string(),
    ));

    agentim.register_agent("claude-demo".to_string(), claude)?;
    agentim.register_channel("tg-demo".to_string(), telegram)?;

    // Create multiple sessions for different users
    let users = vec!["alice", "bob", "charlie"];
    let mut session_ids = Vec::new();

    for user in users {
        let session_id = agentim.create_session(
            "claude-demo".to_string(),
            "tg-demo".to_string(),
            user.to_string(),
        )?;
        session_ids.push((user.to_string(), session_id));
        println!("Created session for {}", user);
    }

    // Simulate conversation flow
    println!("\n--- Simulating conversation flow ---\n");

    for (user, session_id) in &session_ids {
        let mut session = agentim.get_session(session_id)?;

        // Add system context
        session.add_message(
            MessageRole::System,
            "You are a helpful assistant. Keep responses concise.".to_string(),
        );

        // Add user message
        session.add_message(
            MessageRole::User,
            format!("Hello, I'm {}. What can you help me with?", user),
        );

        agentim.update_session(session_id, session)?;

        // Get context for agent
        let session = agentim.get_session(session_id)?;
        let context = session.get_context(5);

        println!("Session {} context ({} messages):", session_id, context.len());
        for msg in &context {
            println!("  [{}] {}", msg.role, msg.content);
        }
        println!();
    }

    // Show session statistics
    println!("--- Session Statistics ---");
    let all_sessions = agentim.list_sessions();
    println!("Total sessions: {}", all_sessions.len());

    for session in all_sessions {
        println!(
            "Session {}: {} messages, last updated: {}",
            session.id,
            session.messages.len(),
            session.updated_at
        );
    }

    // Cleanup
    println!("\n--- Cleanup ---");
    for (user, session_id) in session_ids {
        agentim.delete_session(&session_id)?;
        println!("Deleted session for {}", user);
    }

    println!("Remaining sessions: {}", agentim.list_sessions().len());

    Ok(())
}
