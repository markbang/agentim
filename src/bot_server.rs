use crate::bots::dingtalk::{dingtalk_webhook_handler, DingTalkWebhook};
use crate::bots::discord::{discord_webhook_handler, DiscordMessage};
use crate::bots::feishu::{feishu_webhook_handler, FeishuMessage};
use crate::bots::qq::{qq_webhook_handler, QQMessage};
use crate::bots::slack::{slack_webhook_handler, SlackEvent};
use crate::bots::telegram::{telegram_webhook_handler, TelegramUpdate};
use crate::error::AgentError;
use crate::manager::AgentIM;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use dashmap::DashMap;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use hmac::{Hmac, Mac};
use serde::{de::DeserializeOwned, Serialize};
use sha2::Sha256;
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tower_http::limit::RequestBodyLimitLayer;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug, Serialize)]
pub struct RoutingRule {
    pub channel: Option<String>,
    pub user_id: Option<String>,
    pub user_prefix: Option<String>,
    pub reply_target: Option<String>,
    pub reply_target_prefix: Option<String>,
    pub priority: i32,
    pub agent_id: String,
}

impl RoutingRule {
    fn matches(&self, channel: &str, user_id: &str, reply_target: &str) -> bool {
        self.channel
            .as_deref()
            .map(|value| value == channel)
            .unwrap_or(true)
            && self
                .user_id
                .as_deref()
                .map(|value| value == user_id)
                .unwrap_or(true)
            && self
                .user_prefix
                .as_deref()
                .map(|value| user_id.starts_with(value))
                .unwrap_or(true)
            && self
                .reply_target
                .as_deref()
                .map(|value| value == reply_target)
                .unwrap_or(true)
            && self
                .reply_target_prefix
                .as_deref()
                .map(|value| reply_target.starts_with(value))
                .unwrap_or(true)
    }

    fn specificity(&self) -> i32 {
        [
            self.channel.is_some(),
            self.user_id.is_some(),
            self.user_prefix.is_some(),
            self.reply_target.is_some(),
            self.reply_target_prefix.is_some(),
        ]
        .into_iter()
        .filter(|matched| *matched)
        .count() as i32
    }
}

#[derive(Clone, Debug)]
pub struct BotServerConfig {
    pub telegram_agent_id: String,
    pub discord_agent_id: String,
    pub feishu_agent_id: String,
    pub qq_agent_id: String,
    pub slack_agent_id: String,
    pub dingtalk_agent_id: String,
    pub routing_rules: Vec<RoutingRule>,
    pub max_session_messages: Option<usize>,
    pub context_message_limit: usize,
    pub agent_timeout_ms: Option<u64>,
    pub state_file: Option<String>,
    pub state_backup_count: usize,
    pub webhook_secret: Option<String>,
    pub webhook_signing_secret: Option<String>,
    pub webhook_max_skew_seconds: i64,
    pub telegram_webhook_secret_token: Option<String>,
    pub discord_interaction_public_key: Option<String>,
    pub feishu_verification_token: Option<String>,
    pub slack_signing_secret: Option<String>,
    pub dingtalk_secret: Option<String>,
}

impl BotServerConfig {
    fn resolve_agent<'a>(
        &'a self,
        channel: &str,
        user_id: &str,
        reply_target: &str,
        fallback: &'a str,
    ) -> &'a str {
        self.routing_rules
            .iter()
            .filter(|rule| rule.matches(channel, user_id, reply_target))
            .max_by_key(|rule| (rule.priority, rule.specificity()))
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
            slack_agent_id: "default-agent".to_string(),
            dingtalk_agent_id: "default-agent".to_string(),
            routing_rules: Vec::new(),
            max_session_messages: None,
            context_message_limit: 10,
            agent_timeout_ms: Some(30_000),
            state_file: None,
            state_backup_count: 0,
            webhook_secret: None,
            webhook_signing_secret: None,
            webhook_max_skew_seconds: 300,
            telegram_webhook_secret_token: None,
            discord_interaction_public_key: None,
            feishu_verification_token: None,
            slack_signing_secret: None,
            dingtalk_secret: None,
        }
    }
}

