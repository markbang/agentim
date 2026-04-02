use crate::channel::{Channel, ChannelMessage};
use crate::config::ChannelType;
use crate::error::{AgentError, Result};
use crate::manager::{AgentIM, MessageHandlingOptions};
use async_trait::async_trait;
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::time::{sleep, Duration, MissedTickBehavior};
use tokio_tungstenite::{connect_async, tungstenite::Message as WebSocketMessage};

pub const DISCORD_CHANNEL_ID: &str = "discord-bot";

const DISCORD_GATEWAY_INTENTS: u64 = (1 << 9) | (1 << 12) | (1 << 15);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordMessage {
    pub id: String,
    pub author: DiscordUser,
    pub content: String,
    pub channel_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub bot: bool,
}

#[derive(Debug, Deserialize)]
struct DiscordGatewayEnvelope {
    op: u64,
    #[serde(default)]
    d: serde_json::Value,
    #[serde(default)]
    s: Option<u64>,
    #[serde(default)]
    t: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiscordGatewayHello {
    heartbeat_interval: u64,
}

#[derive(Debug, Deserialize)]
struct DiscordGatewayBotResponse {
    url: String,
}

fn normalize_gateway_url(raw_url: &str) -> Result<String> {
    let mut url = reqwest::Url::parse(raw_url)
        .map_err(|e| AgentError::ChannelError(format!("Invalid Discord gateway URL: {}", e)))?;

    if url.path().is_empty() {
        url.set_path("/");
    }

    let existing_pairs = url
        .query_pairs()
        .into_owned()
        .filter(|(key, _)| key != "v" && key != "encoding")
        .collect::<Vec<_>>();

    {
        let mut query = url.query_pairs_mut();
        query.clear();
        for (key, value) in existing_pairs {
            query.append_pair(&key, &value);
        }
        query.append_pair("v", "10");
        query.append_pair("encoding", "json");
    }

    Ok(url.to_string())
}

pub struct DiscordBotChannel {
    id: String,
    token: String,
    api_url: String,
    pending_messages: Arc<DashMap<String, Vec<String>>>,
}

impl DiscordBotChannel {
    pub fn new(id: String, token: String) -> Self {
        let api_url = "https://discord.com/api/v10".to_string();
        Self {
            id,
            token,
            api_url,
            pending_messages: Arc::new(DashMap::new()),
        }
    }

    pub async fn get_gateway_url(&self) -> Result<String> {
        let client = reqwest::Client::new();
        let url = format!("{}/gateway/bot", self.api_url);
        let response = client
            .get(&url)
            .header("Authorization", format!("Bot {}", self.token))
            .send()
            .await
            .map_err(|e| AgentError::ChannelError(e.to_string()))?
            .error_for_status()
            .map_err(|e| AgentError::ChannelError(e.to_string()))?;

        let body: DiscordGatewayBotResponse = response
            .json()
            .await
            .map_err(|e| AgentError::ChannelError(e.to_string()))?;
        normalize_gateway_url(&body.url)
    }

    pub fn get_pending_messages(&self, user_id: &str) -> Vec<String> {
        self.pending_messages
            .remove(user_id)
            .map(|(_, msgs)| msgs)
            .unwrap_or_default()
    }

    pub fn add_pending_message(&self, user_id: String, message: String) {
        self.pending_messages
            .entry(user_id)
            .or_default()
            .push(message);
    }
}

#[async_trait]
impl Channel for DiscordBotChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Discord
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let url = format!("{}/channels/{}/messages", self.api_url, user_id);

        let params = serde_json::json!({
            "content": content
        });

        client
            .post(&url)
            .header("Authorization", format!("Bot {}", self.token))
            .json(&params)
            .send()
            .await
            .map_err(|e| AgentError::ChannelError(e.to_string()))?
            .error_for_status()
            .map_err(|e| AgentError::ChannelError(e.to_string()))?;

        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        let client = reqwest::Client::new();
        let url = format!("{}/users/@me", self.api_url);

