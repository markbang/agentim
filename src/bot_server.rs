use crate::bots::dingtalk::{dingtalk_webhook_handler, DingTalkWebhook};
use crate::bots::discord::{discord_webhook_handler, DiscordMessage};
use crate::bots::feishu::{feishu_webhook_handler, FeishuMessage};
use crate::bots::qq::{qq_webhook_handler, QQMessage};
use crate::bots::slack::{slack_webhook_handler, verify_signature_with_secret, SlackEvent};
use crate::error::AgentError;
use crate::manager::AgentIM;
use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use chrono::Utc;
use dashmap::DashMap;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use hmac::{Hmac, Mac};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::Sha256;
use std::sync::Arc;

type HmacSha256 = Hmac<Sha256>;
const DINGTALK_MAX_SKEW_MILLIS: i64 = 60 * 60 * 1000;

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
    pub discord_interaction_public_key: Option<String>,
    pub feishu_verification_token: Option<String>,
    pub slack_signing_secret: Option<String>,
    pub dingtalk_secret: Option<String>,
    pub session_ttl_seconds: Option<u64>,
}

impl BotServerConfig {
    pub fn resolve_agent<'a>(
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
            agent_timeout_ms: None,
            state_file: None,
            state_backup_count: 0,
            webhook_secret: None,
            webhook_signing_secret: None,
            webhook_max_skew_seconds: 300,
            discord_interaction_public_key: None,
            feishu_verification_token: None,
            slack_signing_secret: None,
            dingtalk_secret: None,
            session_ttl_seconds: None,
        }
    }
}

#[derive(Clone)]
struct AppState {
    agentim: Arc<AgentIM>,
    config: BotServerConfig,
    replay_cache: Arc<DashMap<String, i64>>,
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
    discord_interaction_public_key_enabled: bool,
    feishu_verification_token_enabled: bool,
    slack_signing_secret_enabled: bool,
    dingtalk_secret_enabled: bool,
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

#[derive(Debug, Default, Deserialize)]
struct DingTalkWebhookQuery {
    sign: Option<String>,
    timestamp: Option<String>,
}

fn persist_if_configured(state: &AppState) -> Result<(), String> {
    if let Some(path) = state.config.state_file.as_deref() {
        state
            .agentim
            .save_sessions_to_path_with_rotation(path, state.config.state_backup_count)
            .map_err(|err| err.to_string())?;
    }

    Ok(())
}

fn header_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn authorize_slack_signature(
    headers: &HeaderMap,
    body: &Bytes,
    state: &AppState,
) -> Result<(), String> {
    let Some(secret) = state.config.slack_signing_secret.as_deref() else {
        return Ok(());
    };

    let timestamp = header_value(headers, "x-slack-request-timestamp")
        .ok_or_else(|| "missing x-slack-request-timestamp".to_string())?;
    let timestamp_value = timestamp
        .parse::<i64>()
        .map_err(|_| "invalid x-slack-request-timestamp".to_string())?;
    let now = Utc::now().timestamp();
    let max_skew = state.config.webhook_max_skew_seconds;
    if (now - timestamp_value).abs() > max_skew {
        return Err("stale Slack request timestamp".to_string());
    }

    let signature = header_value(headers, "x-slack-signature")
        .ok_or_else(|| "missing x-slack-signature".to_string())?;
    let verified = verify_signature_with_secret(secret, body, timestamp, signature)
        .map_err(|_| "invalid Slack signature".to_string())?;

    if !verified {
        return Err("invalid Slack signature".to_string());
    }

    Ok(())
}

fn authorize_dingtalk_signature(
    query: &DingTalkWebhookQuery,
    headers: &HeaderMap,
    state: &AppState,
) -> Result<(), String> {
    let Some(secret) = state.config.dingtalk_secret.as_deref() else {
        return Ok(());
    };

    let timestamp = query
        .timestamp
        .as_deref()
        .or_else(|| header_value(headers, "timestamp"))
        .or_else(|| header_value(headers, "x-dingtalk-timestamp"))
        .ok_or_else(|| "missing DingTalk timestamp".to_string())?;
    let timestamp_value = timestamp
        .parse::<i64>()
        .map_err(|_| "invalid DingTalk timestamp".to_string())?;
    if (Utc::now().timestamp_millis() - timestamp_value).abs() > DINGTALK_MAX_SKEW_MILLIS {
        return Err("stale DingTalk timestamp".to_string());
    }
    let signature = query
        .sign
        .as_deref()
        .or_else(|| header_value(headers, "sign"))
        .or_else(|| header_value(headers, "x-dingtalk-sign"))
        .ok_or_else(|| "missing DingTalk sign".to_string())?;

    let provided_signature = base64::engine::general_purpose::STANDARD
        .decode(signature)
        .map_err(|_| "invalid DingTalk sign".to_string())?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| "invalid DingTalk secret".to_string())?;
    mac.update(format!("{}\n{}", timestamp, secret).as_bytes());
    mac.verify_slice(&provided_signature)
        .map_err(|_| "invalid DingTalk sign".to_string())
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
                discord_interaction_public_key_enabled: false,
                feishu_verification_token_enabled: false,
                slack_signing_secret_enabled: false,
                dingtalk_secret_enabled: false,
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
            discord_interaction_public_key_enabled: state
                .config
                .discord_interaction_public_key
                .is_some(),
            feishu_verification_token_enabled: state.config.feishu_verification_token.is_some(),
            slack_signing_secret_enabled: state.config.slack_signing_secret.is_some(),
            dingtalk_secret_enabled: state.config.dingtalk_secret.is_some(),
        }),
    )
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
    if let Err(err) = authorize_slack_signature(&headers, &body, &state) {
        return (StatusCode::UNAUTHORIZED, err);
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
    Query(query): Query<DingTalkWebhookQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, String) {
    if let Err(err) = authorize_shared(&headers, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }
    if let Err(err) = authorize_signed_webhook(&headers, &body, &state) {
        return (StatusCode::UNAUTHORIZED, err);
    }
    if let Err(err) = authorize_dingtalk_signature(&query, &headers, &state) {
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

async fn readyz(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> (StatusCode, Json<serde_json::Value>) {
    if authorize_shared(&headers, &state).is_err() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"status": "unauthorized"})),
        );
    }

    let agents = state.agentim.list_agents();
    let channels = state.agentim.list_channels();
    let has_agents = !agents.is_empty();
    let has_channels = !channels.is_empty();

    if has_agents && has_channels {
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "ready",
                "agents": agents.len(),
                "channels": channels.len(),
            })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "not_ready",
                "agents": agents.len(),
                "channels": channels.len(),
            })),
        )
    }
}

