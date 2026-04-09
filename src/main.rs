use agentim::agent::{self};
use agentim::bot_server::{self, BotServerConfig, RoutingRule};
use agentim::bots::{
    DingTalkBotChannel, DiscordBotChannel, FeishuBotChannel, QQBotChannel, SlackBotChannel,
    TelegramBotChannel, DINGTALK_CHANNEL_ID, DISCORD_CHANNEL_ID, FEISHU_CHANNEL_ID, QQ_CHANNEL_ID,
    SLACK_CHANNEL_ID, TELEGRAM_CHANNEL_ID,
};
use agentim::channel::Channel;
use agentim::cli::{self, Args};
use agentim::codex::{CodexAgent, CodexBackendConfig};
use agentim::manager::AgentIM;
use clap::Parser;
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
    codex_command: Option<String>,
    #[serde(default)]
    codex_args: Vec<String>,
    codex_cwd: Option<String>,
    #[serde(default)]
    codex_env: HashMap<String, String>,
    #[serde(default)]
    routing_rules: Vec<RuntimeRoutingRuleConfig>,
    telegram_token: Option<String>,
    telegram_webhook_secret_token: Option<String>,
    discord_token: Option<String>,
    discord_interaction_public_key: Option<String>,
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
    session_ttl_seconds: Option<u64>,
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

fn merge_vec(cli: Vec<String>, config: Vec<String>) -> Vec<String> {
    if cli.is_empty() {
        config
    } else {
        cli
    }
}

fn merge_map(
    cli: HashMap<String, String>,
    config: HashMap<String, String>,
) -> HashMap<String, String> {
    if cli.is_empty() {
        config
    } else {
        cli
    }
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
        || Err(anyhow::anyhow!("{flag_name} must be provided as KEY=VALUE")),
        |(key, value)| {
            if key.trim().is_empty() {
                Err(anyhow::anyhow!("{flag_name} requires a non-empty KEY"))
            } else {
                Ok((key.to_string(), value.to_string()))
            }
        },
    )
}

fn parse_env_overrides(
    values: &[String],
    flag_name: &str,
) -> anyhow::Result<HashMap<String, String>> {
    let mut env = HashMap::new();
    for value in values {
        let (key, value) = parse_key_value_assignment(value, flag_name)?;
        env.insert(key, value);
    }
    Ok(env)
}

#[derive(Clone, Default)]
struct AgentRuntimeOptions {
    codex_command: Option<String>,
    codex_args: Vec<String>,
    codex_cwd: Option<PathBuf>,
    codex_env: HashMap<String, String>,
}

fn build_codex_backend_config(options: &AgentRuntimeOptions) -> anyhow::Result<CodexBackendConfig> {
    Ok(CodexBackendConfig {
        command: options
            .codex_command
            .clone()
            .unwrap_or_else(|| "codex".to_string()),
        args: if options.codex_args.is_empty() {
            vec!["app-server".to_string()]
        } else {
            options.codex_args.clone()
        },
        cwd: options
            .codex_cwd
            .clone()
            .unwrap_or(std::env::current_dir()?),
        env: options.codex_env.clone(),
    })
}

fn build_agent(
    id: &str,
    agent_type: &str,
    options: &AgentRuntimeOptions,
) -> anyhow::Result<Arc<dyn agent::Agent>> {
    match agent_type {
        "codex" => Ok(Arc::new(CodexAgent::new(
            id.to_string(),
            build_codex_backend_config(options)?,
        ))),
        other => Err(anyhow::anyhow!(
            "Unknown agent type '{}'. Supported: codex",
            other
        )),
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
        merge_option(args.agent, runtime_config.agent).unwrap_or_else(|| "codex".to_string());
    let telegram_agent = merge_option(args.telegram_agent, runtime_config.telegram_agent);
    let discord_agent = merge_option(args.discord_agent, runtime_config.discord_agent);
    let feishu_agent = merge_option(args.feishu_agent, runtime_config.feishu_agent);
    let qq_agent = merge_option(args.qq_agent, runtime_config.qq_agent);
    let slack_agent = merge_option(args.slack_agent, runtime_config.slack_agent);
    let dingtalk_agent = merge_option(args.dingtalk_agent, runtime_config.dingtalk_agent);
    let codex_command = merge_option(args.codex_command, runtime_config.codex_command);
    let codex_args = merge_vec(args.codex_args, runtime_config.codex_args);
    let codex_cwd = merge_option(args.codex_cwd, runtime_config.codex_cwd);
    let codex_env = merge_map(
        parse_env_overrides(&args.codex_env, "--codex-env")?,
        runtime_config.codex_env,
    );

    let telegram_token = merge_option(args.telegram_token, runtime_config.telegram_token);
    let telegram_webhook_secret_token = merge_option(
        args.telegram_webhook_secret_token,
        runtime_config.telegram_webhook_secret_token,
    );
    let discord_token = merge_option(args.discord_token, runtime_config.discord_token);
    let discord_interaction_public_key = merge_option(
        args.discord_interaction_public_key,
        runtime_config.discord_interaction_public_key,
    );
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
    let agent_timeout_ms = args.agent_timeout_ms.or(runtime_config.agent_timeout_ms);
    let webhook_secret = merge_option(args.webhook_secret, runtime_config.webhook_secret);
    let webhook_signing_secret = merge_option(
        args.webhook_signing_secret,
        runtime_config.webhook_signing_secret,
    );
    let webhook_max_skew_seconds = args
        .webhook_max_skew_seconds
        .or(runtime_config.webhook_max_skew_seconds)
        .unwrap_or(300);
    let session_ttl_seconds = args
        .session_ttl_seconds
        .or(runtime_config.session_ttl_seconds);
    let addr = merge_option(args.addr, runtime_config.addr)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let agent_runtime_options = AgentRuntimeOptions {
        codex_command,
        codex_args,
        codex_cwd: codex_cwd.map(PathBuf::from),
        codex_env,
    };

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
    if default_agent_type == "codex" {
        let backend = build_codex_backend_config(&agent_runtime_options)?;
        cli::print_info(&format!(
            "Codex backend bootstrap: {} (cwd: {})",
            backend.describe(),
            backend.cwd.display()
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

    if let Some(token) = telegram_token {
        cli::print_info("Initializing Telegram Bot...");
        let tg_bot = Arc::new(TelegramBotChannel::new(
            TELEGRAM_CHANNEL_ID.to_string(),
            token,
        ));
        agentim.register_channel(TELEGRAM_CHANNEL_ID.to_string(), tg_bot.clone())?;

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
        let discord_bot = Arc::new(DiscordBotChannel::new(
            DISCORD_CHANNEL_ID.to_string(),
            token,
        ));
        agentim.register_channel(DISCORD_CHANNEL_ID.to_string(), discord_bot.clone())?;

        if args.dry_run {
            cli::print_info("Skipping Discord health check in dry-run mode");
        } else {
            match Channel::health_check(discord_bot.as_ref()).await {
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
        let dingtalk_bot = Arc::new(DingTalkBotChannel::from_token_or_webhook(
            DINGTALK_CHANNEL_ID.to_string(),
            token,
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

    if let Some(ttl) = session_ttl_seconds {
        cli::print_info(&format!("Idle sessions will be cleaned up after {}s", ttl));
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
        session_ttl_seconds,
    };

    bot_server::start_bot_server(Arc::new(agentim), server_config, &addr).await?;

    Ok(())
}
