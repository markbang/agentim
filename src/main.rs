#![allow(dead_code)]

mod agent;
mod bot_server;
mod bots;
mod channel;
mod cli;
mod config;
mod error;
mod manager;
mod session;

use agent::{ClaudeAgent, CodexAgent, PiAgent};
use bots::{
    DiscordBotChannel, FeishuBotChannel, QQBotChannel, TelegramBotChannel, DISCORD_CHANNEL_ID,
    FEISHU_CHANNEL_ID, QQ_CHANNEL_ID, TELEGRAM_CHANNEL_ID,
};
use channel::Channel;
use clap::Parser;
use cli::Args;
use manager::AgentIM;
use std::sync::Arc;

fn parse_compound_credentials(value: &str, flag_name: &str) -> anyhow::Result<(String, String)> {
    value.split_once(':').map_or_else(
        || {
            Err(anyhow::anyhow!(
                "{} must be provided as <id>:<secret>",
                flag_name
            ))
        },
        |(left, right)| Ok((left.to_string(), right.to_string())),
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let agentim = AgentIM::new();

    // Register default agent
    let agent: Arc<dyn agent::Agent> = match args.agent.as_str() {
        "claude" => Arc::new(ClaudeAgent::new("default-agent".to_string(), None)),
        "codex" => Arc::new(CodexAgent::new("default-agent".to_string(), None)),
        "pi" => Arc::new(PiAgent::new("default-agent".to_string())),
        _ => {
            cli::print_error(&format!("Unknown agent type: {}", args.agent));
            return Ok(());
        }
    };
    agentim.register_agent("default-agent".to_string(), agent)?;
    cli::print_success(&format!("Agent '{}' registered", args.agent));

    if let Some(token) = args.telegram_token {
        cli::print_info("Initializing Telegram Bot...");
        let tg_bot = Arc::new(TelegramBotChannel::new(TELEGRAM_CHANNEL_ID.to_string(), token));
        agentim.register_channel(TELEGRAM_CHANNEL_ID.to_string(), tg_bot.clone())?;

        match Channel::health_check(tg_bot.as_ref()).await {
            Ok(_) => cli::print_success("Telegram Bot connected"),
            Err(e) => cli::print_error(&format!("Telegram Bot connection failed: {}", e)),
        }
    }

    if let Some(token) = args.discord_token {
        cli::print_info("Initializing Discord Bot...");
        let discord_bot = Arc::new(DiscordBotChannel::new(DISCORD_CHANNEL_ID.to_string(), token));
        agentim.register_channel(DISCORD_CHANNEL_ID.to_string(), discord_bot.clone())?;

        match Channel::health_check(discord_bot.as_ref()).await {
            Ok(_) => cli::print_success("Discord Bot connected"),
            Err(e) => cli::print_error(&format!("Discord Bot connection failed: {}", e)),
        }
    }

    let feishu_credentials = match (args.feishu_app_id, args.feishu_app_secret, args.feishu_token) {
        (Some(app_id), Some(app_secret), _) => Some((app_id, app_secret)),
        (None, None, Some(compound)) => Some(parse_compound_credentials(&compound, "--feishu-token")?),
        (Some(_), None, _) | (None, Some(_), _) => {
            cli::print_error("Feishu requires both --feishu-app-id and --feishu-app-secret");
            return Ok(());
        }
        _ => None,
    };

    if let Some((app_id, app_secret)) = feishu_credentials {
        cli::print_info("Initializing Feishu Bot...");
        let feishu_bot = Arc::new(FeishuBotChannel::new(
            FEISHU_CHANNEL_ID.to_string(),
            app_id,
            app_secret,
        ));
        agentim.register_channel(FEISHU_CHANNEL_ID.to_string(), feishu_bot.clone())?;

        match Channel::health_check(feishu_bot.as_ref()).await {
            Ok(_) => cli::print_success("Feishu Bot connected"),
            Err(e) => cli::print_error(&format!("Feishu Bot connection failed: {}", e)),
        }
    }

    let qq_credentials = match (args.qq_bot_id, args.qq_bot_token, args.qq_token) {
        (Some(bot_id), Some(bot_token), _) => Some((bot_id, bot_token)),
        (None, None, Some(compound)) => Some(parse_compound_credentials(&compound, "--qq-token")?),
        (Some(_), None, _) | (None, Some(_), _) => {
            cli::print_error("QQ requires both --qq-bot-id and --qq-bot-token");
            return Ok(());
        }
        _ => None,
    };

    if let Some((bot_id, bot_token)) = qq_credentials {
        cli::print_info("Initializing QQ Bot...");
        let qq_bot = Arc::new(QQBotChannel::new(
            QQ_CHANNEL_ID.to_string(),
            bot_id,
            bot_token,
        ));
        agentim.register_channel(QQ_CHANNEL_ID.to_string(), qq_bot.clone())?;

        match Channel::health_check(qq_bot.as_ref()).await {
            Ok(_) => cli::print_success("QQ Bot connected"),
            Err(e) => cli::print_error(&format!("QQ Bot connection failed: {}", e)),
        }
    }

    cli::print_info(&format!("Starting Bot server on {}", args.addr));
    cli::print_info("Waiting for incoming messages...");

    bot_server::start_bot_server(Arc::new(agentim), &args.addr).await?;

    Ok(())
}
