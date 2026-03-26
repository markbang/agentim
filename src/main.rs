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
use bot_server::BotServerConfig;
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

fn build_agent(id: &str, agent_type: &str) -> anyhow::Result<Arc<dyn agent::Agent>> {
    match agent_type {
        "claude" => Ok(Arc::new(ClaudeAgent::new(id.to_string(), None))),
        "codex" => Ok(Arc::new(CodexAgent::new(id.to_string(), None))),
        "pi" => Ok(Arc::new(PiAgent::new(id.to_string()))),
        other => Err(anyhow::anyhow!("Unknown agent type: {}", other)),
    }
}

fn register_agent_variant(agentim: &AgentIM, id: &str, agent_type: &str) -> anyhow::Result<()> {
    let agent = build_agent(id, agent_type)?;
    agentim.register_agent(id.to_string(), agent)?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let agentim = AgentIM::new();

    register_agent_variant(&agentim, "default-agent", &args.agent)?;
    cli::print_success(&format!("Default agent '{}' registered", args.agent));

    let telegram_agent_id = if let Some(agent_type) = args.telegram_agent.as_deref() {
        register_agent_variant(&agentim, "telegram-agent", agent_type)?;
        cli::print_info(&format!("Telegram traffic -> {} agent", agent_type));
        "telegram-agent".to_string()
    } else {
        "default-agent".to_string()
    };

    let discord_agent_id = if let Some(agent_type) = args.discord_agent.as_deref() {
        register_agent_variant(&agentim, "discord-agent", agent_type)?;
        cli::print_info(&format!("Discord traffic -> {} agent", agent_type));
        "discord-agent".to_string()
    } else {
        "default-agent".to_string()
    };

    let feishu_agent_id = if let Some(agent_type) = args.feishu_agent.as_deref() {
        register_agent_variant(&agentim, "feishu-agent", agent_type)?;
        cli::print_info(&format!("Feishu traffic -> {} agent", agent_type));
        "feishu-agent".to_string()
    } else {
        "default-agent".to_string()
    };

    let qq_agent_id = if let Some(agent_type) = args.qq_agent.as_deref() {
        register_agent_variant(&agentim, "qq-agent", agent_type)?;
        cli::print_info(&format!("QQ traffic -> {} agent", agent_type));
        "qq-agent".to_string()
    } else {
        "default-agent".to_string()
    };

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
        (None, None, Some(compound)) => {
            Some(parse_compound_credentials(&compound, "--feishu-token")?)
        }
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

    let server_config = BotServerConfig {
        telegram_agent_id,
        discord_agent_id,
        feishu_agent_id,
        qq_agent_id,
    };

    bot_server::start_bot_server(Arc::new(agentim), server_config, &args.addr).await?;

    Ok(())
}
