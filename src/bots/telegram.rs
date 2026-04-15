use crate::channel::{Channel, ChannelMessage};
use crate::config::ChannelType;
use crate::error::{AgentError, Result};
use crate::manager::{AgentIM, MessageHandlingOptions};
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const TELEGRAM_CHANNEL_ID: &str = "telegram-bot";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramUpdate {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramMessage {
    pub message_id: i64,
    pub chat: TelegramChat,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramChat {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
struct TelegramApiEnvelope<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

pub struct TelegramBotChannel {
    pub(crate) id: String,
    api_url: String,
    pub(crate) pending_messages: Arc<DashMap<String, Vec<String>>>,
}

impl TelegramBotChannel {
    pub fn new(id: String, token: String) -> Self {
        let api_url = format!("https://api.telegram.org/bot{}", token);
        Self {
            id,
            api_url,
            pending_messages: Arc::new(DashMap::new()),
        }
    }

    pub async fn delete_webhook(&self, drop_pending_updates: bool) -> Result<()> {
        let client = reqwest::Client::new();
        let url = format!("{}/deleteWebhook", self.api_url);
        let response = client
            .post(&url)
            .json(&serde_json::json!({
                "drop_pending_updates": drop_pending_updates
            }))
            .send()
            .await
            .map_err(|e| AgentError::ChannelError(e.to_string()))?;
        Self::ensure_api_ok(response).await?;
        Ok(())
    }

    pub async fn get_updates(
        &self,
        offset: Option<i64>,
        timeout_seconds: u64,
        limit: usize,
    ) -> Result<Vec<TelegramUpdate>> {
        let client = reqwest::Client::new();
        let url = format!("{}/getUpdates", self.api_url);
        let response = client
            .post(&url)
            .json(&serde_json::json!({
                "offset": offset,
                "timeout": timeout_seconds,
                "limit": limit,
                "allowed_updates": ["message"]
            }))
            .send()
            .await
            .map_err(|e| AgentError::ChannelError(e.to_string()))?;

        let envelope: TelegramApiEnvelope<Vec<TelegramUpdate>> = response
            .json()
            .await
            .map_err(|e| AgentError::ChannelError(e.to_string()))?;
        if envelope.ok {
            Ok(envelope.result.unwrap_or_default())
        } else {
            Err(AgentError::ChannelError(
                envelope
                    .description
                    .unwrap_or_else(|| "Telegram getUpdates failed".to_string()),
            ))
        }
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

    async fn ensure_api_ok(response: reqwest::Response) -> Result<()> {
        let envelope: TelegramApiEnvelope<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AgentError::ChannelError(e.to_string()))?;
        if envelope.ok {
            Ok(())
        } else {
            Err(AgentError::ChannelError(
                envelope
                    .description
                    .unwrap_or_else(|| "Telegram API call failed".to_string()),
            ))
        }
    }
}

#[async_trait]
impl Channel for TelegramBotChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Telegram
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let url = format!("{}/sendMessage", self.api_url);
        let chat_id: i64 = user_id.parse()?;

        let params = serde_json::json!({
            "chat_id": chat_id,
            "text": content
        });

        client
            .post(&url)
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
        let url = format!("{}/getMe", self.api_url);

        client
            .get(&url)
            .send()
            .await
            .map_err(|e| AgentError::ChannelError(format!("Telegram health check failed: {}", e)))?
            .error_for_status()
            .map_err(|e| {
                AgentError::ChannelError(format!("Telegram health check failed: {}", e))
            })?;

        Ok(())
    }
}

pub async fn handle_telegram_update(
    agentim: Arc<AgentIM>,
    agent_id: &str,
    max_session_messages: Option<usize>,
    context_message_limit: usize,
    agent_timeout_ms: Option<u64>,
    update: TelegramUpdate,
) -> Result<()> {
    if let Some(message) = update.message {
        if let Some(text) = message.text {
            let user_id = message.chat.id.to_string();
            agentim
                .handle_incoming_message_with_options(
                    agent_id,
                    TELEGRAM_CHANNEL_ID,
                    &user_id,
                    Some(&user_id),
                    text,
                    MessageHandlingOptions {
                        max_messages: max_session_messages,
                        context_message_limit,
                        agent_timeout_ms,
                    },
                )
                .await?;
        }
    }

    Ok(())
}

async fn persist_sessions(agentim: Arc<AgentIM>, path: String, backup_count: usize) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        agentim.save_sessions_to_path_with_rotation(&path, backup_count)
    })
    .await
    .map_err(|e| AgentError::ChannelError(format!("Telegram persistence task failed: {}", e)))?
}

