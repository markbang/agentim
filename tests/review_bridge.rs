use agentim::agent::Agent;
use agentim::bot_server::{create_bot_router, create_bot_router_with_config, BotServerConfig};
use agentim::bots::{DISCORD_CHANNEL_ID, FEISHU_CHANNEL_ID, QQ_CHANNEL_ID, TELEGRAM_CHANNEL_ID};
use agentim::channel::{Channel, ChannelMessage};
use agentim::config::{AgentType, ChannelType};
use agentim::manager::AgentIM;
use agentim::session::Message;
use agentim::Result;
use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
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