#[derive(Clone)]
struct AppState {
    agentim: Arc<AgentIM>,
    config: BotServerConfig,
    replay_cache: Arc<DashMap<String, i64>>,
    persistence: Option<Arc<PersistenceWorker>>,
}

struct PersistenceWorker {
    trigger: UnboundedSender<()>,
}

impl PersistenceWorker {
    fn spawn(agentim: Arc<AgentIM>, path: String, backup_count: usize) -> Arc<Self> {
        let (trigger, mut receiver) = unbounded_channel();
        let worker = Arc::new(Self { trigger });

        tokio::spawn(async move {
            while receiver.recv().await.is_some() {
                let agentim = agentim.clone();
                let persist_path = path.clone();
                let log_path = persist_path.clone();
                match tokio::task::spawn_blocking(move || {
                    agentim.save_sessions_to_path_with_rotation(&persist_path, backup_count)
                })
                .await
                {
                    Ok(Ok(())) => {}
                    Ok(Err(err)) => {
                        tracing::error!(error = %err, path = %log_path, "failed to persist sessions");
                    }
                    Err(err) => {
                        tracing::error!(error = %err, path = %log_path, "persistence worker crashed");
                    }
                }
            }
        });

        worker
    }

    fn request_snapshot(&self) -> Result<(), String> {
        self.trigger
            .send(())
            .map_err(|_| "persistence worker unavailable".to_string())
    }
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
    max_session_messages: Option<usize>,
    context_message_limit: usize,
    agent_timeout_ms: Option<u64>,
    state_backup_count: usize,
    persistence_enabled: bool,
    webhook_secret_enabled: bool,
    webhook_signing_enabled: bool,
    webhook_max_skew_seconds: i64,
    telegram_webhook_secret_token_enabled: bool,
    discord_interaction_public_key_enabled: bool,
    feishu_verification_token_enabled: bool,
    slack_signing_secret_enabled: bool,
    dingtalk_secret_enabled: bool,
    acp_sessions: Vec<AcpSessionReview>,
}

#[derive(Serialize)]
struct AcpSessionReview {
    session_id: String,
    agent_id: String,
    channel_id: String,
    user_id: Option<String>,
    remote_session_id: Option<String>,
    backend: String,
    agent: Option<String>,
    stop_reason: Option<String>,
}

#[derive(Serialize)]
struct PlatformAgents {
    telegram: String,
    discord: String,
    feishu: String,
    qq: String,
    slack: String,
    dingtalk: String,
}

#[derive(Serialize)]
struct FeishuChallengeResponse {
    challenge: String,
}

fn collect_acp_sessions(agentim: &AgentIM, include_sensitive_ids: bool) -> Vec<AcpSessionReview> {
    let mut sessions = agentim
        .list_sessions()
        .into_iter()
        .filter_map(|session| {
            let remote_session_id = session.metadata.get("acp_session_id")?.clone();
            Some(AcpSessionReview {
                session_id: session.id,
                agent_id: session.agent_id,
                channel_id: session.channel_id,
                user_id: include_sensitive_ids.then_some(session.user_id),
                remote_session_id: include_sensitive_ids.then_some(remote_session_id),
                backend: session
                    .metadata
                    .get("acp_backend")
                    .cloned()
                    .unwrap_or_default(),
                agent: session.metadata.get("acp_agent").cloned(),
                stop_reason: session.metadata.get("acp_stop_reason").cloned(),
            })
        })
        .collect::<Vec<_>>();

    sessions.sort_by(|left, right| left.session_id.cmp(&right.session_id));
    sessions
}

fn persist_if_configured(state: &AppState) -> Result<(), String> {
    if let Some(worker) = state.persistence.as_ref() {
        worker.request_snapshot()?;
    }

    Ok(())
}

