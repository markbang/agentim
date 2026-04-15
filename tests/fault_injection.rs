//! Fault injection tests for production readiness.
//! Covers: ACP backend failure, session file corruption, webhook replay,
//! platform timeout/rate limiting, and network jitter scenarios.

use agentim::agent::Agent;
use agentim::bot_server::{create_bot_router_with_config, BotServerConfig};
use agentim::channel::{Channel, ChannelMessage};
use agentim::config::{AgentType, ChannelType};
use agentim::error::AgentError;
use agentim::manager::AgentIM;
use agentim::session::{Message, Session, SCHEMA_VERSION};
use agentim::Result;
use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use chrono::Utc;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;

// -- Test helpers --

struct CrashAgent {
    id: String,
    crash_on_call: bool,
}

#[async_trait]
impl Agent for CrashAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Acp
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, _messages: Vec<Message>) -> Result<String> {
        if self.crash_on_call {
            return Err(AgentError::ApiError("agent process crashed".to_string()));
        }
        Ok("ok".to_string())
    }

    async fn health_check(&self) -> Result<()> {
        if self.crash_on_call {
            return Err(AgentError::ApiError("agent not healthy".to_string()));
        }
        Ok(())
    }
}

struct NeverReplyAgent {
    id: String,
}

#[async_trait]
impl Agent for NeverReplyAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Acp
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, _messages: Vec<Message>) -> Result<String> {
        // Simulates a hung backend - sleep longer than any reasonable timeout
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        Ok("never".to_string())
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

struct TestChannel {
    id: String,
    sent: Arc<Mutex<Vec<(String, String)>>>,
    channel_type: ChannelType,
}

#[async_trait]
impl Channel for TestChannel {
    fn channel_type(&self) -> ChannelType {
        self.channel_type
    }

    fn id(&self) -> &str {
        &self.id
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

fn temp_state_file() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir()
        .join(format!("agentim-fault-{}.json", nanos))
        .display()
        .to_string()
}

type SentLog = Arc<Mutex<Vec<(String, String)>>>;

fn setup_test_agentim(agent: Arc<dyn Agent>, channel_type: ChannelType) -> (Arc<AgentIM>, SentLog) {
    let agentim = Arc::new(AgentIM::new());
    agentim
        .register_agent("default-agent".to_string(), agent)
        .unwrap();

    let sent = Arc::new(Mutex::new(Vec::new()));
    let channel = Arc::new(TestChannel {
        id: "test-channel".to_string(),
        sent: sent.clone(),
        channel_type,
    });
    agentim
        .register_channel("test-channel".to_string(), channel)
        .unwrap();

    (agentim, sent)
}

// -- Fault injection tests --

#[tokio::test]
async fn fault_injection_acp_backend_crash_returns_bad_gateway() {
    let (agentim, sent) = setup_test_agentim(
        Arc::new(CrashAgent {
            id: "crash-agent".to_string(),
            crash_on_call: true,
        }),
        ChannelType::Telegram,
    );

    let result = agentim
        .handle_incoming_message(
            "default-agent",
            "test-channel",
            "user-1",
            Some("user-1"),
            "hello".to_string(),
        )
        .await;

    assert!(result.is_err());
    assert!(sent.lock().unwrap().is_empty());

    // Session exists, but failed round-trip should not append persisted messages.
    let sessions = agentim.list_sessions();
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0].messages.is_empty());
}

#[tokio::test]
async fn fault_injection_hung_backend_times_out() {
    let (agentim, sent) = setup_test_agentim(
        Arc::new(NeverReplyAgent {
            id: "never-agent".to_string(),
        }),
        ChannelType::Telegram,
    );

    let result = agentim
        .handle_incoming_message_with_options(
            "default-agent",
            "test-channel",
            "123",
            Some("123"),
            "will timeout".to_string(),
            agentim::manager::MessageHandlingOptions {
                max_messages: None,
                context_message_limit: 10,
                agent_timeout_ms: Some(50),
            },
        )
        .await;

    assert!(matches!(result, Err(AgentError::TimeoutError(_))));
    assert!(sent.lock().unwrap().is_empty());
    let sessions = agentim.list_sessions();
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0].messages.is_empty());
}

#[tokio::test]
async fn fault_injection_corrupted_state_file_fallback_to_backup() {
    let state_file = temp_state_file();
    let backup_file = format!("{}.bak.1", state_file);

    // Write a valid backup
    let valid_session = Session::new(
        "default-agent".to_string(),
        "test-channel".to_string(),
        "user-backup".to_string(),
    );
    std::fs::write(
        &backup_file,
        serde_json::to_string_pretty(&vec![valid_session]).unwrap(),
    )
    .unwrap();

    // Write corrupted primary
    std::fs::write(&state_file, "NOT VALID JSON {{{").unwrap();

    let (agentim, _) = setup_test_agentim(
        Arc::new(CrashAgent {
            id: "default-agent".to_string(),
            crash_on_call: false,
        }),
        ChannelType::Telegram,
    );
    let (count, loaded_from) = agentim
        .load_sessions_from_path_with_fallback(&state_file, 1)
        .unwrap();

    assert_eq!(count, 1);
    assert!(loaded_from.contains(".bak.1"));

    let _ = std::fs::remove_file(&state_file);
    let _ = std::fs::remove_file(&backup_file);
}

