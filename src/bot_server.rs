use crate::manager::AgentIM;
use axum::{extract::Json, routing::post, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize, Serialize)]
pub struct TelegramUpdate {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TelegramMessage {
    pub message_id: i64,
    pub chat: TelegramChat,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TelegramChat {
    pub id: i64,
}

pub async fn telegram_webhook(Json(update): Json<TelegramUpdate>) -> String {
    if let Some(message) = update.message {
        if let Some(text) = message.text {
            let user_id = message.chat.id.to_string();
            tracing::info!("Received message from {}: {}", user_id, text);
        }
    }
    "ok".to_string()
}

pub fn create_bot_router() -> Router {
    Router::new().route("/telegram", post(telegram_webhook))
}

pub async fn start_bot_server(_agentim: Arc<AgentIM>, addr: &str) -> anyhow::Result<()> {
    let app = create_bot_router();
    let listener = tokio::net::TcpListener::bind(addr).await?;

    println!("🤖 Bot server listening on {}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
