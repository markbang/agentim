#![allow(dead_code)]

mod acp;
mod agent;
mod bot_server;
mod bots;
mod channel;
mod cli;
mod config;
mod error;
mod manager;
mod session;

use acp::{AcpAgent, AcpBackendConfig};
use agent::{ClaudeAgent, CodexAgent, OpenAiCompatibleAgent, PiAgent};
use bot_server::{BotServerConfig, RoutingRule};
use bots::{
    DingTalkBotChannel, DiscordBotChannel, FeishuBotChannel, QQBotChannel, SlackBotChannel,
    TelegramBotChannel, DINGTALK_CHANNEL_ID, DISCORD_CHANNEL_ID, FEISHU_CHANNEL_ID, QQ_CHANNEL_ID,
    SLACK_CHANNEL_ID, TELEGRAM_CHANNEL_ID,
};
use channel::Channel;
use clap::Parser;
use cli::Args;
use manager::AgentIM;
use serde::Deserialize;
use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};

#[derive(Debug, Clone, Deserialize)]
struct RuntimeRoutingRuleConfig {
    channel: Option<String>,
    user_id: Option<String>,
    user_prefix: Option<String>,
    reply_target: Option<String>,
    reply_target_prefix: Option<String>,
    priority: Option<i32>,
    agent: String,
}

