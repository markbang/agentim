use clap::{Parser, Subcommand};
use colored::*;
use prettytable::{row, Table};

#[derive(Parser)]
#[command(name = "AgentIM")]
#[command(about = "Multi-Channel AI Agent Manager", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Agent management
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
    /// Channel management
    Channel {
        #[command(subcommand)]
        action: ChannelAction,
    },
    /// Session management
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// System status
    Status,
    /// Interactive mode
    Interactive,
    /// Start Bot server
    BotServer {
        /// Telegram bot token
        #[arg(long)]
        telegram_token: Option<String>,
        /// Server address (default: 127.0.0.1:8080)
        #[arg(long, default_value = "127.0.0.1:8080")]
        addr: String,
    },
}

#[derive(Subcommand)]
pub enum AgentAction {
    /// List all agents
    List,
    /// Register a new agent
    Register {
        /// Agent ID
        #[arg(short, long)]
        id: String,
        /// Agent type (claude, codex, pi)
        #[arg(short, long)]
        agent_type: String,
        /// Model name (optional)
        #[arg(short, long)]
        model: Option<String>,
    },
    /// Remove an agent
    Remove {
        /// Agent ID
        #[arg(short, long)]
        id: String,
    },
    /// Health check for an agent
    Health {
        /// Agent ID
        #[arg(short, long)]
        id: String,
    },
}

#[derive(Subcommand)]
pub enum ChannelAction {
    /// List all channels
    List,
    /// Register a new channel
    Register {
        /// Channel ID
        #[arg(short, long)]
        id: String,
        /// Channel type (telegram, discord, feishu, qq)
        #[arg(short, long)]
        channel_type: String,
    },
    /// Remove a channel
    Remove {
        /// Channel ID
        #[arg(short, long)]
        id: String,
    },
    /// Health check for a channel
    Health {
        /// Channel ID
        #[arg(short, long)]
        id: String,
    },
}

#[derive(Subcommand)]
pub enum SessionAction {
    /// List all sessions
    List,
    /// Create a new session
    Create {
        /// Agent ID
        #[arg(short, long)]
        agent_id: String,
        /// Channel ID
        #[arg(short, long)]
        channel_id: String,
        /// User ID
        #[arg(short, long)]
        user_id: String,
    },
    /// Get session details
    Get {
        /// Session ID
        #[arg(short, long)]
        id: String,
    },
    /// Delete a session
    Delete {
        /// Session ID
        #[arg(short, long)]
        id: String,
    },
    /// Send message in a session
    Send {
        /// Session ID
        #[arg(short, long)]
        session_id: String,
        /// Message content
        #[arg(short, long)]
        message: String,
    },
}

pub fn print_header(text: &str) {
    println!("\n{}", text.bold().cyan());
    println!("{}", "=".repeat(text.len()).cyan());
}

pub fn print_success(text: &str) {
    println!("{} {}", "✓".green(), text);
}

pub fn print_error(text: &str) {
    println!("{} {}", "✗".red(), text);
}

pub fn print_info(text: &str) {
    println!("{} {}", "ℹ".blue(), text);
}

pub fn print_agents_table(agents: Vec<(String, String)>) {
    let mut table = Table::new();
    table.add_row(row!["Agent ID".bold(), "Type".bold()]);
    for (id, agent_type) in agents {
        table.add_row(row![id, agent_type]);
    }
    table.printstd();
}

pub fn print_channels_table(channels: Vec<(String, String)>) {
    let mut table = Table::new();
    table.add_row(row!["Channel ID".bold(), "Type".bold()]);
    for (id, channel_type) in channels {
        table.add_row(row![id, channel_type]);
    }
    table.printstd();
}

pub fn print_sessions_table(sessions: Vec<(String, String, String, String, usize)>) {
    let mut table = Table::new();
    table.add_row(row![
        "Session ID".bold(),
        "Agent".bold(),
        "Channel".bold(),
        "User".bold(),
        "Messages".bold()
    ]);
    for (id, agent, channel, user, msg_count) in sessions {
        table.add_row(row![id, agent, channel, user, msg_count]);
    }
    table.printstd();
}