fn authorize_shared(headers: &HeaderMap, state: &AppState) -> Result<(), String> {
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

fn prune_replay_cache(state: &AppState, oldest_allowed_timestamp: i64) {
    let stale: Vec<String> = state
        .replay_cache
        .iter()
        .filter(|entry| *entry.value() < oldest_allowed_timestamp)
        .map(|entry| entry.key().clone())
        .collect();

    for key in stale {
        state.replay_cache.remove(&key);
    }
}

fn authorize_signed_webhook(
    headers: &HeaderMap,
    body: &Bytes,
    state: &AppState,
) -> Result<(), String> {
    let Some(secret) = state.config.webhook_signing_secret.as_deref() else {
        return Ok(());
    };

    let timestamp = headers
        .get("x-agentim-timestamp")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| "missing x-agentim-timestamp".to_string())?;
    let nonce = headers
        .get("x-agentim-nonce")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| "missing x-agentim-nonce".to_string())?;
    let signature = headers
        .get("x-agentim-signature")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| "missing x-agentim-signature".to_string())?;

    let timestamp_value = timestamp
        .parse::<i64>()
        .map_err(|_| "invalid x-agentim-timestamp".to_string())?;
    let now = Utc::now().timestamp();
    let max_skew = state.config.webhook_max_skew_seconds;

    if (now - timestamp_value).abs() > max_skew {
        return Err("signed webhook timestamp out of range".to_string());
    }

    prune_replay_cache(state, now - max_skew);
    let replay_key = format!("{}:{}", timestamp_value, nonce);
    if state.replay_cache.contains_key(&replay_key) {
        return Err("replayed webhook request".to_string());
    }

    let signature_hex = signature.strip_prefix("sha256=").unwrap_or(signature);
    let provided_signature = hex::decode(signature_hex)
        .map_err(|_| "invalid x-agentim-signature encoding".to_string())?;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| "invalid webhook signing secret".to_string())?;
    mac.update(timestamp.as_bytes());
    mac.update(b"\n");
    mac.update(nonce.as_bytes());
    mac.update(b"\n");
    mac.update(body);

    mac.verify_slice(&provided_signature)
        .map_err(|_| "invalid x-agentim-signature".to_string())?;

    state.replay_cache.insert(replay_key, timestamp_value);
    Ok(())
}

fn authorize_telegram_secret_token(headers: &HeaderMap, state: &AppState) -> Result<(), String> {
    let Some(expected) = state.config.telegram_webhook_secret_token.as_deref() else {
        return Ok(());
    };

    let provided = headers
        .get("x-telegram-bot-api-secret-token")
        .and_then(|value| value.to_str().ok());

    if provided != Some(expected) {
        return Err("missing or invalid x-telegram-bot-api-secret-token".to_string());
    }

    Ok(())
}

fn authorize_discord_interaction_signature(
    headers: &HeaderMap,
    body: &Bytes,
    state: &AppState,
) -> Result<(), String> {
    let Some(public_key_hex) = state.config.discord_interaction_public_key.as_deref() else {
        return Ok(());
    };

    let timestamp = headers
        .get("x-signature-timestamp")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| "missing x-signature-timestamp".to_string())?;
    let signature_hex = headers
        .get("x-signature-ed25519")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| "missing x-signature-ed25519".to_string())?;

    let public_key_bytes = hex::decode(public_key_hex)
        .map_err(|_| "invalid discord interaction public key encoding".to_string())?;
    let verifying_key = VerifyingKey::from_bytes(
        &public_key_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "invalid discord interaction public key length".to_string())?,
    )
    .map_err(|_| "invalid discord interaction public key".to_string())?;

    let signature_bytes = hex::decode(signature_hex)
        .map_err(|_| "invalid x-signature-ed25519 encoding".to_string())?;
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|_| "invalid x-signature-ed25519 length".to_string())?;

    let mut signed_message = timestamp.as_bytes().to_vec();
    signed_message.extend_from_slice(body);
    verifying_key
        .verify(&signed_message, &signature)
        .map_err(|_| "invalid x-signature-ed25519".to_string())
}

fn authorize_feishu_verification_token(body: &Bytes, state: &AppState) -> Result<(), String> {
    let Some(expected) = state.config.feishu_verification_token.as_deref() else {
        return Ok(());
    };

    let value = serde_json::from_slice::<serde_json::Value>(body)
        .map_err(|_| "invalid feishu webhook payload".to_string())?;
    let provided = value.get("token").and_then(|value| value.as_str());

    if provided != Some(expected) {
        return Err("missing or invalid Feishu verification token".to_string());
    }

    Ok(())
}