#[derive(Debug, Default, Deserialize)]
struct RuntimeConfig {
    agent: Option<String>,
    telegram_agent: Option<String>,
    discord_agent: Option<String>,
    feishu_agent: Option<String>,
    qq_agent: Option<String>,
    slack_agent: Option<String>,
    dingtalk_agent: Option<String>,
    openai_api_key: Option<String>,
    openai_base_url: Option<String>,
    openai_model: Option<String>,
    openai_max_retries: Option<usize>,
    acp_command: Option<String>,
    #[serde(default)]
    acp_args: Vec<String>,
    acp_cwd: Option<String>,
    #[serde(default)]
    acp_env: HashMap<String, String>,
    #[serde(default)]
    routing_rules: Vec<RuntimeRoutingRuleConfig>,
    telegram_token: Option<String>,
    telegram_webhook_secret_token: Option<String>,
    telegram_poll: Option<bool>,
    discord_token: Option<String>,
    discord_interaction_public_key: Option<String>,
    discord_gateway: Option<bool>,
    feishu_token: Option<String>,
    feishu_app_id: Option<String>,
    feishu_app_secret: Option<String>,
    feishu_verification_token: Option<String>,
    slack_token: Option<String>,
    slack_signing_secret: Option<String>,
    dingtalk_token: Option<String>,
    dingtalk_secret: Option<String>,
    qq_token: Option<String>,
    qq_bot_id: Option<String>,
    qq_bot_token: Option<String>,
    state_file: Option<String>,
    state_backup_count: Option<usize>,
    max_session_messages: Option<usize>,
    context_message_limit: Option<usize>,
    agent_timeout_ms: Option<u64>,
    webhook_secret: Option<String>,
    webhook_signing_secret: Option<String>,
    webhook_max_skew_seconds: Option<i64>,
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

fn parse_key_value_assignment(value: &str, flag_name: &str) -> anyhow::Result<(String, String)> {
    value.split_once('=').map_or_else(
        || {
            Err(anyhow::anyhow!(
                "{} must be provided as KEY=VALUE",
                flag_name
            ))
        },
        |(key, value)| Ok((key.to_string(), value.to_string())),
    )
}

fn parse_env_assignments(
    values: Vec<String>,
    flag_name: &str,
) -> anyhow::Result<HashMap<String, String>> {
    values
        .into_iter()
        .map(|value| parse_key_value_assignment(&value, flag_name))
        .collect()
}

fn resolve_absolute_path(path: Option<String>) -> anyhow::Result<PathBuf> {
    let path = path.map(PathBuf::from).unwrap_or(std::env::current_dir()?);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn is_stub_agent_type(agent_type: &str) -> bool {
    matches!(agent_type, "claude" | "codex" | "pi")
}

#[allow(clippy::too_many_arguments)]
fn validate_production_runtime(
    configured_agent_types: &[String],
    telegram_enabled: bool,
    telegram_protected: bool,
    discord_enabled: bool,
    discord_protected: bool,
    feishu_enabled: bool,
    feishu_protected: bool,
    qq_enabled: bool,
    qq_protected: bool,
    slack_enabled: bool,
    slack_protected: bool,
    dingtalk_enabled: bool,
    dingtalk_protected: bool,
    webhook_signing_secret_enabled: bool,
    webhook_max_skew_seconds: i64,
) -> anyhow::Result<()> {
    let stub_agents = configured_agent_types
        .iter()
        .filter(|agent_type| is_stub_agent_type(agent_type))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if !stub_agents.is_empty() {
        return Err(anyhow::anyhow!(
            "production bot server cannot use stub agents ({}) ; use 'openai' or 'acp' instead",
            stub_agents.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    if webhook_signing_secret_enabled && webhook_max_skew_seconds <= 0 {
        return Err(anyhow::anyhow!(
            "--webhook-max-skew-seconds must be greater than 0 when signed webhook verification is enabled"
        ));
    }

    for (channel, enabled, protected) in [
        ("Telegram", telegram_enabled, telegram_protected),
        ("Discord", discord_enabled, discord_protected),
        ("Feishu", feishu_enabled, feishu_protected),
        ("QQ", qq_enabled, qq_protected),
        ("Slack", slack_enabled, slack_protected),
        ("DingTalk", dingtalk_enabled, dingtalk_protected),
    ] {
        if enabled && !protected {
            return Err(anyhow::anyhow!(
                "{} webhook ingress is enabled without request authentication",
                channel
            ));
        }
    }

    Ok(())
}

#[derive(Clone, Default)]
struct AgentRuntimeOptions {
    openai_api_key: Option<String>,
    openai_base_url: Option<String>,
    openai_model: Option<String>,
    openai_max_retries: Option<usize>,
    acp_command: Option<String>,
    acp_args: Vec<String>,
    acp_cwd: Option<String>,
    acp_env: HashMap<String, String>,
}

fn build_agent(
    id: &str,
    agent_type: &str,
    options: &AgentRuntimeOptions,
) -> anyhow::Result<Arc<dyn agent::Agent>> {
    match agent_type {
        "claude" => Ok(Arc::new(ClaudeAgent::new(id.to_string(), None))),
        "codex" => Ok(Arc::new(CodexAgent::new(id.to_string(), None))),
        "pi" => Ok(Arc::new(PiAgent::new(id.to_string()))),
        "openai" => {
            let api_key = options
                .openai_api_key
                .clone()
                .ok_or_else(|| anyhow::anyhow!("openai agent requires --openai-api-key"))?;
            let base_url = options
                .openai_base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            let model = options
                .openai_model
                .clone()
                .unwrap_or_else(|| "gpt-4o-mini".to_string());
            Ok(Arc::new(
                OpenAiCompatibleAgent::new(id.to_string(), api_key, base_url, model)
                    .with_max_retries(options.openai_max_retries.unwrap_or(0)),
            ))
        }
        "acp" => {
            let command = options
                .acp_command
                .clone()
                .ok_or_else(|| anyhow::anyhow!("acp agent requires --acp-command"))?;
            let cwd = resolve_absolute_path(options.acp_cwd.clone())?;
            Ok(Arc::new(AcpAgent::new(
                id.to_string(),
                AcpBackendConfig {
                    command,
                    args: options.acp_args.clone(),
                    cwd,
                    env: options.acp_env.clone(),
                },
            )))
        }
        other => Err(anyhow::anyhow!("Unknown agent type: {}", other)),
    }
}

fn register_agent_variant(
    agentim: &AgentIM,
    id: &str,
    agent_type: &str,
    options: &AgentRuntimeOptions,
) -> anyhow::Result<()> {
    let agent = build_agent(id, agent_type, options)?;
    agentim.register_agent(id.to_string(), agent)?;
    Ok(())
}

fn ensure_rule_agent(
    agentim: &AgentIM,
    registered_rule_agents: &mut HashMap<String, String>,
    agent_type: &str,
    options: &AgentRuntimeOptions,
) -> anyhow::Result<String> {
    if let Some(agent_id) = registered_rule_agents.get(agent_type) {
        return Ok(agent_id.clone());
    }

    let agent_id = format!("rule-agent-{}", agent_type);
    register_agent_variant(agentim, &agent_id, agent_type, options)?;
    registered_rule_agents.insert(agent_type.to_string(), agent_id.clone());
    Ok(agent_id)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let runtime_config = load_runtime_config(args.config_file.as_deref())?;
    let agentim = AgentIM::new();

    let default_agent_type =
        merge_option(args.agent, runtime_config.agent).unwrap_or_else(|| "claude".to_string());
    let telegram_agent = merge_option(args.telegram_agent, runtime_config.telegram_agent);
    let discord_agent = merge_option(args.discord_agent, runtime_config.discord_agent);
    let feishu_agent = merge_option(args.feishu_agent, runtime_config.feishu_agent);
    let qq_agent = merge_option(args.qq_agent, runtime_config.qq_agent);
    let slack_agent = merge_option(args.slack_agent, runtime_config.slack_agent);
    let dingtalk_agent = merge_option(args.dingtalk_agent, runtime_config.dingtalk_agent);

    let telegram_token = merge_option(args.telegram_token, runtime_config.telegram_token);
    let telegram_webhook_secret_token = merge_option(
        args.telegram_webhook_secret_token,
        runtime_config.telegram_webhook_secret_token,
    );
    let telegram_poll = args.telegram_poll || runtime_config.telegram_poll.unwrap_or(false);
    let openai_api_key = merge_option(args.openai_api_key, runtime_config.openai_api_key);
    let openai_base_url = merge_option(args.openai_base_url, runtime_config.openai_base_url);
    let openai_model = merge_option(args.openai_model, runtime_config.openai_model);
    let openai_max_retries = args
        .openai_max_retries
        .or(runtime_config.openai_max_retries);
    let acp_command = merge_option(args.acp_command, runtime_config.acp_command);
    let acp_args = if args.acp_args.is_empty() {
        runtime_config.acp_args
    } else {
        args.acp_args
    };
    let acp_cwd = merge_option(args.acp_cwd, runtime_config.acp_cwd);
    let acp_env = if args.acp_env.is_empty() {
        runtime_config.acp_env
    } else {
        parse_env_assignments(args.acp_env, "--acp-env")?
    };
    let discord_token = merge_option(args.discord_token, runtime_config.discord_token);
    let discord_interaction_public_key = merge_option(
        args.discord_interaction_public_key,
        runtime_config.discord_interaction_public_key,
    );
    let discord_gateway = args.discord_gateway || runtime_config.discord_gateway.unwrap_or(false);
    let feishu_token = merge_option(args.feishu_token, runtime_config.feishu_token);
    let feishu_app_id = merge_option(args.feishu_app_id, runtime_config.feishu_app_id);
    let feishu_app_secret = merge_option(args.feishu_app_secret, runtime_config.feishu_app_secret);
    let feishu_verification_token = merge_option(
        args.feishu_verification_token,
        runtime_config.feishu_verification_token,
    );
    let slack_token = merge_option(args.slack_token, runtime_config.slack_token);
    let slack_signing_secret = merge_option(
        args.slack_signing_secret,
        runtime_config.slack_signing_secret,
    );
    let dingtalk_token = merge_option(args.dingtalk_token, runtime_config.dingtalk_token);
    let dingtalk_secret = merge_option(args.dingtalk_secret, runtime_config.dingtalk_secret);
    let qq_token = merge_option(args.qq_token, runtime_config.qq_token);
    let qq_bot_id = merge_option(args.qq_bot_id, runtime_config.qq_bot_id);
    let qq_bot_token = merge_option(args.qq_bot_token, runtime_config.qq_bot_token);
    let state_file = merge_option(args.state_file, runtime_config.state_file);
    let state_backup_count = args
        .state_backup_count
        .or(runtime_config.state_backup_count)
        .unwrap_or(0);
    let max_session_messages = args
        .max_session_messages
        .or(runtime_config.max_session_messages);
    let context_message_limit = args
        .context_message_limit
        .or(runtime_config.context_message_limit)
        .unwrap_or(10);
    let agent_timeout_ms = Some(
        args.agent_timeout_ms
            .or(runtime_config.agent_timeout_ms)
            .unwrap_or(30_000),
    );
    let webhook_secret = merge_option(args.webhook_secret, runtime_config.webhook_secret);
    let webhook_signing_secret = merge_option(
        args.webhook_signing_secret,
        runtime_config.webhook_signing_secret,
    );
    let webhook_max_skew_seconds = args
        .webhook_max_skew_seconds
        .or(runtime_config.webhook_max_skew_seconds)
        .unwrap_or(300);
    let addr = merge_option(args.addr, runtime_config.addr)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let agent_runtime_options = AgentRuntimeOptions {
        openai_api_key,
        openai_base_url,
        openai_model,
        openai_max_retries,
        acp_command,
        acp_args,
        acp_cwd,
        acp_env,
    };

    let mut configured_agent_types = vec![default_agent_type.clone()];
    configured_agent_types.extend(
        [
            telegram_agent.as_ref(),
            discord_agent.as_ref(),
            feishu_agent.as_ref(),
            qq_agent.as_ref(),
            slack_agent.as_ref(),
            dingtalk_agent.as_ref(),
        ]
        .into_iter()
        .flatten()
        .cloned(),
    );
    configured_agent_types.extend(
        runtime_config
            .routing_rules
            .iter()
            .map(|rule| rule.agent.clone()),
    );

    register_agent_variant(
        &agentim,
        "default-agent",
        &default_agent_type,
        &agent_runtime_options,
    )?;
    cli::print_success(&format!(
        "Default agent '{}' registered",
        default_agent_type
    ));
    if let Some(openai_max_retries) = agent_runtime_options.openai_max_retries {
        cli::print_info(&format!(
            "OpenAI-compatible backend retries set to {}",
            openai_max_retries
        ));
    }

    let telegram_agent_id = if let Some(agent_type) = telegram_agent.as_deref() {
        register_agent_variant(
            &agentim,
            "telegram-agent",
            agent_type,
            &agent_runtime_options,
        )?;
        cli::print_info(&format!("Telegram traffic -> {} agent", agent_type));
        "telegram-agent".to_string()
    } else {
        "default-agent".to_string()
    };

    let discord_agent_id = if let Some(agent_type) = discord_agent.as_deref() {
        register_agent_variant(
            &agentim,
            "discord-agent",
            agent_type,
            &agent_runtime_options,
        )?;
        cli::print_info(&format!("Discord traffic -> {} agent", agent_type));
        "discord-agent".to_string()
    } else {
        "default-agent".to_string()
    };

    let feishu_agent_id = if let Some(agent_type) = feishu_agent.as_deref() {
        register_agent_variant(&agentim, "feishu-agent", agent_type, &agent_runtime_options)?;
        cli::print_info(&format!("Feishu traffic -> {} agent", agent_type));
        "feishu-agent".to_string()
    } else {
        "default-agent".to_string()
    };

    let qq_agent_id = if let Some(agent_type) = qq_agent.as_deref() {
        register_agent_variant(&agentim, "qq-agent", agent_type, &agent_runtime_options)?;
        cli::print_info(&format!("QQ traffic -> {} agent", agent_type));
        "qq-agent".to_string()
    } else {
        "default-agent".to_string()
    };

    let slack_agent_id = if let Some(agent_type) = slack_agent.as_deref() {
        register_agent_variant(&agentim, "slack-agent", agent_type, &agent_runtime_options)?;
        cli::print_info(&format!("Slack traffic -> {} agent", agent_type));
        "slack-agent".to_string()
    } else {
        "default-agent".to_string()
    };

    let dingtalk_agent_id = if let Some(agent_type) = dingtalk_agent.as_deref() {
        register_agent_variant(
            &agentim,
            "dingtalk-agent",
            agent_type,
            &agent_runtime_options,
        )?;
        cli::print_info(&format!("DingTalk traffic -> {} agent", agent_type));
        "dingtalk-agent".to_string()
    } else {
        "default-agent".to_string()
    };

    let mut registered_rule_agents = HashMap::new();
    let routing_rules = runtime_config
        .routing_rules
        .into_iter()
        .map(|rule| {
            let agent_id = ensure_rule_agent(
                &agentim,
                &mut registered_rule_agents,
                &rule.agent,
                &agent_runtime_options,
            )?;
            Ok(RoutingRule {
                channel: rule.channel,
                user_id: rule.user_id,
                user_prefix: rule.user_prefix,
                reply_target: rule.reply_target,
                reply_target_prefix: rule.reply_target_prefix,
                priority: rule.priority.unwrap_or(0),
                agent_id,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    if !routing_rules.is_empty() {
        cli::print_info(&format!("Loaded {} routing rule(s)", routing_rules.len()));
    }

    let telegram_enabled = telegram_token.is_some();
    let discord_enabled = discord_token.is_some();
    let feishu_enabled =
        feishu_app_id.is_some() || feishu_app_secret.is_some() || feishu_token.is_some();
    let qq_enabled = qq_bot_id.is_some() || qq_bot_token.is_some() || qq_token.is_some();
    let slack_enabled = slack_token.is_some();
    let dingtalk_enabled = dingtalk_token.is_some();
    let mut telegram_bot: Option<Arc<TelegramBotChannel>> = None;
    let mut discord_bot: Option<Arc<DiscordBotChannel>> = None;

    if let Some(token) = telegram_token {
        cli::print_info("Initializing Telegram Bot...");
        let tg_bot = Arc::new(TelegramBotChannel::new(
            TELEGRAM_CHANNEL_ID.to_string(),
            token,
        ));
        agentim.register_channel(TELEGRAM_CHANNEL_ID.to_string(), tg_bot.clone())?;
        telegram_bot = Some(tg_bot.clone());

        if args.dry_run {
            cli::print_info("Skipping Telegram health check in dry-run mode");
        } else {
            match Channel::health_check(tg_bot.as_ref()).await {
                Ok(_) => cli::print_success("Telegram Bot connected"),
                Err(e) => cli::print_error(&format!("Telegram Bot connection failed: {}", e)),
            }
        }
    }

    if let Some(token) = discord_token {
        cli::print_info("Initializing Discord Bot...");
        let discord_channel = Arc::new(DiscordBotChannel::new(
            DISCORD_CHANNEL_ID.to_string(),
            token,
        ));
        agentim.register_channel(DISCORD_CHANNEL_ID.to_string(), discord_channel.clone())?;
        discord_bot = Some(discord_channel.clone());

        if args.dry_run {
            cli::print_info("Skipping Discord health check in dry-run mode");
        } else {
            match Channel::health_check(discord_channel.as_ref()).await {
                Ok(_) => cli::print_success("Discord Bot connected"),
                Err(e) => cli::print_error(&format!("Discord Bot connection failed: {}", e)),
            }
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

        if args.dry_run {
            cli::print_info("Skipping Feishu health check in dry-run mode");
        } else {
            match Channel::health_check(feishu_bot.as_ref()).await {
                Ok(_) => cli::print_success("Feishu Bot connected"),
                Err(e) => cli::print_error(&format!("Feishu Bot connection failed: {}", e)),
            }
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

        if args.dry_run {
            cli::print_info("Skipping QQ health check in dry-run mode");
        } else {
            match Channel::health_check(qq_bot.as_ref()).await {
                Ok(_) => cli::print_success("QQ Bot connected"),
                Err(e) => cli::print_error(&format!("QQ Bot connection failed: {}", e)),
            }
        }
    }

    if let Some(token) = slack_token {
        cli::print_info("Initializing Slack Bot...");
        let slack_bot = Arc::new(SlackBotChannel::new(
            SLACK_CHANNEL_ID.to_string(),
            token,
            slack_signing_secret.clone(),
        ));
        agentim.register_channel(SLACK_CHANNEL_ID.to_string(), slack_bot.clone())?;

        if args.dry_run {
            cli::print_info("Skipping Slack health check in dry-run mode");
        } else {
            match Channel::health_check(slack_bot.as_ref()).await {
                Ok(_) => cli::print_success("Slack Bot connected"),
                Err(e) => cli::print_error(&format!("Slack Bot connection failed: {}", e)),
            }
        }
    }

    if let Some(token) = dingtalk_token {
        cli::print_info("Initializing DingTalk Bot...");
        let dingtalk_bot = Arc::new(DingTalkBotChannel::new(
            DINGTALK_CHANNEL_ID.to_string(),
            Some(token),
            dingtalk_secret.clone(),
        ));
        agentim.register_channel(DINGTALK_CHANNEL_ID.to_string(), dingtalk_bot.clone())?;

        if args.dry_run {
            cli::print_info("Skipping DingTalk health check in dry-run mode");
        } else {
            match Channel::health_check(dingtalk_bot.as_ref()).await {
                Ok(_) => cli::print_success("DingTalk Bot connected"),
                Err(e) => cli::print_error(&format!("DingTalk Bot connection failed: {}", e)),
            }
        }
    }

    if let Some(path) = state_file.as_deref() {
        let (restored, loaded_from) = if state_backup_count > 0 {
            agentim.load_sessions_from_path_with_fallback(path, state_backup_count)?
        } else {
            (agentim.load_sessions_from_path(path)?, path.to_string())
        };
        cli::print_info(&format!(
            "Restored {} sessions from {}",
            restored, loaded_from
        ));
        if state_backup_count > 0 {
            cli::print_info(&format!(
                "State snapshot rotation enabled ({} backup file(s))",
                state_backup_count
            ));
        }
    }

    if let Some(max_session_messages) = max_session_messages {
        cli::print_info(&format!(
            "Session history will be trimmed to {} message(s)",
            max_session_messages
        ));
    }
    cli::print_info(&format!(
        "Agent context window limited to {} message(s)",
        context_message_limit
    ));
    if let Some(agent_timeout_ms) = agent_timeout_ms {
        cli::print_info(&format!(
            "Agent requests will time out after {}ms",
            agent_timeout_ms
        ));
    }

    if webhook_signing_secret.is_some() {
        cli::print_info(&format!(
            "Signed webhook verification enabled (max skew: {}s)",
            webhook_max_skew_seconds
        ));
    }

    if telegram_webhook_secret_token.is_some() {
        cli::print_info("Telegram native webhook secret token enabled");
    }
    if discord_interaction_public_key.is_some() {
        cli::print_info("Discord interaction signature verification enabled");
    }
    if feishu_verification_token.is_some() {
        cli::print_info("Feishu webhook verification token enabled");
    }
    if slack_signing_secret.is_some() {
        cli::print_info("Slack webhook signature verification enabled");
    }
    if dingtalk_secret.is_some() {
        cli::print_info("DingTalk webhook signature enabled");
    }

    if telegram_poll {
        cli::print_info("Telegram long polling enabled");
    }
    if discord_gateway {
        cli::print_info("Discord gateway mode enabled");
    }

    let start_webhook_server = (!telegram_poll && telegram_enabled)
        || (!discord_gateway && discord_enabled)
        || feishu_enabled
        || qq_enabled
        || slack_enabled
        || dingtalk_enabled;

    if !args.dry_run && start_webhook_server {
        let shared_webhook_protection_enabled =
            webhook_secret.is_some() || webhook_signing_secret.is_some();
        validate_production_runtime(
            &configured_agent_types,
            !telegram_poll && telegram_enabled,
            shared_webhook_protection_enabled || telegram_webhook_secret_token.is_some(),
            !discord_gateway && discord_enabled,
            shared_webhook_protection_enabled || discord_interaction_public_key.is_some(),
            feishu_enabled,
            shared_webhook_protection_enabled || feishu_verification_token.is_some(),
            qq_enabled,
            shared_webhook_protection_enabled,
            slack_enabled,
            shared_webhook_protection_enabled || slack_signing_secret.is_some(),
            dingtalk_enabled,
            shared_webhook_protection_enabled,
            webhook_signing_secret.is_some(),
            webhook_max_skew_seconds,
        )?;
    }

    if args.dry_run {
        cli::print_success("Dry run complete; startup configuration validated.");
        return Ok(());
    }

    let agentim = Arc::new(agentim);

    let server_config = BotServerConfig {
        telegram_agent_id,
        discord_agent_id,
        feishu_agent_id,
        qq_agent_id,
        slack_agent_id,
        dingtalk_agent_id,
        routing_rules,
        max_session_messages,
        context_message_limit,
        agent_timeout_ms,
        state_file,
        state_backup_count,
        webhook_secret,
        webhook_signing_secret,
        webhook_max_skew_seconds,
        telegram_webhook_secret_token,
        discord_interaction_public_key,
        feishu_verification_token,
        slack_signing_secret,
        dingtalk_secret,
    };

    if telegram_poll && !telegram_enabled {
        return Err(anyhow::anyhow!(
            "--telegram-poll requires --telegram-token or telegram_token in the config file"
        ));
    }
    if discord_gateway && !discord_enabled {
        return Err(anyhow::anyhow!(
            "--discord-gateway requires --discord-token or discord_token in the config file"
        ));
    }

    let mut ingress_tasks = tokio::task::JoinSet::new();
    let mut has_background_ingress = false;

    if telegram_poll {
        let telegram_bot = telegram_bot.clone().ok_or_else(|| {
            anyhow::anyhow!("Telegram long polling requested but bot not initialized")
        })?;
        let agentim = agentim.clone();
        let telegram_agent_id = server_config.telegram_agent_id.clone();
        let state_file = server_config.state_file.clone();
        ingress_tasks.spawn(async move {
            cli::print_info("Starting Telegram long polling");
            bots::telegram::start_telegram_long_polling(
                agentim,
                telegram_bot,
                telegram_agent_id,
                manager::MessageHandlingOptions {
                    max_messages: max_session_messages,
                    context_message_limit,
                    agent_timeout_ms,
                },
                state_file,
                state_backup_count,
            )
            .await
            .map_err(anyhow::Error::from)
        });
        has_background_ingress = true;
    }

    if discord_gateway {
        let discord_bot = discord_bot
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Discord gateway requested but bot not initialized"))?;
        let agentim = agentim.clone();
        let discord_agent_id = server_config.discord_agent_id.clone();
        let state_file = server_config.state_file.clone();
        ingress_tasks.spawn(async move {
            cli::print_info("Starting Discord gateway");
            bots::discord::start_discord_gateway(
                agentim,
                discord_bot,
                discord_agent_id,
                manager::MessageHandlingOptions {
                    max_messages: max_session_messages,
                    context_message_limit,
                    agent_timeout_ms,
                },
                state_file,
                state_backup_count,
            )
            .await
            .map_err(anyhow::Error::from)
        });
        has_background_ingress = true;
    }

    match (start_webhook_server, has_background_ingress) {
        (true, true) => {
            cli::print_info(&format!("Starting Bot server on {}", addr));
            cli::print_info("Waiting for webhook, polling, and gateway messages...");
            tokio::select! {
                result = bot_server::start_bot_server(agentim, server_config, &addr) => result?,
                task = ingress_tasks.join_next() => match task {
                    Some(Ok(Ok(()))) => {}
                    Some(Ok(Err(err))) => return Err(err),
                    Some(Err(err)) => return Err(err.into()),
                    None => {}
                }
            }
        }
        (true, false) => {
            cli::print_info(&format!("Starting Bot server on {}", addr));
            cli::print_info("Waiting for incoming messages...");
            bot_server::start_bot_server(agentim, server_config, &addr).await?;
        }
        (false, true) => {
            cli::print_info("Waiting for polling and gateway messages...");
            match ingress_tasks.join_next().await {
                Some(Ok(Ok(()))) => {}
                Some(Ok(Err(err))) => return Err(err),
                Some(Err(err)) => return Err(err.into()),
                None => {}
            }
        }
        (false, false) => {
            return Err(anyhow::anyhow!(
                "no runtime ingress configured; enable at least one channel, --telegram-poll, or --discord-gateway"
            ));
        }
    }

    Ok(())
}
