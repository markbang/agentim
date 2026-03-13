mod agent;
mod channel;
mod cli;
mod config;
mod error;
mod manager;
mod session;

use agent::{ClaudeAgent, CodexAgent, PiAgent};
use channel::{DiscordChannel, FeishuChannel, QQChannel, TelegramChannel};
use clap::Parser;
use cli::{AgentAction, ChannelAction, Cli, Commands, SessionAction};
use manager::AgentIM;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let agentim = AgentIM::new();

    match cli.command {
        Commands::Agent { action } => handle_agent_command(action, &agentim).await?,
        Commands::Channel { action } => handle_channel_command(action, &agentim).await?,
        Commands::Session { action } => handle_session_command(action, &agentim).await?,
        Commands::Status => handle_status(&agentim).await?,
        Commands::Interactive => handle_interactive(&agentim).await?,
    }

    Ok(())
}

async fn handle_agent_command(action: AgentAction, agentim: &AgentIM) -> anyhow::Result<()> {
    match action {
        AgentAction::List => {
            cli::print_header("Registered Agents");
            let agents = agentim.list_agents();
            if agents.is_empty() {
                cli::print_info("No agents registered");
            } else {
                let agent_list: Vec<_> = agents
                    .iter()
                    .map(|id| (id.clone(), "N/A".to_string()))
                    .collect();
                cli::print_agents_table(agent_list);
            }
        }
        AgentAction::Register {
            id,
            agent_type,
            model,
        } => {
            let agent: Arc<dyn agent::Agent> = match agent_type.as_str() {
                "claude" => Arc::new(ClaudeAgent::new(id.clone(), model)),
                "codex" => Arc::new(CodexAgent::new(id.clone(), model)),
                "pi" => Arc::new(PiAgent::new(id.clone())),
                _ => {
                    cli::print_error(&format!("Unknown agent type: {}", agent_type));
                    return Ok(());
                }
            };
            agentim.register_agent(id.clone(), agent)?;
            cli::print_success(&format!("Agent '{}' registered", id));
        }
        AgentAction::Remove { id } => {
            cli::print_info(&format!("Remove agent '{}' - not yet implemented", id));
        }
        AgentAction::Health { id } => match agentim.get_agent(&id) {
            Ok(agent) => match agent.health_check().await {
                Ok(_) => cli::print_success(&format!("Agent '{}' is healthy", id)),
                Err(e) => cli::print_error(&format!("Agent '{}' health check failed: {}", id, e)),
            },
            Err(e) => cli::print_error(&format!("Agent not found: {}", e)),
        },
    }
    Ok(())
}

async fn handle_channel_command(action: ChannelAction, agentim: &AgentIM) -> anyhow::Result<()> {
    match action {
        ChannelAction::List => {
            cli::print_header("Registered Channels");
            let channels = agentim.list_channels();
            if channels.is_empty() {
                cli::print_info("No channels registered");
            } else {
                let channel_list: Vec<_> = channels
                    .iter()
                    .map(|id| (id.clone(), "N/A".to_string()))
                    .collect();
                cli::print_channels_table(channel_list);
            }
        }
        ChannelAction::Register { id, channel_type } => {
            let channel: Arc<dyn channel::Channel> = match channel_type.as_str() {
                "telegram" => Arc::new(TelegramChannel::new(id.clone())),
                "discord" => Arc::new(DiscordChannel::new(id.clone())),
                "feishu" => Arc::new(FeishuChannel::new(id.clone())),
                "qq" => Arc::new(QQChannel::new(id.clone())),
                _ => {
                    cli::print_error(&format!("Unknown channel type: {}", channel_type));
                    return Ok(());
                }
            };
            agentim.register_channel(id.clone(), channel)?;
            cli::print_success(&format!("Channel '{}' registered", id));
        }
        ChannelAction::Remove { id } => {
            cli::print_info(&format!("Remove channel '{}' - not yet implemented", id));
        }
        ChannelAction::Health { id } => match agentim.get_channel(&id) {
            Ok(channel) => match channel.health_check().await {
                Ok(_) => cli::print_success(&format!("Channel '{}' is healthy", id)),
                Err(e) => cli::print_error(&format!("Channel '{}' health check failed: {}", id, e)),
            },
            Err(e) => cli::print_error(&format!("Channel not found: {}", e)),
        },
    }
    Ok(())
}

async fn handle_session_command(action: SessionAction, agentim: &AgentIM) -> anyhow::Result<()> {
    match action {
        SessionAction::List => {
            cli::print_header("Active Sessions");
            let sessions = agentim.list_sessions();
            if sessions.is_empty() {
                cli::print_info("No active sessions");
            } else {
                let session_list: Vec<_> = sessions
                    .iter()
                    .map(|s| {
                        (
                            s.id.clone(),
                            s.agent_id.clone(),
                            s.channel_id.clone(),
                            s.user_id.clone(),
                            s.messages.len(),
                        )
                    })
                    .collect();
                cli::print_sessions_table(session_list);
            }
        }
        SessionAction::Create {
            agent_id,
            channel_id,
            user_id,
        } => match agentim.create_session(agent_id.clone(), channel_id.clone(), user_id.clone()) {
            Ok(session_id) => {
                cli::print_success(&format!("Session created: {}", session_id));
                cli::print_info(&format!(
                    "Agent: {}, Channel: {}, User: {}",
                    agent_id, channel_id, user_id
                ));
            }
            Err(e) => cli::print_error(&format!("Failed to create session: {}", e)),
        },
        SessionAction::Get { id } => match agentim.get_session(&id) {
            Ok(session) => {
                cli::print_header(&format!("Session: {}", id));
                cli::print_info(&format!("Agent: {}", session.agent_id));
                cli::print_info(&format!("Channel: {}", session.channel_id));
                cli::print_info(&format!("User: {}", session.user_id));
                cli::print_info(&format!("Messages: {}", session.messages.len()));
                cli::print_info(&format!("Created: {}", session.created_at));
                cli::print_info(&format!("Updated: {}", session.updated_at));
            }
            Err(e) => cli::print_error(&format!("Session not found: {}", e)),
        },
        SessionAction::Delete { id } => match agentim.delete_session(&id) {
            Ok(_) => cli::print_success(&format!("Session '{}' deleted", id)),
            Err(e) => cli::print_error(&format!("Failed to delete session: {}", e)),
        },
        SessionAction::Send {
            session_id,
            message,
        } => {
            cli::print_info(&format!(
                "Sending message to session '{}': {}",
                session_id, message
            ));
            cli::print_info("Message sending - not yet fully implemented");
        }
    }
    Ok(())
}

async fn handle_status(agentim: &AgentIM) -> anyhow::Result<()> {
    cli::print_header("System Status");

    let agents = agentim.list_agents();
    let channels = agentim.list_channels();
    let sessions = agentim.list_sessions();

    cli::print_info(&format!("Registered Agents: {}", agents.len()));
    cli::print_info(&format!("Registered Channels: {}", channels.len()));
    cli::print_info(&format!("Active Sessions: {}", sessions.len()));

    if let Ok(_) = agentim.health_check().await {
        cli::print_success("All systems healthy");
    } else {
        cli::print_error("Some systems are unhealthy");
    }

    Ok(())
}

async fn handle_interactive(_agentim: &AgentIM) -> anyhow::Result<()> {
    cli::print_header("Interactive Mode");
    cli::print_info("Interactive mode - not yet implemented");
    Ok(())
}
