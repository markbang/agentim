use crate::bots::discord::{discord_webhook_handler, DiscordMessage};
use crate::bots::feishu::{feishu_webhook_handler, FeishuMessage};
use crate::bots::qq::{qq_webhook_handler, QQMessage};
use crate::bots::telegram::{telegram_webhook_handler, TelegramUpdate};
use crate::manager::AgentIM;
use axum::{
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Router,
};
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize)]
pub struct RoutingRule {
    pub channel: Option<String>,
    pub user_id: Option<String>,
    pub agent_id: String,
}

impl RoutingRule {
    fn matches(&self, channel: &str, user_id: &str) -> bool {
        self.channel.as_deref().map(|value| value == channel).unwrap_or(true)
            && self.user_id.as_deref().map(|value| value == user_id).unwrap_or(true)
    }
}

#[derive(Clone, Debug)]
pub struct BotServerConfig {
    pub telegram_agent_id: String,
    pub discord_agent_id: String,
    pub feishu_agent_id: String,
    pub qq_agent_id: String,
    pub routing_rules: Vec<RoutingRule>,
    pub state_file: Option<String>,
    pub webhook_secret: Option<String>,
}

impl BotServerConfig {
    fn resolve_agent<'a>(&'a self, channel: &str, user_id: &str, fallback: &'a str) -> &'a str {
        self.routing_rules
            .iter()
            .find(|rule| rule.matches(channel, user_id))
            .map(|rule| rule.agent_id.as_str())
            .unwrap_or(fallback)
    }
}

impl Default for BotServerConfig {
    fn default() -> Self {
        Self {
            telegram_agent_id: "default-agent".to_string(),
            discord_agent_id: "default-agent".to_string(),
            feishu_agent_id: "default-agent".to_string(),
            qq_agent_id: "default-agent".to_string(),
            routing_rules: Vec::new(),
            state_file: None,
            webhook_secret: None,
        }
    }
}

#[derive(Clone)]
struct AppState {
    agentim: Arc<AgentIM>,
    config: BotServerConfig,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    agents: usize,
    channels: usize,
    sessions: usize,
}

#[derive(Serialize)]
struct ReviewResponse {
    agents: Vec<String>,
    channels: Vec<String>,
    sessions: usize,
    platform_agents: PlatformAgents,
    routing_rules: Vec<RoutingRule>,
    persistence_enabled: bool,
    webhook_secret_enabled: bool,
}

#[derive(Serialize)]
struct PlatformAgents {
    telegram: String,
    discord: String,
    feishu: String,
    qq: String,
}

fn persist_if_configured(state: &AppState) -> Result<(), String> {
    if let Some(path) = state.config.state_file.as_deref() {
        state
            .agentim
            .save_sessions_to_path(path)
            .map_err(|err| err.to_string())?;
    }

    Ok(())
}

fn authorize(headers: &HeaderMap, state: &AppState) -> Result<(), String> {
    if let Some(expected) = state.config.webhook_secret.as_deref() {
        let provided = headers
            .get("x-agentim-secret")
            .and_then(|value| value.to_str().ok());

        if provided != Some(expected) {
            return Err("missing or invalid x-agentim-secret".to_string());
        }
    }

    Ok(())
}

async fn healthz(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> (StatusCode, Json<HealthResponse>) {
    if authorize(&headers, &state).is_err() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(HealthResponse {
                status: "unauthorized",
                agents: 0,
                channels: 0,
                sessions: 0,
            }),
        );
    }

    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok",
            agents: state.agentim.list_agents().len(),
            channels: state.agentim.list_channels().len(),
            sessions: state.agentim.list_sessions().len(),
        }),
    )
}

async fn reviewz(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> (StatusCode, Json<ReviewResponse>) {
    if authorize(&headers, &state).is_err() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ReviewResponse {
                agents: Vec::new(),
                channels: Vec::new(),
                sessions: 0,
                platform_agents: PlatformAgents {
                    telegram: String::new(),
                    discord: String::new(),
                    feishu: String::new(),
                    qq: String::new(),
                },
                routing_rules: Vec::new(),
                persistence_enabled: false,
                webhook_secret_enabled: true,
            }),
        );
    }

    (
        StatusCode::OK,
        Json(ReviewResponse {
            agents: state.agentim.list_agents(),
            channels: state.agentim.list_channels(),
            sessions: state.agentim.list_sessions().len(),
            platform_agents: PlatformAgents {
                telegram: state.config.telegram_agent_id.clone(),
                discord: state.config.discord_agent_id.clone(),
                feishu: state.config.feishu_agent_id.clone(),
                qq: state.config.qq_agent_id.clone(),
            },
            routing_rules: state.config.routing_rules.clone(),
            persistence_enabled: state.config.state_file.is_some(),
            webhook_secret_enabled: state.config.webhook_secret.is_some(),
        }),
    )
}