fn parse_json_body<T: DeserializeOwned>(body: &Bytes) -> Result<T, String> {
    serde_json::from_slice(body).map_err(|err| err.to_string())
}

fn webhook_error_status(err: &AgentError) -> StatusCode {
    match err {
        AgentError::TimeoutError(_) => StatusCode::GATEWAY_TIMEOUT,
        AgentError::ApiError(_) | AgentError::HttpError(_) | AgentError::ChannelError(_) => {
            StatusCode::BAD_GATEWAY
        }
        _ => StatusCode::BAD_REQUEST,
    }
}

async fn healthz(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> (StatusCode, Json<HealthResponse>) {
    if authorize_shared(&headers, &state).is_err() {
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
    if authorize_shared(&headers, &state).is_err() {
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
                    slack: String::new(),
                    dingtalk: String::new(),
                },
                routing_rules: Vec::new(),
                max_session_messages: None,
                context_message_limit: 10,
                agent_timeout_ms: None,
                state_backup_count: 0,
                persistence_enabled: false,
                webhook_secret_enabled: true,
                webhook_signing_enabled: false,
                webhook_max_skew_seconds: 0,
                telegram_webhook_secret_token_enabled: false,
                discord_interaction_public_key_enabled: false,
                feishu_verification_token_enabled: false,
                slack_signing_secret_enabled: false,
                dingtalk_secret_enabled: false,
                acp_sessions: Vec::new(),
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
                slack: state.config.slack_agent_id.clone(),
                dingtalk: state.config.dingtalk_agent_id.clone(),
            },
            routing_rules: state.config.routing_rules.clone(),
            max_session_messages: state.config.max_session_messages,
            context_message_limit: state.config.context_message_limit,
            agent_timeout_ms: state.config.agent_timeout_ms,
            state_backup_count: state.config.state_backup_count,
            persistence_enabled: state.config.state_file.is_some(),
            webhook_secret_enabled: state.config.webhook_secret.is_some(),
            webhook_signing_enabled: state.config.webhook_signing_secret.is_some(),
            webhook_max_skew_seconds: state.config.webhook_max_skew_seconds,
            telegram_webhook_secret_token_enabled: state
                .config
                .telegram_webhook_secret_token
                .is_some(),
            discord_interaction_public_key_enabled: state
                .config
                .discord_interaction_public_key
                .is_some(),
            feishu_verification_token_enabled: state.config.feishu_verification_token.is_some(),
            slack_signing_secret_enabled: state.config.slack_signing_secret.is_some(),
            dingtalk_secret_enabled: state.config.dingtalk_secret.is_some(),
            acp_sessions: collect_acp_sessions(
                &state.agentim,
                state.config.webhook_secret.is_some(),
            ),
        }),
    )
}

async fn telegram_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, String) {
    if let Err(err) = authorize_shared(&headers, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }
    if let Err(err) = authorize_signed_webhook(&headers, &body, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }
    if let Err(err) = authorize_telegram_secret_token(&headers, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }

    let update: TelegramUpdate = match parse_json_body(&body) {
        Ok(update) => update,
        Err(err) => return (StatusCode::BAD_REQUEST, err),
    };

    let agent_id = update
        .message
        .as_ref()
        .map(|message| {
            let user_id = message.chat.id.to_string();
            state
                .config
                .resolve_agent(
                    "telegram",
                    &user_id,
                    &user_id,
                    state.config.telegram_agent_id.as_str(),
                )
                .to_string()
        })
        .unwrap_or_else(|| state.config.telegram_agent_id.clone());

    match telegram_webhook_handler(
        state.agentim.clone(),
        &agent_id,
        state.config.max_session_messages,
        state.config.context_message_limit,
        state.config.agent_timeout_ms,
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
            (webhook_error_status(&err), err.to_string())
        }
    }
}