        client
            .get(&url)
            .header("Authorization", format!("Bot {}", self.token))
            .send()
            .await
            .map_err(|e| AgentError::ChannelError(format!("Discord health check failed: {}", e)))?
            .error_for_status()
            .map_err(|e| AgentError::ChannelError(format!("Discord health check failed: {}", e)))?;

        Ok(())
    }
}

pub async fn discord_webhook_handler(
    agentim: Arc<AgentIM>,
    agent_id: &str,
    max_session_messages: Option<usize>,
    context_message_limit: usize,
    agent_timeout_ms: Option<u64>,
    message: DiscordMessage,
) -> Result<()> {
    if message.author.bot || message.content.is_empty() {
        return Ok(());
    }

    let user_id = message.author.id;
    let reply_target = message.channel_id;
    let content = message.content;

    agentim
        .handle_incoming_message_with_options(
            agent_id,
            DISCORD_CHANNEL_ID,
            &user_id,
            Some(&reply_target),
            content,
            MessageHandlingOptions {
                max_messages: max_session_messages,
                context_message_limit,
                agent_timeout_ms,
            },
        )
        .await?;

    Ok(())
}

async fn persist_sessions(agentim: Arc<AgentIM>, path: String, backup_count: usize) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        agentim.save_sessions_to_path_with_rotation(&path, backup_count)
    })
    .await
    .map_err(|e| AgentError::ChannelError(format!("Discord persistence task failed: {}", e)))?
}

async fn send_gateway_json<
    S: futures_util::Sink<WebSocketMessage, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
>(
    write: &mut S,
    value: serde_json::Value,
) -> Result<()> {
    write
        .send(WebSocketMessage::Text(value.to_string()))
        .await
        .map_err(|e| AgentError::ChannelError(format!("Discord gateway write failed: {}", e)))
}