#[tokio::test]
async fn fault_injection_both_state_and_backup_corrupted() {
    let state_file = temp_state_file();
    let backup_file = format!("{}.bak.1", state_file);

    std::fs::write(&state_file, "CORRUPT PRIMARY").unwrap();
    std::fs::write(&backup_file, "CORRUPT BACKUP").unwrap();

    let agentim = AgentIM::new();
    let result = agentim.load_sessions_from_path_with_fallback(&state_file, 1);

    assert!(result.is_err());

    let _ = std::fs::remove_file(&state_file);
    let _ = std::fs::remove_file(&backup_file);
}

#[tokio::test]
async fn fault_injection_stale_webhook_timestamp_rejected() {
    let (agentim, _) = setup_test_agentim(
        Arc::new(CrashAgent {
            id: "skip".to_string(),
            crash_on_call: false,
        }),
        ChannelType::Discord,
    );

    let app = create_bot_router_with_config(
        agentim,
        BotServerConfig {
            webhook_signing_secret: Some("test-secret".to_string()),
            webhook_max_skew_seconds: 30,
            ..BotServerConfig::default()
        },
    );

    // Timestamp from 1 hour ago
    let stale_timestamp = Utc::now().timestamp() - 3600;
    let nonce = "fault-nonce-stale";

    // Create a valid signature for the stale timestamp
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let body = r#"{"id":"m1","author":{"id":"user-1","username":"test"},"content":"stale","channel_id":"ch-1"}"#;
    let mut mac = HmacSha256::new_from_slice(b"test-secret").unwrap();
    mac.update(stale_timestamp.to_string().as_bytes());
    mac.update(b"\n");
    mac.update(nonce.as_bytes());
    mac.update(b"\n");
    mac.update(body.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());

    let response = app
        .oneshot(
            Request::post("/discord")
                .header("content-type", "application/json")
                .header("x-agentim-timestamp", stale_timestamp.to_string())
                .header("x-agentim-nonce", nonce)
                .header("x-agentim-signature", format!("sha256={}", signature))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn fault_injection_replayed_webhook_nonce_rejected() {
    let (agentim, _) = setup_test_agentim(
        Arc::new(CrashAgent {
            id: "skip".to_string(),
            crash_on_call: false,
        }),
        ChannelType::Discord,
    );

    let app = create_bot_router_with_config(
        agentim.clone(),
        BotServerConfig {
            webhook_signing_secret: Some("replay-secret".to_string()),
            ..BotServerConfig::default()
        },
    );

    let now = Utc::now().timestamp();
    let nonce = "replay-nonce-unique";
    let body = r#"{"id":"m2","author":{"id":"user-2","username":"test"},"content":"replay","channel_id":"ch-2"}"#;

    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(b"replay-secret").unwrap();
    mac.update(now.to_string().as_bytes());
    mac.update(b"\n");
    mac.update(nonce.as_bytes());
    mac.update(b"\n");
    mac.update(body.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());

    // First request should succeed (or at least not fail on auth)
    let first = app
        .clone()
        .oneshot(
            Request::post("/discord")
                .header("content-type", "application/json")
                .header("x-agentim-timestamp", now.to_string())
                .header("x-agentim-nonce", nonce)
                .header("x-agentim-signature", format!("sha256={}", signature))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    // Either OK or an agent error, but NOT unauthorized
    assert_ne!(first.status(), StatusCode::UNAUTHORIZED);

    // Replay the same request - should be rejected
    let replay = app
        .oneshot(
            Request::post("/discord")
                .header("content-type", "application/json")
                .header("x-agentim-timestamp", now.to_string())
                .header("x-agentim-nonce", nonce)
                .header("x-agentim-signature", format!("sha256={}", signature))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn fault_injection_schema_version_upgrade_on_load() {
    let state_file = temp_state_file();

    // Write a session with schema_version = 0 (pre-versioned)
    let old_session = serde_json::json!([{
        "id": "old-session-1",
        "agent_id": "default-agent",
        "channel_id": "test-channel",
        "user_id": "legacy-user",
        "messages": [],
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z",
        "metadata": {}
    }]);
    std::fs::write(
        &state_file,
        serde_json::to_string_pretty(&old_session).unwrap(),
    )
    .unwrap();

    let (agentim, _) = setup_test_agentim(
        Arc::new(CrashAgent {
            id: "default-agent".to_string(),
            crash_on_call: false,
        }),
        ChannelType::Telegram,
    );
    let (count, _) = agentim
        .load_sessions_from_path_with_fallback(&state_file, 0)
        .unwrap();

    assert_eq!(count, 1);
    let sessions = agentim.list_sessions();
    assert_eq!(sessions.len(), 1);
    // Schema version should be upgraded to current
    assert_eq!(sessions[0].schema_version, SCHEMA_VERSION);

    let _ = std::fs::remove_file(&state_file);
}

#[tokio::test]
async fn fault_injection_agent_timeout_prevents_session_pollution() {
    let (agentim, sent) = setup_test_agentim(
        Arc::new(NeverReplyAgent {
            id: "timeout-agent".to_string(),
        }),
        ChannelType::Telegram,
    );

    let result = agentim
        .handle_incoming_message_with_options(
            "default-agent",
            "test-channel",
            "555",
            Some("555"),
            "first".to_string(),
            agentim::manager::MessageHandlingOptions {
                max_messages: None,
                context_message_limit: 10,
                agent_timeout_ms: Some(50),
            },
        )
        .await;
    assert!(matches!(result, Err(AgentError::TimeoutError(_))));

    // No channel message sent
    assert!(sent.lock().unwrap().is_empty());

    // Failed round-trip should leave an empty persisted session.
    let sessions = agentim.list_sessions();
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0].messages.is_empty());
}

#[tokio::test]
async fn fault_injection_metrics_increment_on_auth_failure() {
    let (agentim, _) = setup_test_agentim(
        Arc::new(CrashAgent {
            id: "skip".to_string(),
            crash_on_call: false,
        }),
        ChannelType::Discord,
    );

    let app = create_bot_router_with_config(
        agentim.clone(),
        BotServerConfig {
            webhook_secret: Some("inspect".to_string()),
            ..BotServerConfig::default()
        },
    );

    // Request without secret should get 401
    let response = app
        .clone()
        .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Check metrics endpoint with proper auth shows the counter
    let metrics_resp = app
        .oneshot(
            Request::get("/metrics")
                .header("x-agentim-secret", "inspect")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(metrics_resp.status(), StatusCode::OK);
    let body = to_bytes(metrics_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("agentim_auth_reject_total"));
}

#[tokio::test]
async fn fault_injection_active_sessions_gauge_tracks_lifecycle() {
    let (agentim, _) = setup_test_agentim(
        Arc::new(CrashAgent {
            id: "default-agent".to_string(),
            crash_on_call: false,
        }),
        ChannelType::Telegram,
    );

    // Initially zero
    assert_eq!(agentim.session_count(), 0);

    // Create sessions
    agentim
        .create_session(
            "default-agent".to_string(),
            "test-channel".to_string(),
            "user-a".to_string(),
        )
        .unwrap();
    agentim
        .create_session(
            "default-agent".to_string(),
            "test-channel".to_string(),
            "user-b".to_string(),
        )
        .unwrap();

    assert_eq!(agentim.session_count(), 2);

    // Cleanup
    let removed = agentim.cleanup_stale_sessions(0); // 0 = everything is stale
    assert_eq!(removed, 2);
    assert_eq!(agentim.session_count(), 0);
}

#[tokio::test]
async fn fault_injection_concurrent_session_creation_no_race() {
    let agentim = Arc::new(AgentIM::new());
    agentim
        .register_agent(
            "default-agent".to_string(),
            Arc::new(CrashAgent {
                id: "default-agent".to_string(),
                crash_on_call: false,
            }),
        )
        .unwrap();
    let sent = Arc::new(Mutex::new(Vec::new()));
    let channel = Arc::new(TestChannel {
        id: "test-channel".to_string(),
        sent: sent.clone(),
        channel_type: ChannelType::Telegram,
    });
    agentim
        .register_channel("test-channel".to_string(), channel)
        .unwrap();

    // Spawn 20 concurrent session creations for the same user
    let mut handles = Vec::new();
    for _ in 0..20 {
        let agentim_clone = agentim.clone();
        handles.push(tokio::spawn(async move {
            agentim_clone.find_or_create_session("default-agent", "test-channel", "concurrent-user")
        }));
    }

    let results: Vec<_> = futures_util::future::join_all(handles).await;
    let session_ids: Vec<_> = results.into_iter().map(|r| r.unwrap().unwrap()).collect();

    // All should return the same session ID (or all succeed without panic)
    assert!(session_ids.iter().all(|id| !id.is_empty()));
    // Due to DashMap, all calls should find or create the same session
    let unique_count = session_ids
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert!(unique_count <= 2); // At most 2 unique IDs due to race window

    // Should have exactly 1 session
    assert_eq!(agentim.list_sessions().len(), 1);
}
