use clap::Parser;
use colored::*;

#[derive(Parser)]
#[command(name = "AgentIM")]
#[command(about = "Multi-Channel AI Agent Manager", long_about = None)]
pub struct Args {
    /// Telegram bot token
    #[arg(long)]
    pub telegram_token: Option<String>,

    /// Discord bot token
    #[arg(long)]
    pub discord_token: Option<String>,

    /// Feishu bot token
    #[arg(long)]
    pub feishu_token: Option<String>,

    /// QQ bot token
    #[arg(long)]
    pub qq_token: Option<String>,

    /// Agent type to use (claude, codex, pi) - default: claude
    #[arg(long, default_value = "claude")]
    pub agent: String,

    /// Server address (default: 127.0.0.1:8080)
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub addr: String,
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
