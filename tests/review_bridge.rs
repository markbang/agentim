use agentim::agent::Agent;
use agentim::bot_server::{
    create_bot_router, create_bot_router_with_config, BotServerConfig, RoutingRule,
};
use agentim::bots::{DISCORD_CHANNEL_ID, FEISHU_CHANNEL_ID, QQ_CHANNEL_ID, TELEGRAM_CHANNEL_ID};
use agentim::channel::{Channel, ChannelMessage};
use agentim::config::{AgentType, ChannelType};
use agentim::manager::AgentIM;
use agentim::session::Message;
use agentim::Result;
use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;

struct ReviewAgent {
    id: String,
    label: String,
}

#[async_trait]
impl Agent for ReviewAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Claude
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, messages: Vec<Message>) -> Result<String> {
        let last = messages.last().map(|msg| msg.content.clone()).unwrap_or_default();
        Ok(format!("{}:{}", self.label, last))
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

struct ReviewChannel {
    id: String,
    sent_messages: Arc<Mutex<Vec<(String, String, String)>>>,
    channel_type: ChannelType,
}

#[async_trait]
impl Channel for ReviewChannel {
    fn channel_type(&self) -> ChannelType {
        self.channel_type
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        self.sent_messages.lock().unwrap().push((
            self.id.clone(),
            user_id.to_string(),
            content.to_string(),
        ));
        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

fn register_review_agent(agentim: &Arc<AgentIM>, id: &str, label: &str) {
    agentim
        .register_agent(
            id.to_string(),
            Arc::new(ReviewAgent {
                id: id.to_string(),
                label: label.to_string(),
            }),
        )
        .unwrap();
}

fn register_review_channel(
    agentim: &Arc<AgentIM>,
    sent_messages: Arc<Mutex<Vec<(String, String, String)>>>,
    id: &str,
    channel_type: ChannelType,
) {
    agentim
        .register_channel(
            id.to_string(),
            Arc::new(ReviewChannel {
                id: id.to_string(),
                sent_messages,
                channel_type,
            }),
        )
        .unwrap();
}

type HmacSha256 = Hmac<Sha256>;

fn temp_state_file() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir()
        .join(format!("agentim-review-{}.json", nanos))
        .display()
        .to_string()
}

fn signed_headers(secret: &str, body: &str, timestamp: i64, nonce: &str) -> [(String, String); 3] {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b"\n");
    mac.update(nonce.as_bytes());
    mac.update(b"\n");
    mac.update(body.as_bytes());

    [
        ("x-agentim-timestamp".to_string(), timestamp.to_string()),
        ("x-agentim-nonce".to_string(), nonce.to_string()),
        (
            "x-agentim-signature".to_string(),
            format!("sha256={}", hex::encode(mac.finalize().into_bytes())),
        ),
    ]
}

fn review_manager(sent_messages: Arc<Mutex<Vec<(String, String, String)>>>) -> Arc<AgentIM> {
    let agentim = Arc::new(AgentIM::new());
    register_review_agent(&agentim, "default-agent", "default");

    for (id, channel_type) in [
        (TELEGRAM_CHANNEL_ID, ChannelType::Telegram),
        (DISCORD_CHANNEL_ID, ChannelType::Discord),
        (FEISHU_CHANNEL_ID, ChannelType::Feishu),
        (QQ_CHANNEL_ID, ChannelType::QQ),
    ] {
        register_review_channel(&agentim, sent_messages.clone(), id, channel_type);
    }

    agentim
}

#[tokio::test]
async fn functionality_reviewer_routes_all_platform_webhooks() {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = review_manager(sent_messages.clone());
    let app = create_bot_router(agentim.clone());

    let telegram = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"update_id":1,"message":{"message_id":10,"chat":{"id":123},"text":"hello telegram"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(telegram.status(), StatusCode::OK);

    let discord = app
        .clone()
        .oneshot(
            Request::post("/discord")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"id":"m1","author":{"id":"user-discord","username":"discorder"},"content":"hello discord","channel_id":"channel-discord"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(discord.status(), StatusCode::OK);

    let feishu = app
        .clone()
        .oneshot(
            Request::post("/feishu")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"token":"t","ts":"1","uuid":"u","event":{"message":{"chat_id":"chat-feishu","sender_id":{"user_id":"user-feishu"},"content":"hello feishu"}}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(feishu.status(), StatusCode::OK);

    let qq = app
        .clone()
        .oneshot(
            Request::post("/qq")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"id":"m2","author":{"id":"user-qq","username":"qqer"},"content":"hello qq","channel_id":"channel-qq"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(qq.status(), StatusCode::OK);

    assert_eq!(agentim.list_sessions().len(), 4);
    assert_eq!(sent_messages.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn readiness_reviewer_tracks_reply_targets_for_channel_based_platforms() {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = review_manager(sent_messages.clone());

    agentim
        .handle_incoming_message(
            "default-agent",
            DISCORD_CHANNEL_ID,
            "discord-user",
            Some("discord-channel"),
            "ping discord".to_string(),
        )
        .await
        .unwrap();
    agentim
        .handle_incoming_message(
            "default-agent",
            QQ_CHANNEL_ID,
            "qq-user",
            Some("qq-channel"),
            "ping qq".to_string(),
        )
        .await
        .unwrap();

    let sent = sent_messages.lock().unwrap().clone();
    assert!(sent.contains(&(
        DISCORD_CHANNEL_ID.to_string(),
        "discord-channel".to_string(),
        "default:ping discord".to_string(),
    )));
    assert!(sent.contains(&(
        QQ_CHANNEL_ID.to_string(),
        "qq-channel".to_string(),
        "default:ping qq".to_string(),
    )));
}

#[tokio::test]
async fn usability_reviewer_reuses_session_per_user_and_channel() {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = review_manager(sent_messages);

    agentim
        .handle_incoming_message(
            "default-agent",
            TELEGRAM_CHANNEL_ID,
            "123",
            Some("123"),
            "first".to_string(),
        )
        .await
        .unwrap();
    agentim
        .handle_incoming_message(
            "default-agent",
            TELEGRAM_CHANNEL_ID,
            "123",
            Some("123"),
            "second".to_string(),
        )
        .await
        .unwrap();

    let sessions = agentim.list_sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].messages.len(), 4);
}

#[tokio::test]
async fn functionality_reviewer_routes_channels_to_configured_agents() {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = Arc::new(AgentIM::new());

    register_review_agent(&agentim, "default-agent", "default");
    register_review_agent(&agentim, "telegram-agent", "telegram");
    register_review_agent(&agentim, "discord-agent", "discord");
    register_review_channel(
        &agentim,
        sent_messages.clone(),
        TELEGRAM_CHANNEL_ID,
        ChannelType::Telegram,
    );
    register_review_channel(
        &agentim,
        sent_messages.clone(),
        DISCORD_CHANNEL_ID,
        ChannelType::Discord,
    );

    let app = create_bot_router_with_config(
        agentim,
        BotServerConfig {
            telegram_agent_id: "telegram-agent".to_string(),
            discord_agent_id: "discord-agent".to_string(),
            ..BotServerConfig::default()
        },
    );

    let telegram = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"update_id":2,"message":{"message_id":20,"chat":{"id":456},"text":"channel specific telegram"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(telegram.status(), StatusCode::OK);

    let discord = app
        .clone()
        .oneshot(
            Request::post("/discord")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"id":"m3","author":{"id":"user-2","username":"discorder2"},"content":"channel specific discord","channel_id":"discord-room"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(discord.status(), StatusCode::OK);

    let sent = sent_messages.lock().unwrap().clone();
    assert!(sent.contains(&(
        TELEGRAM_CHANNEL_ID.to_string(),
        "456".to_string(),
        "telegram:channel specific telegram".to_string(),
    )));
    assert!(sent.contains(&(
        DISCORD_CHANNEL_ID.to_string(),
        "discord-room".to_string(),
        "discord:channel specific discord".to_string(),
    )));
}

#[tokio::test]
async fn routing_reviewer_overrides_platform_route_for_matching_user() {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = Arc::new(AgentIM::new());

    register_review_agent(&agentim, "default-agent", "default");
    register_review_agent(&agentim, "telegram-agent", "telegram");
    register_review_agent(&agentim, "rule-agent-pi", "vip");
    register_review_channel(
        &agentim,
        sent_messages.clone(),
        TELEGRAM_CHANNEL_ID,
        ChannelType::Telegram,
    );

    let app = create_bot_router_with_config(
        agentim,
        BotServerConfig {
            telegram_agent_id: "telegram-agent".to_string(),
            routing_rules: vec![RoutingRule {
                channel: Some("telegram".to_string()),
                user_id: Some("999".to_string()),
                user_prefix: None,
                reply_target: None,
                reply_target_prefix: None,
                priority: 0,
                agent_id: "rule-agent-pi".to_string(),
            }],
            ..BotServerConfig::default()
        },
    );

    let vip = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"update_id":3,"message":{"message_id":30,"chat":{"id":999},"text":"vip route"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(vip.status(), StatusCode::OK);

    let normal = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"update_id":4,"message":{"message_id":40,"chat":{"id":555},"text":"normal route"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(normal.status(), StatusCode::OK);

    let sent = sent_messages.lock().unwrap().clone();
    assert!(sent.contains(&(
        TELEGRAM_CHANNEL_ID.to_string(),
        "999".to_string(),
        "vip:vip route".to_string(),
    )));
    assert!(sent.contains(&(
        TELEGRAM_CHANNEL_ID.to_string(),
        "555".to_string(),
        "telegram:normal route".to_string(),
    )));
}

#[tokio::test]
async fn routing_reviewer_overrides_platform_route_for_matching_reply_target() {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = Arc::new(AgentIM::new());

    register_review_agent(&agentim, "default-agent", "default");
    register_review_agent(&agentim, "discord-agent", "discord");
    register_review_agent(&agentim, "rule-agent-codex", "room");
    register_review_channel(
        &agentim,
        sent_messages.clone(),
        DISCORD_CHANNEL_ID,
        ChannelType::Discord,
    );

    let app = create_bot_router_with_config(
        agentim,
        BotServerConfig {
            discord_agent_id: "discord-agent".to_string(),
            routing_rules: vec![RoutingRule {
                channel: Some("discord".to_string()),
                user_id: None,
                user_prefix: None,
                reply_target: Some("room-1".to_string()),
                reply_target_prefix: None,
                priority: 0,
                agent_id: "rule-agent-codex".to_string(),
            }],
            ..BotServerConfig::default()
        },
    );

    let room_match = app
        .clone()
        .oneshot(
            Request::post("/discord")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"id":"m4","author":{"id":"user-a","username":"discorder-a"},"content":"room route","channel_id":"room-1"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(room_match.status(), StatusCode::OK);

    let room_default = app
        .clone()
        .oneshot(
            Request::post("/discord")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"id":"m5","author":{"id":"user-b","username":"discorder-b"},"content":"default room","channel_id":"room-2"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(room_default.status(), StatusCode::OK);

    let sent = sent_messages.lock().unwrap().clone();
    assert!(sent.contains(&(
        DISCORD_CHANNEL_ID.to_string(),
        "room-1".to_string(),
        "room:room route".to_string(),
    )));
    assert!(sent.contains(&(
        DISCORD_CHANNEL_ID.to_string(),
        "room-2".to_string(),
        "discord:default room".to_string(),
    )));
}

#[tokio::test]
async fn routing_reviewer_prefers_higher_priority_rule_when_multiple_match() {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = Arc::new(AgentIM::new());

    register_review_agent(&agentim, "default-agent", "default");
    register_review_agent(&agentim, "telegram-agent", "telegram");
    register_review_agent(&agentim, "rule-agent-pi", "priority-prefix");
    register_review_agent(&agentim, "rule-agent-codex", "priority-exact");
    register_review_channel(
        &agentim,
        sent_messages.clone(),
        TELEGRAM_CHANNEL_ID,
        ChannelType::Telegram,
    );

    let app = create_bot_router_with_config(
        agentim,
        BotServerConfig {
            telegram_agent_id: "telegram-agent".to_string(),
            routing_rules: vec![
                RoutingRule {
                    channel: Some("telegram".to_string()),
                    user_id: None,
                    user_prefix: Some("7".to_string()),
                    reply_target: None,
                    reply_target_prefix: None,
                    priority: 1,
                    agent_id: "rule-agent-pi".to_string(),
                },
                RoutingRule {
                    channel: Some("telegram".to_string()),
                    user_id: Some("7007".to_string()),
                    user_prefix: None,
                    reply_target: None,
                    reply_target_prefix: None,
                    priority: 10,
                    agent_id: "rule-agent-codex".to_string(),
                },
            ],
            ..BotServerConfig::default()
        },
    );

    let exact = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"update_id":18,"message":{"message_id":180,"chat":{"id":7007},"text":"priority exact"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(exact.status(), StatusCode::OK);

    let prefix_only = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"update_id":19,"message":{"message_id":190,"chat":{"id":7008},"text":"priority prefix"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(prefix_only.status(), StatusCode::OK);

    let sent = sent_messages.lock().unwrap().clone();
    assert!(sent.contains(&(
        TELEGRAM_CHANNEL_ID.to_string(),
        "7007".to_string(),
        "priority-exact:priority exact".to_string(),
    )));
    assert!(sent.contains(&(
        TELEGRAM_CHANNEL_ID.to_string(),
        "7008".to_string(),
        "priority-prefix:priority prefix".to_string(),
    )));
}

#[tokio::test]
async fn routing_reviewer_matches_reply_target_prefix() {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = Arc::new(AgentIM::new());

    register_review_agent(&agentim, "default-agent", "default");
    register_review_agent(&agentim, "discord-agent", "discord");
    register_review_agent(&agentim, "rule-agent-pi", "review-prefix");
    register_review_channel(
        &agentim,
        sent_messages.clone(),
        DISCORD_CHANNEL_ID,
        ChannelType::Discord,
    );

    let app = create_bot_router_with_config(
        agentim,
        BotServerConfig {
            discord_agent_id: "discord-agent".to_string(),
            routing_rules: vec![RoutingRule {
                channel: Some("discord".to_string()),
                user_id: None,
                user_prefix: None,
                reply_target: None,
                reply_target_prefix: Some("review-".to_string()),
                priority: 0,
                agent_id: "rule-agent-pi".to_string(),
            }],
            ..BotServerConfig::default()
        },
    );

    let matched = app
        .clone()
        .oneshot(
            Request::post("/discord")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"id":"m8","author":{"id":"user-prefix-a","username":"prefix-a"},"content":"prefix match","channel_id":"review-room-9"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(matched.status(), StatusCode::OK);

    let fallback = app
        .clone()
        .oneshot(
            Request::post("/discord")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"id":"m9","author":{"id":"user-prefix-b","username":"prefix-b"},"content":"prefix default","channel_id":"general-room-2"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fallback.status(), StatusCode::OK);

    let sent = sent_messages.lock().unwrap().clone();
    assert!(sent.contains(&(
        DISCORD_CHANNEL_ID.to_string(),
        "review-room-9".to_string(),
        "review-prefix:prefix match".to_string(),
    )));
    assert!(sent.contains(&(
        DISCORD_CHANNEL_ID.to_string(),
        "general-room-2".to_string(),
        "discord:prefix default".to_string(),
    )));
}

#[tokio::test]
async fn readiness_reviewer_enforces_max_session_messages() {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = review_manager(sent_messages);
    let app = create_bot_router_with_config(
        agentim.clone(),
        BotServerConfig {
            max_session_messages: Some(2),
            ..BotServerConfig::default()
        },
    );

    let first = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"update_id":6,"message":{"message_id":60,"chat":{"id":321},"text":"first turn"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"update_id":7,"message":{"message_id":70,"chat":{"id":321},"text":"second turn"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);

    let sessions = agentim.list_sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].messages.len(), 2);
    assert_eq!(sessions[0].messages[0].content, "second turn");
    assert_eq!(sessions[0].messages[1].content, "default:second turn");
}

#[tokio::test]
async fn readiness_reviewer_preserves_system_messages_when_trimming_history() {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = review_manager(sent_messages);
    let session_id = agentim
        .create_session(
            "default-agent".to_string(),
            TELEGRAM_CHANNEL_ID.to_string(),
            "2020".to_string(),
        )
        .unwrap();
    let mut session = agentim.get_session(&session_id).unwrap();
    session.add_message(agentim::session::MessageRole::System, "system prompt".to_string());
    agentim.update_session(&session_id, session).unwrap();

    let app = create_bot_router_with_config(
        agentim.clone(),
        BotServerConfig {
            max_session_messages: Some(3),
            ..BotServerConfig::default()
        },
    );

    let first = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"update_id":20,"message":{"message_id":200,"chat":{"id":2020},"text":"first"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"update_id":21,"message":{"message_id":210,"chat":{"id":2020},"text":"second"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);

    let trimmed = agentim.get_session(&session_id).unwrap();
    assert_eq!(trimmed.messages.len(), 3);
    assert_eq!(trimmed.messages[0].role, agentim::session::MessageRole::System);
    assert_eq!(trimmed.messages[1].content, "second");
    assert_eq!(trimmed.messages[2].content, "default:second");
}

#[tokio::test]
async fn readiness_reviewer_persists_sessions_between_restarts() {
    let state_file = temp_state_file();
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = review_manager(sent_messages);
    let app = create_bot_router_with_config(
        agentim,
        BotServerConfig {
            state_file: Some(state_file.clone()),
            ..BotServerConfig::default()
        },
    );

    let response = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"update_id":9,"message":{"message_id":90,"chat":{"id":999},"text":"persist me"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let restored_manager = Arc::new(AgentIM::new());
    register_review_agent(&restored_manager, "default-agent", "default");
    register_review_channel(
        &restored_manager,
        Arc::new(Mutex::new(Vec::new())),
        TELEGRAM_CHANNEL_ID,
        ChannelType::Telegram,
    );

    let restored = restored_manager.load_sessions_from_path(&state_file).unwrap();
    assert_eq!(restored, 1);
    assert_eq!(restored_manager.list_sessions().len(), 1);
    assert_eq!(restored_manager.list_sessions()[0].messages.len(), 2);

    let _ = std::fs::remove_file(state_file);
}

#[tokio::test]
async fn persistence_reviewer_writes_clean_snapshot_without_temp_artifacts() {
    let state_file = temp_state_file();
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = review_manager(sent_messages);
    let app = create_bot_router_with_config(
        agentim,
        BotServerConfig {
            state_file: Some(state_file.clone()),
            ..BotServerConfig::default()
        },
    );

    let response = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"update_id":10,"message":{"message_id":100,"chat":{"id":1000},"text":"save cleanly"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let snapshot = std::fs::read_to_string(&state_file).unwrap();
    let sessions: serde_json::Value = serde_json::from_str(&snapshot).unwrap();
    assert!(sessions.is_array());

    let temp_path = std::path::Path::new(&state_file)
        .with_extension(format!("{}.tmp", std::process::id()));
    assert!(!temp_path.exists());

    let _ = std::fs::remove_file(state_file);
}

#[tokio::test]
async fn security_reviewer_rejects_missing_secret_and_accepts_valid_secret() {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = review_manager(sent_messages);
    let app = create_bot_router_with_config(
        agentim,
        BotServerConfig {
            webhook_secret: Some("top-secret".to_string()),
            ..BotServerConfig::default()
        },
    );

    let unauthorized = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"update_id":11,"message":{"message_id":110,"chat":{"id":11},"text":"no secret"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let wrong_secret = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .header("x-agentim-secret", "wrong")
                .body(Body::from(
                    r#"{"update_id":12,"message":{"message_id":120,"chat":{"id":12},"text":"wrong secret"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong_secret.status(), StatusCode::UNAUTHORIZED);

    let authorized = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .header("x-agentim-secret", "top-secret")
                .body(Body::from(
                    r#"{"update_id":13,"message":{"message_id":130,"chat":{"id":13},"text":"good secret"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(authorized.status(), StatusCode::OK);
}

#[tokio::test]
async fn security_reviewer_rejects_invalid_signed_webhooks_and_replay() {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = review_manager(sent_messages);
    let app = create_bot_router_with_config(
        agentim,
        BotServerConfig {
            webhook_signing_secret: Some("signed-secret".to_string()),
            webhook_max_skew_seconds: 300,
            ..BotServerConfig::default()
        },
    );

    let body = r#"{"update_id":14,"message":{"message_id":140,"chat":{"id":14},"text":"signed hello"}}"#;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let missing_signature = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_signature.status(), StatusCode::UNAUTHORIZED);

    let invalid_signature = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .header("x-agentim-timestamp", now.to_string())
                .header("x-agentim-nonce", "nonce-bad")
                .header("x-agentim-signature", "sha256=deadbeef")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_signature.status(), StatusCode::UNAUTHORIZED);

    let headers = signed_headers("signed-secret", body, now, "nonce-good");
    let valid_request = headers.iter().fold(
        Request::post("/telegram")
            .header("content-type", "application/json"),
        |req, (name, value)| req.header(name, value),
    );
    let signed_ok = app
        .clone()
        .oneshot(valid_request.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    assert_eq!(signed_ok.status(), StatusCode::OK);

    let replay_request = headers.iter().fold(
        Request::post("/telegram")
            .header("content-type", "application/json"),
        |req, (name, value)| req.header(name, value),
    );
    let replay = app
        .clone()
        .oneshot(replay_request.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn security_reviewer_accepts_telegram_secret_token_only_when_header_matches() {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = review_manager(sent_messages);
    let app = create_bot_router_with_config(
        agentim,
        BotServerConfig {
            telegram_webhook_secret_token: Some("tg-native".to_string()),
            ..BotServerConfig::default()
        },
    );

    let body = r#"{"update_id":15,"message":{"message_id":150,"chat":{"id":15},"text":"telegram native"}}"#;

    let missing = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let wrong = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .header("x-telegram-bot-api-secret-token", "wrong")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

    let ok = app
        .clone()
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .header("x-telegram-bot-api-secret-token", "tg-native")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
}

#[tokio::test]
async fn ops_reviewer_reports_runtime_status_and_review_config() {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = review_manager(sent_messages);
    agentim
        .handle_incoming_message(
            "default-agent",
            TELEGRAM_CHANNEL_ID,
            "status-user",
            Some("status-user"),
            "status ping".to_string(),
        )
        .await
        .unwrap();

    let app = create_bot_router_with_config(
        agentim,
        BotServerConfig {
            telegram_agent_id: "default-agent".to_string(),
            max_session_messages: Some(6),
            state_file: Some("/tmp/agentim-status.json".to_string()),
            webhook_secret: Some("inspect".to_string()),
            webhook_signing_secret: Some("signed-inspect".to_string()),
            webhook_max_skew_seconds: 120,
            telegram_webhook_secret_token: Some("tg-inspect".to_string()),
            ..BotServerConfig::default()
        },
    );

    let health = app
        .clone()
        .oneshot(
            Request::get("/healthz")
                .header("x-agentim-secret", "inspect")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    let health_bytes = to_bytes(health.into_body(), usize::MAX).await.unwrap();
    let health_json: serde_json::Value = serde_json::from_slice(&health_bytes).unwrap();
    assert_eq!(health_json["status"], "ok");
    assert_eq!(health_json["sessions"], 1);

    let review = app
        .clone()
        .oneshot(
            Request::get("/reviewz")
                .header("x-agentim-secret", "inspect")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(review.status(), StatusCode::OK);
    let review_bytes = to_bytes(review.into_body(), usize::MAX).await.unwrap();
    let review_json: serde_json::Value = serde_json::from_slice(&review_bytes).unwrap();
    assert_eq!(review_json["sessions"], 1);
    assert_eq!(review_json["platform_agents"]["telegram"], "default-agent");
    assert_eq!(review_json["max_session_messages"], 6);
    assert_eq!(review_json["persistence_enabled"], true);
    assert_eq!(review_json["webhook_secret_enabled"], true);
    assert_eq!(review_json["webhook_signing_enabled"], true);
    assert_eq!(review_json["webhook_max_skew_seconds"], 120);
    assert_eq!(review_json["telegram_webhook_secret_token_enabled"], true);
}

#[test]
fn usability_reviewer_binary_dry_run_exits_cleanly() {
    let state_file = temp_state_file();
    let output = Command::new(env!("CARGO_BIN_EXE_agentim"))
        .args([
            "--dry-run",
            "--agent",
            "claude",
            "--telegram-agent",
            "pi",
            "--state-file",
            &state_file,
            "--webhook-secret",
            "secret",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Dry run complete"));
}

#[test]
fn usability_reviewer_loads_runtime_config_file() {
    let config_path = temp_state_file();
    let config = r#"{
  "agent": "codex",
  "telegram_agent": "pi",
  "routing_rules": [
    {"channel": "telegram", "user_id": "vip-user", "agent": "claude"},
    {"channel": "discord", "reply_target_prefix": "review-", "agent": "pi"}
  ],
  "state_file": ".agentim/test-sessions.json",
  "max_session_messages": 4,
  "webhook_secret": "cfg-secret",
  "webhook_signing_secret": "cfg-sign",
  "webhook_max_skew_seconds": 90,
  "telegram_webhook_secret_token": "tg-config",
  "addr": "127.0.0.1:9090"
}"#;
    std::fs::write(&config_path, config).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_agentim"))
        .args(["--config-file", &config_path, "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Default agent 'codex' registered"));
    assert!(stdout.contains("Telegram traffic -> pi agent"));
    assert!(stdout.contains("Loaded 2 routing rule"));
    assert!(stdout.contains("Session history will be trimmed to 4 message"));
    assert!(stdout.contains("Signed webhook verification enabled (max skew: 90s)"));
    assert!(stdout.contains("Telegram native webhook secret token enabled"));
    assert!(stdout.contains("Dry run complete"));

    let _ = std::fs::remove_file(config_path);
}
