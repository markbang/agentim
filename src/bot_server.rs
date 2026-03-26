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

#[derive(Clone)]
struct AppState {
    agentim: Arc<AgentIM>,
}

async fn telegram_webhook(
    State(state): State<AppState>,
    Json(update): Json<TelegramUpdate>,
) -> (StatusCode, String) {
    match telegram_webhook_handler(state.agentim.clone(), update).await {
        Ok(_) => (StatusCode::OK, "ok".to_string()),
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
    match discord_webhook_handler(state.agentim.clone(), message).await {
        Ok(_) => (StatusCode::OK, "ok".to_string()),
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
    match feishu_webhook_handler(state.agentim.clone(), message).await {
        Ok(_) => (StatusCode::OK, "ok".to_string()),
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
    match qq_webhook_handler(state.agentim.clone(), message).await {
        Ok(_) => (StatusCode::OK, "ok".to_string()),
        Err(err) => {
            tracing::error!("qq webhook failed: {}", err);
            (StatusCode::BAD_REQUEST, err.to_string())
        }
    }
}

pub fn create_bot_router(agentim: Arc<AgentIM>) -> Router {
    Router::new()
        .route("/telegram", post(telegram_webhook))
        .route("/discord", post(discord_webhook))
        .route("/feishu", post(feishu_webhook))
        .route("/qq", post(qq_webhook))
        .with_state(AppState { agentim })
}

pub async fn start_bot_server(agentim: Arc<AgentIM>, addr: &str) -> anyhow::Result<()> {
    let app = create_bot_router(agentim);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    println!("🤖 Bot server listening on {}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