async fn discord_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, String) {
    if let Err(err) = authorize_shared(&headers, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }
    if let Err(err) = authorize_signed_webhook(&headers, &body, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }
    if let Err(err) = authorize_discord_interaction_signature(&headers, &body, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }

    let message: DiscordMessage = match parse_json_body(&body) {
        Ok(message) => message,
        Err(err) => return (StatusCode::BAD_REQUEST, err),
    };

    let agent_id = state
        .config
        .resolve_agent(
            "discord",
            &message.author.id,
            &message.channel_id,
            state.config.discord_agent_id.as_str(),
        )
        .to_string();

    match discord_webhook_handler(
        state.agentim.clone(),
        &agent_id,
        state.config.max_session_messages,
        state.config.context_message_limit,
        state.config.agent_timeout_ms,
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
            (webhook_error_status(&err), err.to_string())
        }
    }
}

async fn feishu_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, String) {
    if let Err(err) = authorize_shared(&headers, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }
    if let Err(err) = authorize_signed_webhook(&headers, &body, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }
    if let Err(err) = authorize_feishu_verification_token(&body, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }

    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body) {
        if value.get("type").and_then(|value| value.as_str()) == Some("url_verification") {
            if let Some(challenge) = value.get("challenge").and_then(|value| value.as_str()) {
                return (
                    StatusCode::OK,
                    serde_json::to_string(&FeishuChallengeResponse {
                        challenge: challenge.to_string(),
                    })
                    .unwrap_or_else(|_| format!("{{\"challenge\":\"{}\"}}", challenge)),
                );
            }
        }
    }

    let message: FeishuMessage = match parse_json_body(&body) {
        Ok(message) => message,
        Err(err) => return (StatusCode::BAD_REQUEST, err),
    };

    let agent_id = state
        .config
        .resolve_agent(
            "feishu",
            &message.event.message.sender_id.user_id,
            &message.event.message.sender_id.user_id,
            state.config.feishu_agent_id.as_str(),
        )
        .to_string();

    match feishu_webhook_handler(
        state.agentim.clone(),
        &agent_id,
        state.config.max_session_messages,
        state.config.context_message_limit,
        state.config.agent_timeout_ms,
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
            (webhook_error_status(&err), err.to_string())
        }
    }
}

async fn qq_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, String) {
    if let Err(err) = authorize_shared(&headers, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }
    if let Err(err) = authorize_signed_webhook(&headers, &body, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }

    let message: QQMessage = match parse_json_body(&body) {
        Ok(message) => message,
        Err(err) => return (StatusCode::BAD_REQUEST, err),
    };

    let agent_id = state
        .config
        .resolve_agent(
            "qq",
            &message.author.id,
            &message.channel_id,
            state.config.qq_agent_id.as_str(),
        )
        .to_string();

    match qq_webhook_handler(
        state.agentim.clone(),
        &agent_id,
        state.config.max_session_messages,
        state.config.context_message_limit,
        state.config.agent_timeout_ms,
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
            (webhook_error_status(&err), err.to_string())
        }
    }
}

