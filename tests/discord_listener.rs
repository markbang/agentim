use agentim::bots::discord::{run_discord_gateway_once, DiscordBotChannel};
use agentim::bots::DISCORD_CHANNEL_ID;
use agentim::channel::{Channel, ChannelMessage};
use agentim::config::ChannelType;
use agentim::manager::{AgentIM, MessageHandlingOptions};
use agentim::Result;
use async_trait::async_trait;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio_tungstenite::{accept_async, tungstenite::Message as WebSocketMessage};

struct CaptureChannel {
    sent: Arc<Mutex<Vec<(String, String)>>>,
}

#[async_trait]
impl Channel for CaptureChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Discord
    }

    fn id(&self) -> &str {
        DISCORD_CHANNEL_ID
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        self.sent
            .lock()
            .unwrap()
            .push((user_id.to_string(), content.to_string()));
        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Default)]
struct DiscordMockState {
    sent_messages: Arc<Mutex<Vec<(String, Value)>>>,
}

async fn discord_mock_api_server(
    state: DiscordMockState,
    gateway_url: String,
) -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route(
            "/gateway/bot",
            get(move || {
                let gateway_url = gateway_url.clone();
                async move { Json(serde_json::json!({ "url": gateway_url })) }
            }),
        )
        .route(
            "/channels/:channel_id/messages",
            post(
                |State(state): State<DiscordMockState>,
                 Path(channel_id): Path<String>,
                 Json(body): Json<Value>| async move {
                    state.sent_messages.lock().unwrap().push((channel_id, body));
                    Json(serde_json::json!({"id": "msg-1"}))
                },
            ),
        )
        .route(
            "/users/@me",
            get(|| async { Json(serde_json::json!({"id": "bot-user"})) }),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (format!("http://{}", addr), handle)
}

async fn discord_mock_gateway_server() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();

        socket
            .send(WebSocketMessage::Text(
                serde_json::json!({
                    "op": 10,
                    "d": { "heartbeat_interval": 50 }
                })
                .to_string(),
            ))
            .await
            .unwrap();

        let _identify = socket.next().await.unwrap().unwrap();

        socket
            .send(WebSocketMessage::Text(
                serde_json::json!({
                    "op": 0,
                    "s": 99,
                    "t": "MESSAGE_CREATE",
                    "d": {
                        "id": "m1",
                        "author": {"id": "user-1", "username": "tester", "bot": false},
                        "content": "hello discord",
                        "channel_id": "discord-room"
                    }
                })
                .to_string(),
            ))
            .await
            .unwrap();

        socket.close(None).await.unwrap();
    });

    (format!("ws://{}", addr), handle)
}

#[tokio::test]
async fn discord_gateway_once_returns_resume_sequence_and_processes_message() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let agentim = Arc::new(AgentIM::new());

    struct EchoAgent;
    #[async_trait]
    impl agentim::agent::Agent for EchoAgent {
        fn agent_type(&self) -> agentim::config::AgentType {
            agentim::config::AgentType::Acp
        }
        fn id(&self) -> &str {
            "default-agent"
        }
        async fn send_message(&self, messages: Vec<agentim::session::Message>) -> Result<String> {
            Ok(format!(
                "echo:{}",
                messages
                    .last()
                    .map(|m| m.content.clone())
                    .unwrap_or_default()
            ))
        }
        async fn health_check(&self) -> Result<()> {
            Ok(())
        }
    }

    agentim
        .register_agent("default-agent".to_string(), Arc::new(EchoAgent))
        .unwrap();
    agentim
        .register_channel(
            DISCORD_CHANNEL_ID.to_string(),
            Arc::new(CaptureChannel { sent: sent.clone() }),
        )
        .unwrap();

    let (gateway_url, gateway_handle) = discord_mock_gateway_server().await;
    let (api_url, api_handle) =
        discord_mock_api_server(DiscordMockState::default(), gateway_url.clone()).await;

    let channel = Arc::new(DiscordBotChannel::with_api_url(
        DISCORD_CHANNEL_ID.to_string(),
        "test-token".to_string(),
        api_url,
    ));

    let sequence = run_discord_gateway_once(
        agentim,
        channel,
        "default-agent",
        MessageHandlingOptions::default(),
        &gateway_url,
        None,
        0,
        None,
    )
    .await
    .unwrap();

    assert_eq!(sequence, Some(99));
    assert_eq!(sent.lock().unwrap().len(), 1);

    gateway_handle.abort();
    api_handle.abort();
}