pub async fn run_discord_gateway_once(
    agentim: Arc<AgentIM>,
    channel: Arc<DiscordBotChannel>,
    agent_id: &str,
    options: MessageHandlingOptions,
    gateway_url: &str,
    state_file: Option<&str>,
    state_backup_count: usize,
) -> Result<()> {
    let (socket, _) = connect_async(gateway_url)
        .await
        .map_err(|e| AgentError::ChannelError(format!("Discord gateway connect failed: {}", e)))?;
    let (mut write, mut read) = socket.split();

    let hello = loop {
        let Some(frame) = read.next().await else {
            return Ok(());
        };
        let frame = frame
            .map_err(|e| AgentError::ChannelError(format!("Discord gateway read failed: {}", e)))?;
        match frame {
            WebSocketMessage::Text(text) => {
                let payload: DiscordGatewayEnvelope = serde_json::from_str(&text).map_err(|e| {
                    AgentError::ChannelError(format!("Invalid Discord payload: {}", e))
                })?;
                if payload.op == 10 {
                    break serde_json::from_value::<DiscordGatewayHello>(payload.d).map_err(
                        |e| {
                            AgentError::ChannelError(format!(
                                "Invalid Discord hello payload: {}",
                                e
                            ))
                        },
                    )?;
                }
            }
            WebSocketMessage::Binary(binary) => {
                let payload: DiscordGatewayEnvelope =
                    serde_json::from_slice(&binary).map_err(|e| {
                        AgentError::ChannelError(format!("Invalid Discord payload: {}", e))
                    })?;
                if payload.op == 10 {
                    break serde_json::from_value::<DiscordGatewayHello>(payload.d).map_err(
                        |e| {
                            AgentError::ChannelError(format!(
                                "Invalid Discord hello payload: {}",
                                e
                            ))
                        },
                    )?;
                }
            }
            WebSocketMessage::Close(_) => return Ok(()),
            WebSocketMessage::Ping(payload) => {
                write
                    .send(WebSocketMessage::Pong(payload))
                    .await
                    .map_err(|e| {
                        AgentError::ChannelError(format!("Discord gateway write failed: {}", e))
                    })?;
            }
            _ => {}
        }
    };

    send_gateway_json(
        &mut write,
        serde_json::json!({
            "op": 2,
            "d": {
                "token": channel.token,
                "intents": DISCORD_GATEWAY_INTENTS,
                "properties": {
                    "os": std::env::consts::OS,
                    "browser": "agentim",
                    "device": "agentim"
                }
            }
        }),
    )
    .await?;

    let mut heartbeat = tokio::time::interval(Duration::from_millis(hello.heartbeat_interval));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    heartbeat.tick().await;

    let mut last_sequence = None;

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                send_gateway_json(
                    &mut write,
                    serde_json::json!({
                        "op": 1,
                        "d": last_sequence,
                    }),
                ).await?;
            }
            frame = read.next() => {
                let Some(frame) = frame else {
                    return Ok(());
                };
                let frame = frame.map_err(|e| {
                    AgentError::ChannelError(format!("Discord gateway read failed: {}", e))
                })?;
                match frame {
                    WebSocketMessage::Text(text) => {
                        let payload: DiscordGatewayEnvelope = serde_json::from_str(&text).map_err(|e| {
                            AgentError::ChannelError(format!("Invalid Discord payload: {}", e))
                        })?;
                        if let Some(sequence) = payload.s {
                            last_sequence = Some(sequence);
                        }

                        match payload.op {
                            0 if payload.t.as_deref() == Some("MESSAGE_CREATE") => {
                                let message: DiscordMessage = serde_json::from_value(payload.d).map_err(|e| {
                                    AgentError::ChannelError(format!("Invalid Discord message payload: {}", e))
                                })?;
                                discord_webhook_handler(
                                    agentim.clone(),
                                    agent_id,
                                    options.max_messages,
                                    options.context_message_limit,
                                    options.agent_timeout_ms,
                                    message,
                                ).await?;
                                if let Some(path) = state_file {
                                    persist_sessions(agentim.clone(), path.to_string(), state_backup_count).await?;
                                }
                            }
                            1 => {
                                send_gateway_json(
                                    &mut write,
                                    serde_json::json!({
                                        "op": 1,
                                        "d": last_sequence,
                                    }),
                                ).await?;
                            }
                            7 => {
                                return Err(AgentError::ChannelError(
                                    "Discord gateway requested reconnect".to_string(),
                                ));
                            }
                            9 => {
                                return Err(AgentError::ChannelError(
                                    "Discord gateway invalid session".to_string(),
                                ));
                            }
                            _ => {}
                        }
                    }
                    WebSocketMessage::Binary(binary) => {
                        let payload: DiscordGatewayEnvelope = serde_json::from_slice(&binary).map_err(|e| {
                            AgentError::ChannelError(format!("Invalid Discord payload: {}", e))
                        })?;
                        if let Some(sequence) = payload.s {
                            last_sequence = Some(sequence);
                        }
                    }
                    WebSocketMessage::Ping(payload) => {
                        write
                            .send(WebSocketMessage::Pong(payload))
                            .await
                            .map_err(|e| AgentError::ChannelError(format!("Discord gateway write failed: {}", e)))?;
                    }
                    WebSocketMessage::Close(_) => return Ok(()),
                    _ => {}
                }
            }
        }
    }
}