#[allow(clippy::too_many_arguments)]
pub async fn run_telegram_poll_once(
    agentim: Arc<AgentIM>,
    channel: Arc<TelegramBotChannel>,
    agent_id: &str,
    options: MessageHandlingOptions,
    next_offset: Option<i64>,
    state_file: Option<&str>,
    state_backup_count: usize,
    timeout_seconds: u64,
) -> Result<Option<i64>> {
    let updates = channel
        .get_updates(next_offset, timeout_seconds, 100)
        .await?;
    let mut next_offset = next_offset;

    for update in updates {
        next_offset = Some(update.update_id + 1);
        let start = std::time::Instant::now();
        let result = handle_telegram_update(
            agentim.clone(),
            agent_id,
            options.max_messages,
            options.context_message_limit,
            options.agent_timeout_ms,
            update,
        )
        .await;
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

        match result {
            Ok(_) => {
                crate::metrics::inc_webhook_request("telegram");
                crate::metrics::observe_agent_latency(agent_id, elapsed_ms);
            }
            Err(ref err) => {
                crate::metrics::inc_webhook_request("telegram");
                crate::metrics::inc_webhook_failure("telegram", "agent");
                crate::metrics::observe_agent_latency(agent_id, elapsed_ms);
                tracing::error!(error = %err, "Telegram polling update failed");
                continue;
            }
        }

        if let Some(path) = state_file {
            persist_sessions(agentim.clone(), path.to_string(), state_backup_count).await?;
        }
    }

    Ok(next_offset)
}

