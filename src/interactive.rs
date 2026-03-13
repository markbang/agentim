use crate::agent::{ClaudeAgent, CodexAgent, PiAgent};
use crate::channel::{DiscordChannel, FeishuChannel, QQChannel, TelegramChannel};
use crate::cli;
use crate::manager::AgentIM;
use std::io::{self, Write};
use std::sync::Arc;

pub async fn run_interactive(agentim: &AgentIM) -> anyhow::Result<()> {
    cli::print_header("AgentIM Interactive Setup");
    cli::print_info("Welcome to AgentIM! Let's set up your agents and channels.");
    println!();

    loop {
        println!();
        cli::print_header("Main Menu");
        println!("1. Register Agent");
        println!("2. Register Channel");
        println!("3. Create Session");
        println!("4. Send Message");
        println!("5. View Status");
        println!("6. Exit");
        print!("\nSelect option (1-6): ");
        io::stdout().flush()?;

        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;

        match choice.trim() {
            "1" => register_agent_interactive(agentim).await?,
            "2" => register_channel_interactive(agentim).await?,
            "3" => create_session_interactive(agentim).await?,
            "4" => send_message_interactive(agentim).await?,
            "5" => view_status(agentim).await?,
            "6" => {
                cli::print_success("Goodbye!");
                break;
            }
            _ => cli::print_error("Invalid option. Please try again."),
        }
    }

    Ok(())
}

async fn register_agent_interactive(agentim: &AgentIM) -> anyhow::Result<()> {
    cli::print_header("Register Agent");

    print!("Agent ID: ");
    io::stdout().flush()?;
    let mut id = String::new();
    io::stdin().read_line(&mut id)?;
    let id = id.trim().to_string();

    println!("\nAgent Type:");
    println!("1. Claude");
    println!("2. Codex");
    println!("3. Pi");
    println!("4. CLI (Interactive)");
    print!("Select (1-4): ");
    io::stdout().flush()?;

    let mut agent_type = String::new();
    io::stdin().read_line(&mut agent_type)?;

    let agent: Arc<dyn crate::agent::Agent> = match agent_type.trim() {
        "1" => {
            print!("Model (default: claude-3-5-sonnet-20241022): ");
            io::stdout().flush()?;
            let mut model = String::new();
            io::stdin().read_line(&mut model)?;
            let model = model.trim();
            let model = if model.is_empty() {
                None
            } else {
                Some(model.to_string())
            };
            Arc::new(ClaudeAgent::new(id.clone(), model))
        }
        "2" => {
            print!("Model (default: code-davinci-002): ");
            io::stdout().flush()?;
            let mut model = String::new();
            io::stdin().read_line(&mut model)?;
            let model = model.trim();
            let model = if model.is_empty() {
                None
            } else {
                Some(model.to_string())
            };
            Arc::new(CodexAgent::new(id.clone(), model))
        }
        "3" => Arc::new(PiAgent::new(id.clone())),
        "4" => Arc::new(crate::agents::CliAgent::new(id.clone())),
        _ => {
            cli::print_error("Invalid agent type");
            return Ok(());
        }
    };

    agentim.register_agent(id.clone(), agent)?;
    cli::print_success(&format!("Agent '{}' registered successfully!", id));
    Ok(())
}

async fn register_channel_interactive(agentim: &AgentIM) -> anyhow::Result<()> {
    cli::print_header("Register Channel");

    print!("Channel ID: ");
    io::stdout().flush()?;
    let mut id = String::new();
    io::stdin().read_line(&mut id)?;
    let id = id.trim().to_string();

    println!("\nChannel Type:");
    println!("1. Telegram");
    println!("2. Discord");
    println!("3. Feishu");
    println!("4. QQ");
    print!("Select (1-4): ");
    io::stdout().flush()?;

    let mut channel_type = String::new();
    io::stdin().read_line(&mut channel_type)?;

    let channel: Arc<dyn crate::channel::Channel> = match channel_type.trim() {
        "1" => Arc::new(TelegramChannel::new(id.clone())),
        "2" => Arc::new(DiscordChannel::new(id.clone())),
        "3" => Arc::new(FeishuChannel::new(id.clone())),
        "4" => Arc::new(QQChannel::new(id.clone())),
        _ => {
            cli::print_error("Invalid channel type");
            return Ok(());
        }
    };

    agentim.register_channel(id.clone(), channel)?;
    cli::print_success(&format!("Channel '{}' registered successfully!", id));
    Ok(())
}

