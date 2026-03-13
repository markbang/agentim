use crate::bots::TelegramBotChannel;
use crate::manager::AgentIM;
use axum::{
    extract::Json,
    routing::post,
    Router,
};
use serde_json::Value;
use std::sync::Arc;

pub struct BotServerState {
    pub agentim: Arc<AgentIM>,
    pub telegram_channel: Option<Arc<TelegramBotChannel>>,
}

pub async fn telegram_webhook(
    Json(_update): Json<Value>,
) -> String {
    "ok".to_string()
}

pub fn create_bot_router() -> Router {
    Router::new()
        .route("/telegram", post(telegram_webhook))
}

pub async fn start_bot_server(
    _state: BotServerState,
    addr: &str,
) -> anyhow::Result<()> {
    let app = create_bot_router();
    let listener = tokio::net::TcpListener::bind(addr).await?;

    println!("🤖 Bot server listening on {}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