pub async fn start_discord_gateway(
    agentim: Arc<AgentIM>,
    channel: Arc<DiscordBotChannel>,
    agent_id: String,
    options: MessageHandlingOptions,
    state_file: Option<String>,
    state_backup_count: usize,
) -> Result<()> {
    loop {
        let gateway_url = channel.get_gateway_url().await?;
        match run_discord_gateway_once(
            agentim.clone(),
            channel.clone(),
            &agent_id,
            options,
            &gateway_url,
            state_file.as_deref(),
            state_backup_count,
        )
        .await
        {
            Ok(()) => {
                tracing::warn!("Discord gateway disconnected; reconnecting");
            }
            Err(err) => {
                tracing::error!(error = %err, "Discord gateway session failed; reconnecting");
            }
        }

        sleep(Duration::from_secs(5)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::config::AgentType;
    use axum::{
        extract::{Path, State},
        routing::{get, post},
        Json, Router,
    };
    use serde_json::Value;
    use std::sync::Mutex;
    use tokio_tungstenite::{accept_async, tungstenite::Message as WebSocketMessage};

    struct EchoAgent;

    #[async_trait]
    impl Agent for EchoAgent {
        fn agent_type(&self) -> AgentType {
            AgentType::OpenAI
        }

        fn id(&self) -> &str {
            "echo-agent"
        }

        async fn send_message(
            &self,
            _session: &mut crate::session::Session,
            messages: Vec<crate::session::Message>,
        ) -> Result<String> {
            Ok(format!(
                "echo:{}",
                messages
                    .last()
                    .map(|message| message.content.clone())
                    .unwrap_or_default()
            ))
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

            let identify = socket.next().await.unwrap().unwrap();
            let identify_text = match identify {
                WebSocketMessage::Text(text) => text.to_string(),
                other => panic!("unexpected identify frame: {:?}", other),
            };
            let identify_json: Value = serde_json::from_str(&identify_text).unwrap();
            assert_eq!(identify_json["op"], 2);

            socket
                .send(WebSocketMessage::Text(
                    serde_json::json!({
                        "op": 0,
                        "t": "MESSAGE_CREATE",
                        "s": 1,
                        "d": {
                            "id": "discord-message-1",
                            "author": {
                                "id": "user-1",
                                "username": "tester"
                            },
                            "content": "hello gateway",
                            "channel_id": "channel-42"
                        }
                    })
                    .to_string(),
                ))
                .await
                .unwrap();

            tokio::time::sleep(Duration::from_millis(50)).await;
            socket.close(None).await.unwrap();
        });

        (format!("ws://{}", addr), handle)
    }

    fn test_channel(api_url: String) -> DiscordBotChannel {
        DiscordBotChannel {
            id: DISCORD_CHANNEL_ID.to_string(),
            token: "discord-test-token".to_string(),
            api_url,
            pending_messages: Arc::new(DashMap::new()),
        }
    }

    #[tokio::test]
    async fn discord_get_gateway_url_uses_gateway_bot_endpoint() {
        let state = DiscordMockState::default();
        let (api_url, _handle) =
            discord_mock_api_server(state, "ws://127.0.0.1:9999".to_string()).await;
        let channel = test_channel(api_url);

        let gateway_url = channel.get_gateway_url().await.unwrap();
        assert_eq!(gateway_url, "ws://127.0.0.1:9999/?v=10&encoding=json");
    }

    #[tokio::test]
    async fn discord_gateway_once_processes_message_create() {
        let (gateway_url, _gateway_handle) = discord_mock_gateway_server().await;
        let state = DiscordMockState::default();
        let (api_url, _api_handle) = discord_mock_api_server(state.clone(), gateway_url).await;
        let channel = Arc::new(test_channel(api_url));
        let agentim = Arc::new(AgentIM::new());
        agentim
            .register_agent("default-agent".to_string(), Arc::new(EchoAgent))
            .unwrap();
        agentim
            .register_channel(DISCORD_CHANNEL_ID.to_string(), channel.clone())
            .unwrap();

        let gateway_url = channel.get_gateway_url().await.unwrap();
        run_discord_gateway_once(
            agentim.clone(),
            channel.clone(),
            "default-agent",
            MessageHandlingOptions {
                max_messages: None,
                context_message_limit: 10,
                agent_timeout_ms: Some(30_000),
            },
            &gateway_url,
            None,
            0,
        )
        .await
        .unwrap();

        assert_eq!(agentim.list_sessions().len(), 1);
        assert_eq!(agentim.list_sessions()[0].messages.len(), 2);

        let sent = state.sent_messages.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0, "channel-42");
        assert_eq!(sent[0].1["content"], "echo:hello gateway");
    }
}