async fn slack_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, String) {
    if let Err(err) = authorize_shared(&headers, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }
    if let Err(err) = authorize_signed_webhook(&headers, &body, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }

    // Verify Slack signature if configured
    if let Some(ref _secret) = state.config.slack_signing_secret {
        let timestamp = match headers
            .get("x-slack-request-timestamp")
            .and_then(|value| value.to_str().ok())
        {
            Some(value) => value,
            None => {
                return (
                    StatusCode::UNAUTHORIZED,
                    "missing x-slack-request-timestamp".to_string(),
                )
            }
        };
        let signature = match headers
            .get("x-slack-signature")
            .and_then(|value| value.to_str().ok())
        {
            Some(value) => value,
            None => {
                return (
                    StatusCode::UNAUTHORIZED,
                    "missing x-slack-signature".to_string(),
                )
            }
        };

        let Ok(channel) = state.agentim.get_channel(crate::bots::SLACK_CHANNEL_ID) else {
            return (
                StatusCode::UNAUTHORIZED,
                "slack channel not initialized for signature verification".to_string(),
            );
        };
        let Some(slack_channel) = channel
            .as_any()
            .downcast_ref::<crate::bots::SlackBotChannel>()
        else {
            return (
                StatusCode::UNAUTHORIZED,
                "slack channel not initialized for signature verification".to_string(),
            );
        };
        if !slack_channel
            .verify_signature(&body, timestamp, signature)
            .unwrap_or(false)
        {
            return (
                StatusCode::UNAUTHORIZED,
                "invalid Slack signature".to_string(),
            );
        }
    }

    let event: SlackEvent = match parse_json_body(&body) {
        Ok(event) => event,
        Err(err) => return (StatusCode::BAD_REQUEST, err),
    };

    // Handle URL verification challenge
    if event.event_type == "url_verification" {
        if let Some(challenge) = event.challenge {
            return (StatusCode::OK, challenge);
        }
    }

    let agent_id = event
        .event
        .as_ref()
        .map(|detail| {
            let user_id = detail.user.as_deref().unwrap_or("");
            let channel = detail.channel.as_deref().unwrap_or("");
            state
                .config
                .resolve_agent(
                    "slack",
                    user_id,
                    channel,
                    state.config.slack_agent_id.as_str(),
                )
                .to_string()
        })
        .unwrap_or_else(|| state.config.slack_agent_id.clone());

    match slack_webhook_handler(
        state.agentim.clone(),
        &agent_id,
        state.config.max_session_messages,
        state.config.context_message_limit,
        state.config.agent_timeout_ms,
        event,
    )
    .await
    {
        Ok(challenge_response) => match persist_if_configured(&state) {
            Ok(_) => {
                if let Some(challenge) = challenge_response {
                    (StatusCode::OK, challenge)
                } else {
                    (StatusCode::OK, "ok".to_string())
                }
            }
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err),
        },
        Err(err) => {
            tracing::error!("slack webhook failed: {}", err);
            (webhook_error_status(&err), err.to_string())
        }
    }
}

async fn dingtalk_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, String) {
    if let Err(err) = authorize_shared(&headers, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }
    if let Err(err) = authorize_signed_webhook(&headers, &body, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }

    let webhook: DingTalkWebhook = match parse_json_body(&body) {
        Ok(webhook) => webhook,
        Err(err) => return (StatusCode::BAD_REQUEST, err),
    };

    let agent_id = state
        .config
        .resolve_agent(
            "dingtalk",
            &webhook.sender_id,
            &webhook.conversation_id,
            state.config.dingtalk_agent_id.as_str(),
        )
        .to_string();

    match dingtalk_webhook_handler(
        state.agentim.clone(),
        &agent_id,
        state.config.max_session_messages,
        state.config.context_message_limit,
        state.config.agent_timeout_ms,
        webhook,
    )
    .await
    {
        Ok(_) => match persist_if_configured(&state) {
            Ok(_) => (StatusCode::OK, "ok".to_string()),
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err),
        },
        Err(err) => {
            tracing::error!("dingtalk webhook failed: {}", err);
            (webhook_error_status(&err), err.to_string())
        }
    }
}

pub fn create_bot_router(agentim: Arc<AgentIM>) -> Router {
    create_bot_router_with_config(agentim, BotServerConfig::default())
}

pub fn create_bot_router_with_config(agentim: Arc<AgentIM>, config: BotServerConfig) -> Router {
    let persistence = config.state_file.as_ref().map(|path| {
        PersistenceWorker::spawn(agentim.clone(), path.clone(), config.state_backup_count)
    });

    Router::new()
        .route("/healthz", get(healthz))
        .route("/reviewz", get(reviewz))
        .route("/telegram", post(telegram_webhook))
        .route("/discord", post(discord_webhook))
        .route("/feishu", post(feishu_webhook))
        .route("/qq", post(qq_webhook))
        .route("/slack", post(slack_webhook))
        .route("/dingtalk", post(dingtalk_webhook))
        .layer(RequestBodyLimitLayer::new(256 * 1024))
        .with_state(AppState {
            agentim,
            config,
            replay_cache: Arc::new(DashMap::new()),
            persistence,
        })
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
