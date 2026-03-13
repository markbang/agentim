use agentim::{AgentIM, agent::{ClaudeAgent, CodexAgent, PiAgent}, channel::{TelegramChannel, DiscordChannel, FeishuChannel, QQChannel}, session::MessageRole};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // Create AgentIM instance
    let agentim = AgentIM::new();

    // Register agents
    let claude = Arc::new(ClaudeAgent::new(
        "claude-main".to_string(),
        std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
        Some("claude-3-5-sonnet-20241022".to_string()),
        None,
    ));

    let codex = Arc::new(CodexAgent::new(
        "codex-main".to_string(),
        std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        None,
    ));

    let pi = Arc::new(PiAgent::new(
        "pi-main".to_string(),
        std::env::var("PI_API_KEY").unwrap_or_default(),
    ));

    agentim.register_agent("claude-main".to_string(), claude)?;
    agentim.register_agent("codex-main".to_string(), codex)?;
    agentim.register_agent("pi-main".to_string(), pi)?;

    println!("✓ Registered 3 agents");

    // Register channels
    let telegram = Arc::new(TelegramChannel::new(
        "tg-main".to_string(),
        std::env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default(),
    ));

    let discord = Arc::new(DiscordChannel::new(
        "discord-main".to_string(),
        std::env::var("DISCORD_BOT_TOKEN").unwrap_or_default(),
    ));

    let feishu = Arc::new(FeishuChannel::new(
        "feishu-main".to_string(),
        std::env::var("FEISHU_APP_ID").unwrap_or_default(),
        std::env::var("FEISHU_APP_SECRET").unwrap_or_default(),
    ));

    let qq = Arc::new(QQChannel::new(
        "qq-main".to_string(),
        std::env::var("QQ_BOT_ID").unwrap_or_default(),
        std::env::var("QQ_BOT_TOKEN").unwrap_or_default(),
    ));

    agentim.register_channel("tg-main".to_string(), telegram)?;
    agentim.register_channel("discord-main".to_string(), discord)?;
    agentim.register_channel("feishu-main".to_string(), feishu)?;
    agentim.register_channel("qq-main".to_string(), qq)?;

    println!("✓ Registered 4 channels");

    // Create a session
    let session_id = agentim.create_session(
        "claude-main".to_string(),
        "tg-main".to_string(),
        "user123".to_string(),
    )?;

    println!("✓ Created session: {}", session_id);

    // Get session and add messages
    let mut session = agentim.get_session(&session_id)?;
    session.add_message(MessageRole::System, "You are a helpful coding assistant.".to_string());
    agentim.update_session(&session_id, session)?;

    println!("✓ Session initialized with system prompt");

    // List all resources
    println!("\nRegistered Agents:");
    for agent_id in agentim.list_agents() {
        println!("  - {}", agent_id);
    }

    println!("\nRegistered Channels:");
    for channel_id in agentim.list_channels() {
        println!("  - {}", channel_id);
    }

    println!("\nActive Sessions:");
    for session in agentim.list_sessions() {
        println!("  - {} (agent: {}, channel: {}, user: {})",
            session.id, session.agent_id, session.channel_id, session.user_id);
    }

    Ok(())
}
