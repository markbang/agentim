use clap::Parser;
use colored::*;

#[derive(Parser)]
#[command(name = "AgentIM")]
#[command(about = "Multi-Channel AI Agent Manager", long_about = None)]
pub struct Args {
    /// Telegram bot token
    #[arg(long)]
    pub telegram_token: Option<String>,

    /// Native Telegram webhook secret token expected in x-telegram-bot-api-secret-token
    #[arg(long)]
    pub telegram_webhook_secret_token: Option<String>,

    /// Discord bot token
    #[arg(long)]
    pub discord_token: Option<String>,

    /// Deprecated fallback: Feishu credentials as "app_id:app_secret"
    #[arg(long)]
    pub feishu_token: Option<String>,

    /// Feishu app id
    #[arg(long)]
    pub feishu_app_id: Option<String>,

    /// Feishu app secret
    #[arg(long)]
    pub feishu_app_secret: Option<String>,

    /// Deprecated fallback: QQ credentials as "bot_id:bot_token"
    #[arg(long)]
    pub qq_token: Option<String>,

    /// QQ bot id
    #[arg(long)]
    pub qq_bot_id: Option<String>,

    /// QQ bot token
    #[arg(long)]
    pub qq_bot_token: Option<String>,

    /// Default agent type to use (claude, codex, pi) when no channel-specific override is set
    #[arg(long)]
    pub agent: Option<String>,

    /// Agent override for Telegram traffic
    #[arg(long)]
    pub telegram_agent: Option<String>,

    /// Agent override for Discord traffic
    #[arg(long)]
    pub discord_agent: Option<String>,

    /// Agent override for Feishu traffic
    #[arg(long)]
    pub feishu_agent: Option<String>,

    /// Agent override for QQ traffic
    #[arg(long)]
    pub qq_agent: Option<String>,

    /// Load runtime options from this JSON file; CLI flags still take precedence
    #[arg(long)]
    pub config_file: Option<String>,

    /// Validate startup configuration and exit before starting the server
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// Persist sessions to this JSON file and reload them on startup
    #[arg(long)]
    pub state_file: Option<String>,

    /// Trim session history to this many messages after each webhook round-trip
    #[arg(long)]
    pub max_session_messages: Option<usize>,

    /// Require this shared secret in the x-agentim-secret header for all protected routes
    #[arg(long)]
    pub webhook_secret: Option<String>,

    /// Verify webhook requests with x-agentim-timestamp/x-agentim-nonce/x-agentim-signature HMAC headers
    #[arg(long)]
    pub webhook_signing_secret: Option<String>,

    /// Maximum allowed timestamp skew in seconds for signed webhooks
    #[arg(long)]
    pub webhook_max_skew_seconds: Option<i64>,

    /// Server address (default: 127.0.0.1:8080)
    #[arg(long)]
    pub addr: Option<String>,
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
