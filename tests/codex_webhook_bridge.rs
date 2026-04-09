use agentim::bot_server::create_bot_router;
use agentim::bots::TELEGRAM_CHANNEL_ID;
use agentim::channel::{Channel, ChannelMessage};
use agentim::codex::{CodexAgent, CodexBackendConfig};
use agentim::config::ChannelType;
use agentim::manager::AgentIM;
use agentim::Result;
use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;

struct CaptureChannel {
    id: String,
    sent_messages: Arc<Mutex<Vec<(String, String)>>>,
}

#[async_trait]
impl Channel for CaptureChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Telegram
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        self.sent_messages
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

fn temp_python_script(name: &str, content: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("agentim-{name}-{nanos}.py"));
    std::fs::write(&path, content).unwrap();
    path
}

#[tokio::test]
async fn telegram_webhook_bridges_to_codex_app_server_backend() {
    let script = temp_python_script(
        "mock-codex-app-server",
        r#"#!/usr/bin/env python3
import json
import sys

next_thread = 0
threads = set()

for raw in sys.stdin:
    raw = raw.strip()
    if not raw:
        continue
    request = json.loads(raw)
    req_id = request.get("id")
    method = request.get("method")
    params = request.get("params") or {}

    if method == "initialize":
        print(json.dumps({
            "id": req_id,
            "result": {
                "platformOs": "linux",
                "userAgent": "mock-codex/0.1"
            }
        }), flush=True)
    elif method == "thread/start":
        next_thread += 1
        thread_id = f"thread-{next_thread}"
        threads.add(thread_id)
        print(json.dumps({
            "id": req_id,
            "result": {
                "thread": {
                    "id": thread_id,
                    "path": f"/tmp/{thread_id}.jsonl"
                }
            }
        }), flush=True)
    elif method == "thread/resume":
        thread_id = params["threadId"]
        if thread_id not in threads:
            print(json.dumps({
                "id": req_id,
                "error": {
                    "code": -32600,
                    "message": f"no rollout found for thread id {thread_id}"
                }
            }), flush=True)
        else:
            print(json.dumps({
                "id": req_id,
                "result": {
                    "thread": {
                        "id": thread_id,
                        "path": f"/tmp/{thread_id}.jsonl"
                    }
                }
            }), flush=True)
    elif method == "turn/start":
        thread_id = params["threadId"]
        turn_id = f"turn-{thread_id}"
        text = params["input"][0]["text"]
        reply = f"codex:{thread_id}:{text}"
        print(json.dumps({
            "id": req_id,
            "result": {
                "turn": {
                    "id": turn_id,
                    "status": "inProgress",
                    "items": []
                }
            }
        }), flush=True)
        print(json.dumps({
            "method": "item/agentMessage/delta",
            "params": {
                "threadId": thread_id,
                "turnId": turn_id,
                "delta": reply
            }
        }), flush=True)
        print(json.dumps({
            "method": "turn/completed",
            "params": {
                "threadId": thread_id,
                "turn": {
                    "id": turn_id,
                    "status": "completed",
                    "error": None
                }
            }
        }), flush=True)
    else:
        print(json.dumps({
            "id": req_id,
            "error": {
                "code": -32601,
                "message": f"method not found: {method}"
            }
        }), flush=True)
"#,
    );

    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let agentim = Arc::new(AgentIM::new());
    agentim
        .register_agent(
            "default-agent".to_string(),
            Arc::new(CodexAgent::new(
                "default-agent".to_string(),
                CodexBackendConfig {
                    command: "python3".to_string(),
                    args: vec![script.display().to_string()],
                    cwd: std::env::current_dir().unwrap(),
                    env: HashMap::new(),
                },
            )),
        )
        .unwrap();
    agentim
        .register_channel(
            TELEGRAM_CHANNEL_ID.to_string(),
            Arc::new(CaptureChannel {
                id: TELEGRAM_CHANNEL_ID.to_string(),
                sent_messages: sent_messages.clone(),
            }),
        )
        .unwrap();

    let app = create_bot_router(agentim.clone());
    let response = app
        .oneshot(
            Request::post("/telegram")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"update_id":501,"message":{"message_id":5001,"chat":{"id":4242},"text":"hello codex"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let sent = sent_messages.lock().unwrap().clone();
    assert_eq!(
        sent,
        vec![("4242".to_string(), "codex:thread-1:hello codex".to_string())]
    );

    let sessions = agentim.list_sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(
        sessions[0].metadata.get("codex_thread_id"),
        Some(&"thread-1".to_string())
    );

    let _ = std::fs::remove_file(script);
}
