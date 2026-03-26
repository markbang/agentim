use crate::bots::discord::{discord_webhook_handler, DiscordMessage};
use crate::bots::feishu::{feishu_webhook_handler, FeishuMessage};
use crate::bots::qq::{qq_webhook_handler, QQMessage};
use crate::bots::telegram::{telegram_webhook_handler, TelegramUpdate};
use crate::manager::AgentIM;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::post,
    Router,
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct BotServerConfig {
    pub telegram_agent_id: String,
    pub discord_agent_id: String,
    pub feishu_agent_id: String,
    pub qq_agent_id: String,
    pub state_file: Option<String>,
}

impl Default for BotServerConfig {
    fn default() -> Self {
        Self {
            telegram_agent_id: "default-agent".to_string(),
            discord_agent_id: "default-agent".to_string(),
            feishu_agent_id: "default-agent".to_string(),
            qq_agent_id: "default-agent".to_string(),
            state_file: None,
        }
    }
}

#[derive(Clone)]
struct AppState {
    agentim: Arc<AgentIM>,
    config: BotServerConfig,
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

async fn telegram_webhook(
    State(state): State<AppState>,
    Json(update): Json<TelegramUpdate>,
) -> (StatusCode, String) {
    match telegram_webhook_handler(
        state.agentim.clone(),
        state.config.telegram_agent_id.as_str(),
        update,
    )
    .await
    {
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
    Json(message): Json<DiscordMessage>,
) -> (StatusCode, String) {
    match discord_webhook_handler(
        state.agentim.clone(),
        state.config.discord_agent_id.as_str(),
        message,
    )
    .await
    {
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
    Json(message): Json<FeishuMessage>,
) -> (StatusCode, String) {
    match feishu_webhook_handler(
        state.agentim.clone(),
        state.config.feishu_agent_id.as_str(),
        message,
    )
    .await
    {
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
    Json(message): Json<QQMessage>,
) -> (StatusCode, String) {
    match qq_webhook_handler(
        state.agentim.clone(),
        state.config.qq_agent_id.as_str(),
        message,
    )
    .await
    {
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
