use agentim::agent::Agent;
use agentim::bot_server::create_bot_router;
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
use tower::ServiceExt;

struct ReviewAgent;

#[async_trait]
impl Agent for ReviewAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Claude
    }

    fn id(&self) -> &str {
        "default-agent"
    }

    async fn send_message(&self, messages: Vec<Message>) -> Result<String> {
        let last = messages.last().map(|msg| msg.content.clone()).unwrap_or_default();
        Ok(format!("reviewed:{}", last))
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

fn review_manager(sent_messages: Arc<Mutex<Vec<(String, String, String)>>>) -> Arc<AgentIM> {
    let agentim = Arc::new(AgentIM::new());
    agentim
        .register_agent("default-agent".to_string(), Arc::new(ReviewAgent))
        .unwrap();

    for (id, channel_type) in [
        (TELEGRAM_CHANNEL_ID, ChannelType::Telegram),
        (DISCORD_CHANNEL_ID, ChannelType::Discord),
        (FEISHU_CHANNEL_ID, ChannelType::Feishu),
        (QQ_CHANNEL_ID, ChannelType::QQ),
    ] {
        agentim
            .register_channel(
                id.to_string(),
                Arc::new(ReviewChannel {
                    id: id.to_string(),
                    sent_messages: sent_messages.clone(),
                    channel_type,
                }),
            )
            .unwrap();
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
        "reviewed:ping discord".to_string(),
    )));
    assert!(sent.contains(&(
        QQ_CHANNEL_ID.to_string(),
        "qq-channel".to_string(),
        "reviewed:ping qq".to_string(),
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