async fn telegram_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(update): Json<TelegramUpdate>,
) -> (StatusCode, String) {
    if let Err(err) = authorize(&headers, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }

    let agent_id = update
        .message
        .as_ref()
        .map(|message| {
            let user_id = message.chat.id.to_string();
            state
                .config
                .resolve_agent("telegram", &user_id, state.config.telegram_agent_id.as_str())
                .to_string()
        })
        .unwrap_or_else(|| state.config.telegram_agent_id.clone());

    match telegram_webhook_handler(state.agentim.clone(), &agent_id, update).await {
        Ok(_) => match persist_if_configured(&state) {
            Ok(_) => (StatusCode::OK, "ok".to_string()),
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err),
        },
        Err(err) => {
            tracing::error!("telegram webhook failed: {}", err);
            (StatusCode::BAD_REQUEST, err.to_string())
        }
    }
}

async fn discord_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(message): Json<DiscordMessage>,
) -> (StatusCode, String) {
    if let Err(err) = authorize(&headers, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }

    let agent_id = state
        .config
        .resolve_agent(
            "discord",
            &message.author.id,
            state.config.discord_agent_id.as_str(),
        )
        .to_string();

    match discord_webhook_handler(state.agentim.clone(), &agent_id, message).await {
        Ok(_) => match persist_if_configured(&state) {
            Ok(_) => (StatusCode::OK, "ok".to_string()),
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err),
        },
        Err(err) => {
            tracing::error!("discord webhook failed: {}", err);
            (StatusCode::BAD_REQUEST, err.to_string())
        }
    }
}

async fn feishu_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(message): Json<FeishuMessage>,
) -> (StatusCode, String) {
    if let Err(err) = authorize(&headers, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }

    let agent_id = state
        .config
        .resolve_agent(
            "feishu",
            &message.event.message.sender_id.user_id,
            state.config.feishu_agent_id.as_str(),
        )
        .to_string();

    match feishu_webhook_handler(state.agentim.clone(), &agent_id, message).await {
        Ok(_) => match persist_if_configured(&state) {
            Ok(_) => (StatusCode::OK, "ok".to_string()),
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err),
        },
        Err(err) => {
            tracing::error!("feishu webhook failed: {}", err);
            (StatusCode::BAD_REQUEST, err.to_string())
        }
    }
}

async fn qq_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(message): Json<QQMessage>,
) -> (StatusCode, String) {
    if let Err(err) = authorize(&headers, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }

    let agent_id = state
        .config
        .resolve_agent("qq", &message.author.id, state.config.qq_agent_id.as_str())
        .to_string();

    match qq_webhook_handler(state.agentim.clone(), &agent_id, message).await {
        Ok(_) => match persist_if_configured(&state) {
            Ok(_) => (StatusCode::OK, "ok".to_string()),
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err),
        },
        Err(err) => {
            tracing::error!("qq webhook failed: {}", err);
            (StatusCode::BAD_REQUEST, err.to_string())
        }
    }
}

pub fn create_bot_router(agentim: Arc<AgentIM>) -> Router {
    create_bot_router_with_config(agentim, BotServerConfig::default())
}

pub fn create_bot_router_with_config(agentim: Arc<AgentIM>, config: BotServerConfig) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/reviewz", get(reviewz))
        .route("/telegram", post(telegram_webhook))
        .route("/discord", post(discord_webhook))
        .route("/feishu", post(feishu_webhook))
        .route("/qq", post(qq_webhook))
        .with_state(AppState { agentim, config })
}

pub async fn start_bot_server(
    agentim: Arc<AgentIM>,
    config: BotServerConfig,
    addr: &str,
) -> anyhow::Result<()> {
    let app = create_bot_router_with_config(agentim, config);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    println!("🤖 Bot server listening on {}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