pub fn create_bot_router_with_config(agentim: Arc<AgentIM>, config: BotServerConfig) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/reviewz", get(reviewz))
        .route("/discord", post(discord_webhook))
        .route("/feishu", post(feishu_webhook))
        .route("/qq", post(qq_webhook))
        .route("/slack", post(slack_webhook))
        .route("/dingtalk", post(dingtalk_webhook))
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024)) // 1 MB
        .with_state(AppState {
            agentim,
            config,
            replay_cache: Arc::new(DashMap::new()),
        })
}

fn cleanup_stale_sessions_with_persistence(
    agentim: &AgentIM,
    max_idle_seconds: u64,
    state_file: Option<&str>,
    state_backup_count: usize,
) -> Result<usize, String> {
    let removed = agentim.cleanup_stale_sessions(max_idle_seconds);
    if removed > 0 {
        if let Some(path) = state_file {
            agentim
                .save_sessions_to_path_with_rotation(path, state_backup_count)
                .map_err(|err| err.to_string())?;
        }
    }
    Ok(removed)
}

pub async fn start_bot_server(
    agentim: Arc<AgentIM>,
    config: BotServerConfig,
    addr: &str,
) -> anyhow::Result<()> {
    if let Some(ttl) = config.session_ttl_seconds {
        let agentim_clone = agentim.clone();
        let state_file = config.state_file.clone();
        let state_backup_count = config.state_backup_count;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
            loop {
                interval.tick().await;
                match cleanup_stale_sessions_with_persistence(
                    agentim_clone.as_ref(),
                    ttl,
                    state_file.as_deref(),
                    state_backup_count,
                ) {
                    Ok(removed) if removed > 0 => {
                        tracing::info!("Cleaned up {} stale session(s)", removed);
                    }
                    Ok(_) => {}
                    Err(err) => {
                        tracing::error!("Failed to persist stale session cleanup: {}", err);
                    }
                }
            }
        });
    }

    let app = create_bot_router_with_config(agentim, config);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Bot server listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Server shut down gracefully");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;

    fn temp_state_file() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("agentim-bot-server-{}.json", nanos))
            .display()
            .to_string()
    }

    #[test]
    fn ttl_cleanup_persists_evictions_to_state_file() {
        let agentim = AgentIM::new();
        let state_file = temp_state_file();
        let mut session = Session::new(
            "default-agent".to_string(),
            "telegram-bot".to_string(),
            "user-1".to_string(),
        );
        session.updated_at = Utc::now() - chrono::Duration::seconds(600);
        let session_id = session.id.clone();
        agentim.update_session(&session_id, session).unwrap();
        agentim.save_sessions_to_path(&state_file).unwrap();

        let removed =
            cleanup_stale_sessions_with_persistence(&agentim, 300, Some(&state_file), 0).unwrap();
        assert_eq!(removed, 1);

        let snapshot = std::fs::read_to_string(&state_file).unwrap();
        let sessions: Vec<Session> = serde_json::from_str(&snapshot).unwrap();
        assert!(sessions.is_empty());

        let _ = std::fs::remove_file(state_file);
    }
}
