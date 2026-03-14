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
use bots::TelegramBotChannel;
use channel::Channel;
use clap::Parser;
use cli::Args;
use manager::AgentIM;
use std::sync::Arc;

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

    // Register channels based on provided tokens
    if let Some(token) = args.telegram_token {
        cli::print_info("Initializing Telegram Bot...");
        let tg_bot = Arc::new(TelegramBotChannel::new("telegram-bot".to_string(), token));
        agentim.register_channel("telegram-bot".to_string(), tg_bot.clone())?;

        match Channel::health_check(tg_bot.as_ref()).await {
            Ok(_) => cli::print_success("Telegram Bot connected"),
            Err(e) => cli::print_error(&format!("Telegram Bot connection failed: {}", e)),
        }
    }

    if args.discord_token.is_some() {
        cli::print_info("Discord support coming soon");
    }

    if args.feishu_token.is_some() {
        cli::print_info("Feishu support coming soon");
    }

    if args.qq_token.is_some() {
        cli::print_info("QQ support coming soon");
    }

    // Start bot server
    cli::print_info(&format!("Starting Bot server on {}", args.addr));
    cli::print_info("Waiting for incoming messages...");

    bot_server::start_bot_server(Arc::new(agentim), &args.addr).await?;

    Ok(())
}
