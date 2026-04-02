use clap::Parser;
use colored::*;

#[derive(Parser)]
#[command(name = "AgentIM")]
#[command(about = "ACP-first IM bridge for coding agents", long_about = None)]
pub struct Args {
    /// Telegram bot token
    #[arg(long)]
    pub telegram_token: Option<String>,

    /// Native Telegram webhook secret token expected in x-telegram-bot-api-secret-token
    #[arg(long)]
    pub telegram_webhook_secret_token: Option<String>,

    /// Use Telegram getUpdates long polling instead of waiting for Telegram webhook callbacks
    #[arg(long, default_value_t = false)]
    pub telegram_poll: bool,

    /// Discord bot token
    #[arg(long)]
    pub discord_token: Option<String>,

    /// Native Discord interaction public key for x-signature-ed25519 verification
    #[arg(long)]
    pub discord_interaction_public_key: Option<String>,

    /// Use the Discord Gateway instead of waiting for Discord webhook callbacks
    #[arg(long, default_value_t = false)]
    pub discord_gateway: bool,

    /// Deprecated fallback: Feishu credentials as "app_id:app_secret"
    #[arg(long)]
    pub feishu_token: Option<String>,

    /// Feishu app id
    #[arg(long)]
    pub feishu_app_id: Option<String>,

    /// Feishu app secret
    #[arg(long)]
    pub feishu_app_secret: Option<String>,

    /// Native Feishu webhook verification token expected in the payload's token field
    #[arg(long)]
    pub feishu_verification_token: Option<String>,

    /// Deprecated fallback: QQ credentials as "bot_id:bot_token"
    #[arg(long)]
    pub qq_token: Option<String>,

    /// QQ bot id
    #[arg(long)]
    pub qq_bot_id: Option<String>,

    /// QQ bot token
    #[arg(long)]
    pub qq_bot_token: Option<String>,

    /// Slack bot token (xoxb-...)
    #[arg(long)]
    pub slack_token: Option<String>,

    /// Slack signing secret for webhook verification
    #[arg(long)]
    pub slack_signing_secret: Option<String>,

    /// DingTalk robot webhook URL or access token
    #[arg(long)]
    pub dingtalk_token: Option<String>,

    /// DingTalk robot secret for signing
    #[arg(long)]
    pub dingtalk_secret: Option<String>,

    /// Default agent type to use (recommended: acp; also supports openai, claude, codex, pi) when no channel-specific override is set
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

    /// Agent override for Slack traffic
    #[arg(long)]
    pub slack_agent: Option<String>,

    /// Agent override for DingTalk traffic
    #[arg(long)]
    pub dingtalk_agent: Option<String>,

    /// Optional API key for the built-in OpenAI-compatible HTTP agent backend
    #[arg(long)]
    pub openai_api_key: Option<String>,

    /// Optional base URL for the built-in OpenAI-compatible HTTP agent backend
    #[arg(long)]
    pub openai_base_url: Option<String>,

    /// Optional model name for the built-in OpenAI-compatible HTTP agent backend
    #[arg(long)]
    pub openai_model: Option<String>,

    /// Optional retry count for transient 5xx/network failures from the OpenAI-compatible backend
    #[arg(long)]
    pub openai_max_retries: Option<usize>,

    /// Command used to launch the ACP-compatible agent subprocess; this is the recommended backend
    #[arg(long)]
    pub acp_command: Option<String>,

    /// Extra argument passed to the ACP-compatible agent subprocess; may be repeated
    #[arg(long = "acp-arg")]
    pub acp_args: Vec<String>,

    /// Working directory shared with the ACP-compatible agent subprocess and ACP sessions
    #[arg(long)]
    pub acp_cwd: Option<String>,

    /// Environment variable passed to the ACP-compatible agent subprocess as KEY=VALUE; may be repeated
    #[arg(long = "acp-env")]
    pub acp_env: Vec<String>,

    /// Load runtime options from this JSON file; CLI flags still take precedence
    #[arg(long)]
    pub config_file: Option<String>,

    /// Validate startup configuration and exit before starting the server
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// Persist sessions to this JSON file and reload them on startup
    #[arg(long)]
    pub state_file: Option<String>,

    /// Keep this many rotated backup snapshots alongside the state file
    #[arg(long)]
    pub state_backup_count: Option<usize>,

    /// Trim session history to this many messages after each webhook round-trip
    #[arg(long)]
    pub max_session_messages: Option<usize>,

    /// Send at most this many messages from session history into the agent context window
    #[arg(long)]
    pub context_message_limit: Option<usize>,

    /// Fail a webhook round-trip if the selected agent does not answer within this many milliseconds
    #[arg(long)]
    pub agent_timeout_ms: Option<u64>,

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