pub async fn start_telegram_long_polling(
    agentim: Arc<AgentIM>,
    channel: Arc<TelegramBotChannel>,
    agent_id: String,
    options: MessageHandlingOptions,
    state_file: Option<String>,
    state_backup_count: usize,
) -> Result<()> {
    channel.delete_webhook(true).await?;
    let mut next_offset = channel
        .get_updates(None, 0, 100)
        .await?
        .into_iter()
        .map(|update| update.update_id + 1)
        .max();
    loop {
        next_offset = run_telegram_poll_once(
            agentim.clone(),
            channel.clone(),
            &agent_id,
            options,
            next_offset,
            state_file.as_deref(),
            state_backup_count,
            30,
        )
        .await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::config::AgentType;
    use axum::{
        extract::State,
        routing::{get, post},
        Json, Router,
    };
    use serde_json::Value;
    use std::sync::Mutex;

    struct EchoAgent;

    #[async_trait]
    impl Agent for EchoAgent {
        fn agent_type(&self) -> AgentType {
            AgentType::Acp
        }

        fn id(&self) -> &str {
            "echo-agent"
        }

        async fn send_message(&self, messages: Vec<crate::session::Message>) -> Result<String> {
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
    struct TelegramMockState {
        updates: Arc<Mutex<Vec<TelegramUpdate>>>,
        sent_messages: Arc<Mutex<Vec<Value>>>,
        delete_webhook_calls: Arc<Mutex<usize>>,
    }

    async fn telegram_mock_server(
        state: TelegramMockState,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let app = Router::new()
            .route(
                "/bot-test-token/getUpdates",
                post(|State(state): State<TelegramMockState>| async move {
                    let updates = state.updates.lock().unwrap().drain(..).collect::<Vec<_>>();
                    Json(serde_json::json!({
                        "ok": true,
                        "result": updates,
                    }))
                }),
            )
            .route(
                "/bot-test-token/sendMessage",
                post(
                    |State(state): State<TelegramMockState>, Json(body): Json<Value>| async move {
                        state.sent_messages.lock().unwrap().push(body);
                        Json(serde_json::json!({"ok": true, "result": true}))
                    },
                ),
            )
            .route(
                "/bot-test-token/deleteWebhook",
                post(|State(state): State<TelegramMockState>| async move {
                    *state.delete_webhook_calls.lock().unwrap() += 1;
                    Json(serde_json::json!({"ok": true, "result": true}))
                }),
            )
            .route(
                "/bot-test-token/getMe",
                get(|| async { Json(serde_json::json!({"ok": true, "result": {"id": 1}})) }),
            )
            .with_state(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        (format!("http://{}/bot-test-token", addr), handle)
    }

    fn test_channel(api_url: String) -> TelegramBotChannel {
        TelegramBotChannel {
            id: TELEGRAM_CHANNEL_ID.to_string(),
            api_url,
            pending_messages: Arc::new(DashMap::new()),
        }
    }

    #[tokio::test]
    async fn telegram_get_updates_parses_mock_response() {
        let state = TelegramMockState::default();
        state.updates.lock().unwrap().push(TelegramUpdate {
            update_id: 42,
            message: Some(TelegramMessage {
                message_id: 7,
                chat: TelegramChat { id: 123 },
                text: Some("hello".to_string()),
            }),
        });
        let (api_url, _handle) = telegram_mock_server(state).await;
        let channel = test_channel(api_url);

        let updates = channel.get_updates(None, 0, 100).await.unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].update_id, 42);
        assert_eq!(
            updates[0].message.as_ref().unwrap().text.as_deref(),
            Some("hello")
        );
    }

    #[tokio::test]
    async fn telegram_poll_once_processes_updates_and_persists_sessions() {
        let state = TelegramMockState::default();
        state.updates.lock().unwrap().push(TelegramUpdate {
            update_id: 99,
            message: Some(TelegramMessage {
                message_id: 11,
                chat: TelegramChat { id: 321 },
                text: Some("ping".to_string()),
            }),
        });
        let (api_url, _handle) = telegram_mock_server(state.clone()).await;
        let channel = Arc::new(test_channel(api_url));
        let agentim = Arc::new(AgentIM::new());
        agentim
            .register_agent("default-agent".to_string(), Arc::new(EchoAgent))
            .unwrap();
        agentim
            .register_channel(TELEGRAM_CHANNEL_ID.to_string(), channel.clone())
            .unwrap();

        let state_file = std::env::temp_dir()
            .join(format!(
                "agentim-telegram-poll-{}.json",
                uuid::Uuid::new_v4()
            ))
            .display()
            .to_string();
        let next_offset = run_telegram_poll_once(
            agentim.clone(),
            channel.clone(),
            "default-agent",
            MessageHandlingOptions {
                max_messages: None,
                context_message_limit: 10,
                agent_timeout_ms: Some(30_000),
            },
            None,
            Some(&state_file),
            1,
            0,
        )
        .await
        .unwrap();

        assert_eq!(next_offset, Some(100));
        assert_eq!(agentim.list_sessions().len(), 1);
        assert_eq!(agentim.list_sessions()[0].messages.len(), 2);
        assert_eq!(state.sent_messages.lock().unwrap().len(), 1);
        assert!(std::path::Path::new(&state_file).exists());

        let _ = std::fs::remove_file(&state_file);
        let _ = std::fs::remove_file(format!("{}.bak.1", state_file));
    }

    #[tokio::test]
    async fn telegram_long_polling_skips_stale_updates_on_start() {
        let state = TelegramMockState::default();
        state.updates.lock().unwrap().push(TelegramUpdate {
            update_id: 50,
            message: Some(TelegramMessage {
                message_id: 5,
                chat: TelegramChat { id: 111 },
                text: Some("/start".to_string()),
            }),
        });
        state.updates.lock().unwrap().push(TelegramUpdate {
            update_id: 51,
            message: Some(TelegramMessage {
                message_id: 6,
                chat: TelegramChat { id: 111 },
                text: Some("fresh".to_string()),
            }),
        });
        let (api_url, _handle) = telegram_mock_server(state.clone()).await;
        let channel = Arc::new(test_channel(api_url));
        let agentim = Arc::new(AgentIM::new());
        agentim
            .register_agent("default-agent".to_string(), Arc::new(EchoAgent))
            .unwrap();
        agentim
            .register_channel(TELEGRAM_CHANNEL_ID.to_string(), channel.clone())
            .unwrap();

        let polling = tokio::spawn(start_telegram_long_polling(
            agentim.clone(),
            channel,
            "default-agent".to_string(),
            MessageHandlingOptions::default(),
            None,
            0,
        ));

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        polling.abort();

        let sent = state.sent_messages.lock().unwrap().clone();
        assert!(
            sent.is_empty(),
            "stale updates should be skipped at startup"
        );
    }
}
