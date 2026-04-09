use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

struct JsonRpcHarness {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Value>,
}

impl JsonRpcHarness {
    fn spawn(command: &str, args: &[&str]) -> Option<Self> {
        let mut child = Command::new(command)
            .args(args)
            .env_remove("OPENAI_API_KEY")
            .env_remove("OPENAI_BASE_URL")
            .env_remove("OPENAI_MODEL")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .ok()?;

        let stdin = child.stdin.take()?;
        let stdout = child.stdout.take()?;
        let messages = spawn_json_reader(stdout);

        Some(Self {
            child,
            stdin,
            messages,
        })
    }

    fn send(&mut self, value: Value) {
        serde_json::to_writer(&mut self.stdin, &value).unwrap();
        self.stdin.write_all(b"\n").unwrap();
        self.stdin.flush().unwrap();
    }

    fn wait_for_response(&self, id: i64, timeout: Duration) -> (Value, Vec<Value>) {
        let deadline = Instant::now() + timeout;
        let mut extra = Vec::new();

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let message = self
                .messages
                .recv_timeout(remaining)
                .unwrap_or_else(|_| panic!("timed out waiting for response id {id}"));

            if message.get("id").and_then(Value::as_i64) == Some(id) {
                return (message, extra);
            }

            extra.push(message);
        }
    }

    fn drain_for(&self, timeout: Duration) -> Vec<Value> {
        let deadline = Instant::now() + timeout;
        let mut drained = Vec::new();

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.messages.recv_timeout(remaining) {
                Ok(message) => drained.push(message),
                Err(_) => return drained,
            }
        }
    }
}

impl Drop for JsonRpcHarness {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_json_reader(stdout: ChildStdout) -> Receiver<Value> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else {
                break;
            };
            if line.trim().is_empty() {
                continue;
            }
            let Ok(value) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if tx.send(value).is_err() {
                break;
            }
        }
    });
    rx
}

fn initialize_request() -> Value {
    json!({
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "agentim-test", "version": "0.1"}
        }
    })
}

#[test]
fn codex_app_server_starts_a_thread_without_openai_env() {
    let Some(mut harness) = JsonRpcHarness::spawn("codex", &["app-server"]) else {
        eprintln!("skipping: codex CLI not installed in PATH");
        return;
    };

    harness.send(initialize_request());
    let (initialize, _) = harness.wait_for_response(1, Duration::from_secs(5));
    assert_eq!(initialize["result"]["platformOs"], "linux");

    let cwd = std::env::current_dir().unwrap();
    harness.send(json!({
        "id": 2,
        "method": "thread/start",
        "params": {"cwd": cwd}
    }));

    let (response, mut notifications) = harness.wait_for_response(2, Duration::from_secs(10));
    notifications.extend(harness.drain_for(Duration::from_millis(500)));
    let thread = &response["result"]["thread"];
    assert!(thread["id"].as_str().is_some_and(|id| !id.is_empty()));
    assert_eq!(
        thread["cwd"],
        Value::String(std::env::current_dir().unwrap().display().to_string())
    );
    assert!(notifications.iter().any(|message| {
        message.get("method") == Some(&Value::String("thread/started".to_string()))
    }));
}

#[test]
fn codex_app_server_rejects_legacy_acp_session_methods() {
    let Some(mut harness) = JsonRpcHarness::spawn("codex", &["app-server"]) else {
        eprintln!("skipping: codex CLI not installed in PATH");
        return;
    };

    harness.send(initialize_request());
    let _ = harness.wait_for_response(1, Duration::from_secs(5));

    harness.send(json!({
        "id": 2,
        "method": "session/new",
        "params": {"cwd": std::env::current_dir().unwrap()}
    }));

    let (response, _) = harness.wait_for_response(2, Duration::from_secs(5));
    let message = response["error"]["message"].as_str().unwrap_or_default();
    assert!(message.contains("session/new"));
    assert!(message.contains("thread/start"));
}