async fn create_session_interactive(agentim: &AgentIM) -> anyhow::Result<()> {
    cli::print_header("Create Session");

    let agents = agentim.list_agents();
    if agents.is_empty() {
        cli::print_error("No agents registered. Please register an agent first.");
        return Ok(());
    }

    let channels = agentim.list_channels();
    if channels.is_empty() {
        cli::print_error("No channels registered. Please register a channel first.");
        return Ok(());
    }

    println!("\nAvailable Agents:");
    for (i, agent) in agents.iter().enumerate() {
        println!("{}. {}", i + 1, agent);
    }
    print!("Select agent (1-{}): ", agents.len());
    io::stdout().flush()?;
    let mut agent_choice = String::new();
    io::stdin().read_line(&mut agent_choice)?;
    let agent_idx = agent_choice.trim().parse::<usize>().unwrap_or(0);
    if agent_idx < 1 || agent_idx > agents.len() {
        cli::print_error("Invalid selection");
        return Ok(());
    }
    let agent_id = agents[agent_idx - 1].clone();

    println!("\nAvailable Channels:");
    for (i, channel) in channels.iter().enumerate() {
        println!("{}. {}", i + 1, channel);
    }
    print!("Select channel (1-{}): ", channels.len());
    io::stdout().flush()?;
    let mut channel_choice = String::new();
    io::stdin().read_line(&mut channel_choice)?;
    let channel_idx = channel_choice.trim().parse::<usize>().unwrap_or(0);
    if channel_idx < 1 || channel_idx > channels.len() {
        cli::print_error("Invalid selection");
        return Ok(());
    }
    let channel_id = channels[channel_idx - 1].clone();

    print!("User ID: ");
    io::stdout().flush()?;
    let mut user_id = String::new();
    io::stdin().read_line(&mut user_id)?;
    let user_id = user_id.trim().to_string();

    match agentim.create_session(agent_id.clone(), channel_id.clone(), user_id.clone()) {
        Ok(session_id) => {
            cli::print_success(&format!("Session created: {}", session_id));
            cli::print_info(&format!(
                "Agent: {}, Channel: {}, User: {}",
                agent_id, channel_id, user_id
            ));
        }
        Err(e) => cli::print_error(&format!("Failed to create session: {}", e)),
    }

    Ok(())
}

async fn send_message_interactive(agentim: &AgentIM) -> anyhow::Result<()> {
    cli::print_header("Send Message");

    let sessions = agentim.list_sessions();
    if sessions.is_empty() {
        cli::print_error("No active sessions. Please create a session first.");
        return Ok(());
    }

    println!("\nActive Sessions:");
    for (i, session) in sessions.iter().enumerate() {
        println!(
            "{}. {} (Agent: {}, Channel: {}, User: {})",
            i + 1, session.id, session.agent_id, session.channel_id, session.user_id
        );
    }
    print!("Select session (1-{}): ", sessions.len());
    io::stdout().flush()?;
    let mut session_choice = String::new();
    io::stdin().read_line(&mut session_choice)?;
    let session_idx = session_choice.trim().parse::<usize>().unwrap_or(0);
    if session_idx < 1 || session_idx > sessions.len() {
        cli::print_error("Invalid selection");
        return Ok(());
    }
    let session_id = sessions[session_idx - 1].id.clone();

    print!("Message: ");
    io::stdout().flush()?;
    let mut message = String::new();
    io::stdin().read_line(&mut message)?;
    let message = message.trim().to_string();

    cli::print_info("Processing message...");
    match agentim.send_to_agent(&session_id, message).await {
        Ok(response) => {
            cli::print_success("Message sent to agent");
            cli::print_info(&format!("Agent response: {}", response));
            match agentim.send_to_channel(&session_id, response).await {
                Ok(_) => cli::print_success("Response sent to channel"),
                Err(e) => cli::print_error(&format!("Failed to send to channel: {}", e)),
            }
        }
        Err(e) => cli::print_error(&format!("Failed to send message: {}", e)),
    }

    Ok(())
}

async fn view_status(agentim: &AgentIM) -> anyhow::Result<()> {
    cli::print_header("System Status");

    let agents = agentim.list_agents();
    let channels = agentim.list_channels();
    let sessions = agentim.list_sessions();

    cli::print_info(&format!("Registered Agents: {}", agents.len()));
    for agent in agents {
        println!("  • {}", agent);
    }

    println!();
    cli::print_info(&format!("Registered Channels: {}", channels.len()));
    for channel in channels {
        println!("  • {}", channel);
    }

    println!();
    cli::print_info(&format!("Active Sessions: {}", sessions.len()));
    for session in sessions {
        println!(
            "  • {} (Agent: {}, Channel: {}, User: {}, Messages: {})",
            session.id, session.agent_id, session.channel_id, session.user_id, session.messages.len()
        );
    }

    if agentim.health_check().await.is_ok() {
        println!();
        cli::print_success("All systems healthy");
    } else {
        println!();
        cli::print_error("Some systems are unhealthy");
    }

    Ok(())
}
