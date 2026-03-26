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
use serde::Deserialize;
use std::{fs, sync::Arc};

#[derive(Debug, Default, Deserialize)]
struct RuntimeConfig {
    agent: Option<String>,
    telegram_agent: Option<String>,
    discord_agent: Option<String>,
    feishu_agent: Option<String>,
    qq_agent: Option<String>,
    telegram_token: Option<String>,
    discord_token: Option<String>,
    feishu_token: Option<String>,
    feishu_app_id: Option<String>,
    feishu_app_secret: Option<String>,
    qq_token: Option<String>,
    qq_bot_id: Option<String>,
    qq_bot_token: Option<String>,
    state_file: Option<String>,
    webhook_secret: Option<String>,
    addr: Option<String>,
}

fn load_runtime_config(path: Option<&str>) -> anyhow::Result<RuntimeConfig> {
    match path {
        Some(path) => {
            let content = fs::read_to_string(path)?;
            Ok(serde_json::from_str(&content)?)
        }
        None => Ok(RuntimeConfig::default()),
    }
}

fn merge_option(cli: Option<String>, config: Option<String>) -> Option<String> {
    cli.or(config)
}

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
    let runtime_config = load_runtime_config(args.config_file.as_deref())?;
    let agentim = AgentIM::new();

    let default_agent_type = merge_option(args.agent, runtime_config.agent)
        .unwrap_or_else(|| "claude".to_string());
    let telegram_agent = merge_option(args.telegram_agent, runtime_config.telegram_agent);
    let discord_agent = merge_option(args.discord_agent, runtime_config.discord_agent);
    let feishu_agent = merge_option(args.feishu_agent, runtime_config.feishu_agent);
    let qq_agent = merge_option(args.qq_agent, runtime_config.qq_agent);

    let telegram_token = merge_option(args.telegram_token, runtime_config.telegram_token);
    let discord_token = merge_option(args.discord_token, runtime_config.discord_token);
    let feishu_token = merge_option(args.feishu_token, runtime_config.feishu_token);
    let feishu_app_id = merge_option(args.feishu_app_id, runtime_config.feishu_app_id);
    let feishu_app_secret = merge_option(args.feishu_app_secret, runtime_config.feishu_app_secret);
    let qq_token = merge_option(args.qq_token, runtime_config.qq_token);
    let qq_bot_id = merge_option(args.qq_bot_id, runtime_config.qq_bot_id);
    let qq_bot_token = merge_option(args.qq_bot_token, runtime_config.qq_bot_token);
    let state_file = merge_option(args.state_file, runtime_config.state_file);
    let webhook_secret = merge_option(args.webhook_secret, runtime_config.webhook_secret);
    let addr = merge_option(args.addr, runtime_config.addr)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    register_agent_variant(&agentim, "default-agent", &default_agent_type)?;
    cli::print_success(&format!("Default agent '{}' registered", default_agent_type));

    let telegram_agent_id = if let Some(agent_type) = telegram_agent.as_deref() {
        register_agent_variant(&agentim, "telegram-agent", agent_type)?;
        cli::print_info(&format!("Telegram traffic -> {} agent", agent_type));
        "telegram-agent".to_string()
    } else {
        "default-agent".to_string()
    };

    let discord_agent_id = if let Some(agent_type) = discord_agent.as_deref() {
        register_agent_variant(&agentim, "discord-agent", agent_type)?;
        cli::print_info(&format!("Discord traffic -> {} agent", agent_type));
        "discord-agent".to_string()
    } else {
        "default-agent".to_string()
    };

    let feishu_agent_id = if let Some(agent_type) = feishu_agent.as_deref() {
        register_agent_variant(&agentim, "feishu-agent", agent_type)?;
        cli::print_info(&format!("Feishu traffic -> {} agent", agent_type));
        "feishu-agent".to_string()
    } else {
        "default-agent".to_string()
    };

    let qq_agent_id = if let Some(agent_type) = qq_agent.as_deref() {
        register_agent_variant(&agentim, "qq-agent", agent_type)?;
        cli::print_info(&format!("QQ traffic -> {} agent", agent_type));
        "qq-agent".to_string()
    } else {
        "default-agent".to_string()
    };

    if let Some(token) = telegram_token {
        cli::print_info("Initializing Telegram Bot...");
        let tg_bot = Arc::new(TelegramBotChannel::new(TELEGRAM_CHANNEL_ID.to_string(), token));
        agentim.register_channel(TELEGRAM_CHANNEL_ID.to_string(), tg_bot.clone())?;

        match Channel::health_check(tg_bot.as_ref()).await {
            Ok(_) => cli::print_success("Telegram Bot connected"),
            Err(e) => cli::print_error(&format!("Telegram Bot connection failed: {}", e)),
        }
    }

    if let Some(token) = discord_token {
        cli::print_info("Initializing Discord Bot...");
        let discord_bot = Arc::new(DiscordBotChannel::new(DISCORD_CHANNEL_ID.to_string(), token));
        agentim.register_channel(DISCORD_CHANNEL_ID.to_string(), discord_bot.clone())?;

        match Channel::health_check(discord_bot.as_ref()).await {
            Ok(_) => cli::print_success("Discord Bot connected"),
            Err(e) => cli::print_error(&format!("Discord Bot connection failed: {}", e)),
        }
    }

    let feishu_credentials = match (feishu_app_id, feishu_app_secret, feishu_token) {
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

    let qq_credentials = match (qq_bot_id, qq_bot_token, qq_token) {
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

    if let Some(path) = state_file.as_deref() {
        let restored = agentim.load_sessions_from_path(path)?;
        cli::print_info(&format!("Restored {} sessions from {}", restored, path));
    }

    if args.dry_run {
        cli::print_success("Dry run complete; startup configuration validated.");
        return Ok(());
    }

    cli::print_info(&format!("Starting Bot server on {}", addr));
    cli::print_info("Waiting for incoming messages...");

    let server_config = BotServerConfig {
        telegram_agent_id,
        discord_agent_id,
        feishu_agent_id,
        qq_agent_id,
        state_file,
        webhook_secret,
    };

    bot_server::start_bot_server(Arc::new(agentim), server_config, &addr).await?;

    Ok(())
}
