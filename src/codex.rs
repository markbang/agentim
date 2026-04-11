use crate::agent::Agent;
use crate::config::AgentType;
use crate::error::{AgentError, Result};
use crate::session::{Message, MessageRole, Session};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use toml::Value as TomlValue;
use tracing::{debug, info, warn};

const CODEX_THREAD_ID_METADATA_KEY: &str = "codex_thread_id";
const CODEX_BACKEND_METADATA_KEY: &str = "codex_backend";
const CODEX_THREAD_PATH_METADATA_KEY: &str = "codex_thread_path";
const CODEX_PROTOCOL_VERSION: &str = "2025-03-26";

const INITIALIZE_METHOD: &str = "initialize";
const THREAD_START_METHOD: &str = "thread/start";
const THREAD_RESUME_METHOD: &str = "thread/resume";
const TURN_START_METHOD: &str = "turn/start";

const TURN_COMPLETED_NOTIFICATION: &str = "turn/completed";
const AGENT_MESSAGE_DELTA_NOTIFICATION: &str = "item/agentMessage/delta";
const ITEM_COMPLETED_NOTIFICATION: &str = "item/completed";
const ERROR_NOTIFICATION: &str = "error";

type BoxRead = Pin<Box<dyn AsyncRead + Send>>;
type BoxWrite = Pin<Box<dyn AsyncWrite + Send>>;

#[derive(Debug, Clone)]
pub struct CodexBackendConfig {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
}

impl CodexBackendConfig {
    pub fn describe(&self) -> String {
        if self.args.is_empty() {
            self.command.clone()
        } else {
            format!("{} {}", self.command, self.args.join(" "))
        }
    }
}

pub struct CodexAgent {
    id: String,
    client: Arc<CodexSessionClient>,
}

impl CodexAgent {
    pub fn new(id: String, config: CodexBackendConfig) -> Self {
        let session_cwd = config.cwd.clone();
        let factory: Arc<dyn CodexTransportFactory> =
            Arc::new(ProcessCodexTransportFactory { config });
        Self::from_factory(id, factory, session_cwd)
    }

    fn from_factory(
        id: String,
        factory: Arc<dyn CodexTransportFactory>,
        session_cwd: PathBuf,
    ) -> Self {
        let backend_description = factory.describe();
        Self {
            id,
            client: Arc::new(CodexSessionClient {
                factory,
                state: Mutex::new(None),
                session_cwd,
                backend_description,
            }),
        }
    }
}

#[async_trait]
impl Agent for CodexAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Codex
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, _messages: Vec<Message>) -> Result<String> {
        Err(AgentError::ConfigError(
            "Codex agents require session-aware dispatch".to_string(),
        ))
    }

    async fn send_message_with_session(
        &self,
        session: &mut Session,
        messages: Vec<Message>,
    ) -> Result<String> {
        self.client.send_message(session, &messages).await
    }

    async fn health_check(&self) -> Result<()> {
        self.client.ensure_ready().await
    }
}

struct CodexSessionClient {
    factory: Arc<dyn CodexTransportFactory>,
    state: Mutex<Option<CodexClientState>>,
    session_cwd: PathBuf,
    backend_description: String,
}

impl CodexSessionClient {
    async fn ensure_ready(&self) -> Result<()> {
        let mut state = self.state.lock().await;
        self.ensure_connected(&mut state).await?;
        Ok(())
    }

    async fn send_message(&self, session: &mut Session, messages: &[Message]) -> Result<String> {
        match self.send_message_once(session, messages).await {
            Ok(response) => Ok(response),
            Err(error) if error.is_transport() => {
                warn!(
                    agent_id = %session.agent_id,
                    session_id = %session.id,
                    error = %error,
                    "Codex transport failed; reconnecting once"
                );
                self.reset().await;
                self.send_message_once(session, messages)
                    .await
                    .map_err(Into::into)
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn send_message_once(
        &self,
        session: &mut Session,
        messages: &[Message],
    ) -> std::result::Result<String, CodexClientError> {
        let mut state = self.state.lock().await;
        self.ensure_connected(&mut state).await?;
        let state = state.as_mut().expect("codex state initialized");

        let remote_thread = state
            .ensure_remote_thread(session, &self.session_cwd, &self.backend_description)
            .await?;

        let prompt_text = if remote_thread.is_fresh {
            build_bootstrap_prompt(messages)
        } else {
            latest_user_message(messages)
        };

        state
            .prompt_thread(session, &remote_thread.id, prompt_text)
            .await
    }

    async fn ensure_connected(
        &self,
        state: &mut Option<CodexClientState>,
    ) -> std::result::Result<(), CodexClientError> {
        let needs_connect = match state.as_mut() {
            Some(existing) => existing.transport_has_exited()?,
            None => true,
        };

        if needs_connect {
            *state = Some(CodexClientState::connect(self.factory.clone()).await?);
        }

        Ok(())
    }

    async fn reset(&self) {
        let mut state = self.state.lock().await;
        *state = None;
    }
}

struct CodexClientState {
    transport: CodexTransport,
    next_request_id: i64,
    remote_threads: HashMap<String, String>,
}

impl CodexClientState {
    async fn connect(
        factory: Arc<dyn CodexTransportFactory>,
    ) -> std::result::Result<Self, CodexClientError> {
        let mut state = Self {
            transport: factory.connect().await?,
            next_request_id: 0,
            remote_threads: HashMap::new(),
        };

        let response = state
            .request(
                INITIALIZE_METHOD,
                &serde_json::json!({
                    "protocolVersion": CODEX_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {
                        "name": "agentim",
                        "version": env!("CARGO_PKG_VERSION"),
                    }
                }),
            )
            .await?;

        let platform = response
            .get("platformOs")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        info!(platform = %platform, "Codex app-server initialized");
        Ok(state)
    }

    fn transport_has_exited(&mut self) -> std::result::Result<bool, CodexClientError> {
        self.transport.has_exited()
    }

    async fn ensure_remote_thread(
        &mut self,
        session: &mut Session,
        session_cwd: &Path,
        backend_description: &str,
    ) -> std::result::Result<RemoteThread, CodexClientError> {
        if let Some(thread_id) = self.remote_threads.get(&session.id) {
            return Ok(RemoteThread {
                id: thread_id.clone(),
                is_fresh: false,
            });
        }

        if let Some(saved_thread_id) = session.metadata.get(CODEX_THREAD_ID_METADATA_KEY).cloned() {
            match self
                .request(
                    THREAD_RESUME_METHOD,
                    &serde_json::json!({
                        "threadId": saved_thread_id,
                        "cwd": session_cwd,
                    }),
                )
                .await
            {
                Ok(result) => {
                    let thread = parse_thread(&result)?;
                    self.remote_threads
                        .insert(session.id.clone(), thread.id.clone());
                    apply_session_metadata(session, backend_description, &thread);
                    debug!(
                        session_id = %session.id,
                        thread_id = %thread.id,
                        "Codex thread resumed"
                    );
                    return Ok(RemoteThread {
                        id: thread.id,
                        is_fresh: false,
                    });
                }
                Err(error) if error.is_thread_not_found() => {
                    warn!(
                        session_id = %session.id,
                        thread_id = %saved_thread_id,
                        error = %error,
                        "Saved Codex thread is unavailable; creating a new thread"
                    );
                }
                Err(error) => return Err(error),
            }
        }

        self.create_remote_thread(session, session_cwd, backend_description)
            .await
    }

    async fn create_remote_thread(
        &mut self,
        session: &mut Session,
        session_cwd: &Path,
        backend_description: &str,
    ) -> std::result::Result<RemoteThread, CodexClientError> {
        let result = self
            .request(
                THREAD_START_METHOD,
                &serde_json::json!({
                    "cwd": session_cwd,
                }),
            )
            .await?;
        let thread = parse_thread(&result)?;
        self.remote_threads
            .insert(session.id.clone(), thread.id.clone());
        apply_session_metadata(session, backend_description, &thread);
        debug!(
            session_id = %session.id,
            thread_id = %thread.id,
            "Codex thread created"
        );
        Ok(RemoteThread {
            id: thread.id,
            is_fresh: true,
        })
    }

    async fn prompt_thread(
        &mut self,
        session: &mut Session,
        thread_id: &str,
        prompt_text: String,
    ) -> std::result::Result<String, CodexClientError> {
        let result = self
            .request(
                TURN_START_METHOD,
                &serde_json::json!({
                    "threadId": thread_id,
                    "input": [
                        {
                            "type": "text",
                            "text": prompt_text,
                        }
                    ],
                }),
            )
            .await?;
        let turn_id = parse_turn_id(&result)?;
        let response = self.wait_for_turn_completion(thread_id, &turn_id).await?;
        session.metadata.insert(
            CODEX_THREAD_ID_METADATA_KEY.to_string(),
            thread_id.to_string(),
        );
        Ok(response)
    }

    async fn wait_for_turn_completion(
        &mut self,
        thread_id: &str,
        turn_id: &str,
    ) -> std::result::Result<String, CodexClientError> {
        let mut response_text = String::new();

        loop {
            let mut line = String::new();
            let bytes_read = self.transport.reader.read_line(&mut line).await?;
            if bytes_read == 0 {
                return Err(CodexClientError::TransportClosed);
            }

            let envelope: JsonRpcEnvelope = serde_json::from_str(&line)?;

            if let Some(id) = &envelope.id {
                if envelope.method.is_some() {
                    reject_server_request(&mut self.transport.writer, id.clone()).await?;
                }
                continue;
            }

            let Some(method) = envelope.method.as_deref() else {
                continue;
            };
            let params = envelope.params.unwrap_or(Value::Null);

            match method {
                AGENT_MESSAGE_DELTA_NOTIFICATION => {
                    if params.get("threadId").and_then(Value::as_str) == Some(thread_id)
                        && params.get("turnId").and_then(Value::as_str) == Some(turn_id)
                    {
                        if let Some(delta) = params.get("delta").and_then(Value::as_str) {
                            response_text.push_str(delta);
                        }
                    }
                }
                ITEM_COMPLETED_NOTIFICATION => {
                    if params.get("threadId").and_then(Value::as_str) == Some(thread_id)
                        && params.get("turnId").and_then(Value::as_str) == Some(turn_id)
                        && params
                            .get("item")
                            .and_then(|item| item.get("type"))
                            .and_then(Value::as_str)
                            == Some("agentMessage")
                    {
                        if let Some(text) = params
                            .get("item")
                            .and_then(|item| item.get("text"))
                            .and_then(Value::as_str)
                        {
                            if response_text.trim().is_empty() || text.len() >= response_text.len()
                            {
                                response_text = text.to_string();
                            }
                        }
                    }
                }
                TURN_COMPLETED_NOTIFICATION => {
                    if params.get("threadId").and_then(Value::as_str) != Some(thread_id) {
                        continue;
                    }
                    let Some(turn) = params.get("turn") else {
                        continue;
                    };
                    if turn.get("id").and_then(Value::as_str) != Some(turn_id) {
                        continue;
                    }
                    if let Some(error) = turn.get("error") {
                        if !error.is_null() {
                            return Err(CodexClientError::Protocol(format!(
                                "Codex turn failed: {}",
                                error
                            )));
                        }
                    }
                    if response_text.trim().is_empty() {
                        return Err(CodexClientError::Protocol(
                            "Codex turn completed without any text response".to_string(),
                        ));
                    }
                    return Ok(response_text);
                }
                ERROR_NOTIFICATION => {
                    if params.get("threadId").and_then(Value::as_str) == Some(thread_id)
                        && params.get("turnId").and_then(Value::as_str) == Some(turn_id)
                        && params.get("willRetry").and_then(Value::as_bool) != Some(true)
                    {
                        let message = params
                            .get("error")
                            .and_then(|error| error.get("message"))
                            .and_then(Value::as_str)
                            .unwrap_or("unknown Codex transport error");
                        return Err(CodexClientError::Protocol(message.to_string()));
                    }
                }
                _ => {}
            }
        }
    }

    async fn request<T>(
        &mut self,
        method: &str,
        params: &T,
    ) -> std::result::Result<Value, CodexClientError>
    where
        T: Serialize,
    {
        let request_id = self.next_request_id;
        self.next_request_id += 1;

        write_json_line(
            &mut self.transport.writer,
            &serde_json::json!({
                "id": request_id,
                "method": method,
                "params": params,
            }),
        )
        .await?;

        loop {
            let mut line = String::new();
            let bytes_read = self.transport.reader.read_line(&mut line).await?;
            if bytes_read == 0 {
                return Err(CodexClientError::TransportClosed);
            }

            let envelope: JsonRpcEnvelope = serde_json::from_str(&line)?;

            if let Some(id) = &envelope.id {
                if envelope.method.is_some() {
                    reject_server_request(&mut self.transport.writer, id.clone()).await?;
                    continue;
                }

                if !matches_request_id(id, request_id) {
                    continue;
                }

                if let Some(error) = envelope.error {
                    return Err(error.into());
                }

                let result = envelope.result.ok_or_else(|| {
                    CodexClientError::Protocol(format!(
                        "Codex response for '{}' is missing a result payload",
                        method
                    ))
                })?;
                return Ok(result);
            }
        }
    }
}

struct RemoteThread {
    id: String,
    is_fresh: bool,
}

#[derive(Debug, Clone)]
struct ThreadInfo {
    id: String,
    path: Option<String>,
}

struct CodexTransport {
    reader: BufReader<BoxRead>,
    writer: BoxWrite,
    child: Option<Child>,
}

impl CodexTransport {
    fn new(reader: BoxRead, writer: BoxWrite, child: Option<Child>) -> Self {
        Self {
            reader: BufReader::new(reader),
            writer,
            child,
        }
    }

    fn has_exited(&mut self) -> std::result::Result<bool, CodexClientError> {
        let Some(child) = self.child.as_mut() else {
            return Ok(false);
        };

        match child.try_wait()? {
            Some(status) => {
                warn!(status = %status, "Codex app-server subprocess exited");
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

#[async_trait]
trait CodexTransportFactory: Send + Sync {
    async fn connect(&self) -> std::result::Result<CodexTransport, CodexClientError>;
    fn describe(&self) -> String;
}

#[derive(Debug)]
struct ProcessCodexTransportFactory {
    config: CodexBackendConfig,
}

#[async_trait]
impl CodexTransportFactory for ProcessCodexTransportFactory {
    async fn connect(&self) -> std::result::Result<CodexTransport, CodexClientError> {
        let isolated_codex_home = prepare_isolated_codex_home(&self.config)?;
        let mut command = Command::new(&self.config.command);
        command
            .args(&self.config.args)
            .current_dir(&self.config.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);

        for (key, value) in &self.config.env {
            command.env(key, value);
        }
        command.env("CODEX_HOME", &isolated_codex_home);

        let mut child = command.spawn()?;
        let stdout = child.stdout.take().ok_or_else(|| {
            CodexClientError::Protocol("Codex subprocess did not expose stdout".to_string())
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            CodexClientError::Protocol("Codex subprocess did not expose stdin".to_string())
        })?;

        Ok(CodexTransport::new(
            Box::pin(stdout),
            Box::pin(stdin),
            Some(child),
        ))
    }

    fn describe(&self) -> String {
        self.config.describe()
    }
}

fn prepare_isolated_codex_home(
    config: &CodexBackendConfig,
) -> std::result::Result<PathBuf, CodexClientError> {
    let isolated_home = config.cwd.join(".omx/runtime/codex-home");
    std::fs::create_dir_all(&isolated_home)?;

    let source_home = source_codex_home();
    if let Some(source_config) = source_home
        .as_ref()
        .map(|home| home.join("config.toml"))
        .filter(|path| path.exists())
    {
        let source = std::fs::read_to_string(&source_config)?;
        let sanitized = sanitize_codex_config(&source, &config.cwd)?;
        std::fs::write(isolated_home.join("config.toml"), sanitized)?;
    } else {
        std::fs::write(
            isolated_home.join("config.toml"),
            minimal_codex_config(&config.cwd),
        )?;
    }

    if let Some(source_auth) = source_home
        .as_ref()
        .map(|home| home.join("auth.json"))
        .filter(|path| path.exists())
    {
        std::fs::copy(source_auth, isolated_home.join("auth.json"))?;
    }

    Ok(isolated_home)
}

fn source_codex_home() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
}

fn sanitize_codex_config(
    source: &str,
    cwd: &Path,
) -> std::result::Result<String, CodexClientError> {
    let parsed = source
        .parse::<TomlValue>()
        .map_err(|err| CodexClientError::Protocol(format!("invalid CODEX config: {}", err)))?;

    let mut root = toml::map::Map::new();
    let table = parsed.as_table().ok_or_else(|| {
        CodexClientError::Protocol("CODEX config must be a TOML table".to_string())
    })?;

    for key in [
        "model_provider",
        "model",
        "preferred_auth_method",
        "personality",
        "approval_policy",
        "sandbox_mode",
        "approvals_reviewer",
        "model_reasoning_effort",
        "model_context_window",
        "model_auto_compact_token_limit",
    ] {
        if let Some(value) = table.get(key) {
            root.insert(key.to_string(), value.clone());
        }
    }

    if let Some(value) = table.get("sandbox_workspace_write") {
        root.insert("sandbox_workspace_write".to_string(), value.clone());
    }
    if let Some(value) = table.get("model_providers") {
        root.insert("model_providers".to_string(), value.clone());
    }

    let mut projects = toml::map::Map::new();
    projects.insert(
        cwd.display().to_string(),
        TomlValue::Table({
            let mut project = toml::map::Map::new();
            project.insert(
                "trust_level".to_string(),
                TomlValue::String("trusted".to_string()),
            );
            project
        }),
    );
    root.insert("projects".to_string(), TomlValue::Table(projects));

    Ok(TomlValue::Table(root).to_string())
}

fn minimal_codex_config(cwd: &Path) -> String {
    format!(
        r#"model = "gpt-5.4"
approval_policy = "never"
sandbox_mode = "danger-full-access"
approvals_reviewer = "user"

[projects."{cwd}"]
trust_level = "trusted"
"#,
        cwd = cwd.display()
    )
}

#[derive(Debug, Deserialize)]
struct JsonRpcEnvelope {
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    params: Option<Value>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<CodexRpcErrorPayload>,
}

#[derive(Debug, Clone, Deserialize)]
struct CodexRpcErrorPayload {
    code: i32,
    message: String,
    #[serde(default)]
    data: Option<Value>,
}

#[derive(Debug, Error)]
enum CodexClientError {
    #[error("Codex transport closed while awaiting a response")]
    TransportClosed,

    #[error("Codex protocol error: {0}")]
    Protocol(String),

    #[error("Codex JSON serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Codex I/O failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("Codex RPC error {0}: {1}")]
    Rpc(i32, String),
}

impl CodexClientError {
    fn is_transport(&self) -> bool {
        matches!(self, Self::TransportClosed | Self::Io(_))
    }

    fn is_thread_not_found(&self) -> bool {
        matches!(self, Self::Rpc(_, message) if message.to_ascii_lowercase().contains("no rollout found") || message.to_ascii_lowercase().contains("thread id"))
    }
}

impl From<CodexRpcErrorPayload> for CodexClientError {
    fn from(value: CodexRpcErrorPayload) -> Self {
        let message = match value.data {
            Some(data) => format!("{} ({})", value.message, data),
            None => value.message,
        };
        Self::Rpc(value.code, message)
    }
}

impl From<CodexClientError> for AgentError {
    fn from(value: CodexClientError) -> Self {
        match value {
            CodexClientError::TransportClosed => {
                AgentError::ApiError("Codex transport closed unexpectedly".to_string())
            }
            CodexClientError::Protocol(message) => AgentError::ApiError(message),
            CodexClientError::Serialization(error) => AgentError::SerializationError(error),
            CodexClientError::Io(error) => AgentError::IoError(error),
            CodexClientError::Rpc(code, message) => {
                AgentError::ApiError(format!("Codex RPC error {}: {}", code, message))
            }
        }
    }
}

fn parse_thread(result: &Value) -> std::result::Result<ThreadInfo, CodexClientError> {
    let thread = result
        .get("thread")
        .ok_or_else(|| CodexClientError::Protocol("Codex response missing thread".to_string()))?;
    let id = thread.get("id").and_then(Value::as_str).ok_or_else(|| {
        CodexClientError::Protocol("Codex thread response missing id".to_string())
    })?;
    Ok(ThreadInfo {
        id: id.to_string(),
        path: thread
            .get("path")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    })
}

fn parse_turn_id(result: &Value) -> std::result::Result<String, CodexClientError> {
    result
        .get("turn")
        .and_then(|turn| turn.get("id"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| {
            CodexClientError::Protocol("Codex turn/start response missing turn id".to_string())
        })
}

fn apply_session_metadata(session: &mut Session, backend_description: &str, thread: &ThreadInfo) {
    session.metadata.insert(
        CODEX_THREAD_ID_METADATA_KEY.to_string(),
        thread.id.to_string(),
    );
    session.metadata.insert(
        CODEX_BACKEND_METADATA_KEY.to_string(),
        backend_description.to_string(),
    );
    if let Some(path) = &thread.path {
        session
            .metadata
            .insert(CODEX_THREAD_PATH_METADATA_KEY.to_string(), path.clone());
    }
}

fn latest_user_message(messages: &[Message]) -> String {
    messages
        .iter()
        .rev()
        .find(|message| message.role == MessageRole::User)
        .or_else(|| messages.last())
        .map(|message| message.content.clone())
        .unwrap_or_default()
}

fn build_bootstrap_prompt(messages: &[Message]) -> String {
    if messages.len() <= 1 {
        return latest_user_message(messages);
    }

    let mut prompt = String::from(
        "AgentIM is reconnecting an existing conversation. Continue naturally from the transcript below.\n\n",
    );

    for message in messages {
        prompt.push_str(match message.role {
            MessageRole::User => "User",
            MessageRole::Assistant => "Assistant",
            MessageRole::System => "System",
        });
        prompt.push_str(": ");
        prompt.push_str(message.content.trim());
        prompt.push_str("\n\n");
    }

    prompt.push_str("Please answer the latest user message.");
    prompt
}

async fn write_json_line<W, T>(
    writer: &mut W,
    value: &T,
) -> std::result::Result<(), CodexClientError>
where
    W: AsyncWrite + Unpin + ?Sized,
    T: Serialize,
{
    let mut encoded = serde_json::to_vec(value)?;
    encoded.push(b'\n');
    writer.write_all(&encoded).await?;
    writer.flush().await?;
    Ok(())
}

async fn reject_server_request<W>(
    writer: &mut W,
    id: Value,
) -> std::result::Result<(), CodexClientError>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    write_json_line(
        writer,
        &serde_json::json!({
            "id": id,
            "error": {
                "code": -32601,
                "message": "AgentIM Codex bridge does not support server-to-client requests yet"
            }
        }),
    )
    .await
}

fn matches_request_id(value: &Value, request_id: i64) -> bool {
    value.as_i64() == Some(request_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Mutex as StdMutex;
    use tokio::io::split;

    #[derive(Default)]
    struct MockCodexState {
        next_thread: usize,
        threads: HashSet<String>,
        thread_start_calls: usize,
        thread_resume_calls: usize,
        turn_calls: Vec<(String, String)>,
    }

    #[derive(Clone)]
    struct InMemoryCodexTransportFactory {
        state: Arc<StdMutex<MockCodexState>>,
    }

    #[async_trait]
    impl CodexTransportFactory for InMemoryCodexTransportFactory {
        async fn connect(&self) -> std::result::Result<CodexTransport, CodexClientError> {
            let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
            let (client_reader, client_writer) = split(client_stream);
            let (server_reader, server_writer) = split(server_stream);
            let state = self.state.clone();

            tokio::spawn(async move {
                run_mock_codex_server(server_reader, server_writer, state).await;
            });

            Ok(CodexTransport::new(
                Box::pin(client_reader),
                Box::pin(client_writer),
                None,
            ))
        }

        fn describe(&self) -> String {
            "in-memory-codex".to_string()
        }
    }

    #[tokio::test]
    async fn codex_agent_creates_and_reuses_threads() {
        let shared = Arc::new(StdMutex::new(MockCodexState::default()));
        let factory: Arc<dyn CodexTransportFactory> = Arc::new(InMemoryCodexTransportFactory {
            state: shared.clone(),
        });
        let mut session = Session::new(
            "codex-agent".to_string(),
            "telegram-bot".to_string(),
            "user-1".to_string(),
        );
        let agent = CodexAgent::from_factory(
            "codex-agent".to_string(),
            factory,
            std::env::current_dir().unwrap(),
        );

        session.add_message(MessageRole::User, "hello".to_string());
        let first_context = session.get_context(10);
        let first = agent
            .send_message_with_session(&mut session, first_context)
            .await
            .unwrap();
        assert_eq!(first, "codex:codex-thread-1:hello");
        assert_eq!(
            session.metadata.get(CODEX_THREAD_ID_METADATA_KEY),
            Some(&"codex-thread-1".to_string())
        );

        session.add_message(MessageRole::Assistant, first);
        session.add_message(MessageRole::User, "second".to_string());
        let second_context = session.get_context(10);
        let second = agent
            .send_message_with_session(&mut session, second_context)
            .await
            .unwrap();
        assert_eq!(second, "codex:codex-thread-1:second");

        let state = shared.lock().unwrap();
        assert_eq!(state.thread_start_calls, 1);
        assert_eq!(state.thread_resume_calls, 0);
        assert_eq!(
            state.turn_calls,
            vec![
                ("codex-thread-1".to_string(), "hello".to_string()),
                ("codex-thread-1".to_string(), "second".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn codex_agent_resumes_saved_threads_after_reconnect() {
        let shared = Arc::new(StdMutex::new(MockCodexState::default()));
        let factory: Arc<dyn CodexTransportFactory> = Arc::new(InMemoryCodexTransportFactory {
            state: shared.clone(),
        });
        let mut session = Session::new(
            "codex-agent".to_string(),
            "telegram-bot".to_string(),
            "user-1".to_string(),
        );
        let agent = CodexAgent::from_factory(
            "codex-agent".to_string(),
            factory,
            std::env::current_dir().unwrap(),
        );

        session.add_message(MessageRole::User, "hello".to_string());
        let first_context = session.get_context(10);
        let first = agent
            .send_message_with_session(&mut session, first_context)
            .await
            .unwrap();
        session.add_message(MessageRole::Assistant, first);

        agent.client.reset().await;

        session.add_message(MessageRole::User, "after reconnect".to_string());
        let second_context = session.get_context(10);
        let second = agent
            .send_message_with_session(&mut session, second_context)
            .await
            .unwrap();

        assert_eq!(second, "codex:codex-thread-1:after reconnect");

        let state = shared.lock().unwrap();
        assert_eq!(state.thread_start_calls, 1);
        assert_eq!(state.thread_resume_calls, 1);
        assert_eq!(
            state.turn_calls,
            vec![
                ("codex-thread-1".to_string(), "hello".to_string()),
                ("codex-thread-1".to_string(), "after reconnect".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn codex_agent_falls_back_to_new_thread_when_saved_thread_is_gone() {
        let shared = Arc::new(StdMutex::new(MockCodexState::default()));
        let factory: Arc<dyn CodexTransportFactory> = Arc::new(InMemoryCodexTransportFactory {
            state: shared.clone(),
        });
        let mut session = Session::new(
            "codex-agent".to_string(),
            "telegram-bot".to_string(),
            "user-1".to_string(),
        );
        let agent = CodexAgent::from_factory(
            "codex-agent".to_string(),
            factory,
            std::env::current_dir().unwrap(),
        );

        session.add_message(MessageRole::User, "hello".to_string());
        let first_context = session.get_context(10);
        let first = agent
            .send_message_with_session(&mut session, first_context)
            .await
            .unwrap();
        let first_thread = session
            .metadata
            .get(CODEX_THREAD_ID_METADATA_KEY)
            .cloned()
            .unwrap();
        session.add_message(MessageRole::Assistant, first);

        {
            let mut state = shared.lock().unwrap();
            state.threads.clear();
        }
        agent.client.reset().await;

        session.add_message(MessageRole::User, "recovered".to_string());
        let second_context = session.get_context(10);
        let second = agent
            .send_message_with_session(&mut session, second_context)
            .await
            .unwrap();
        let second_thread = session
            .metadata
            .get(CODEX_THREAD_ID_METADATA_KEY)
            .cloned()
            .unwrap();

        assert!(second.starts_with("codex:codex-thread-2:AgentIM is reconnecting"));
        assert!(second.contains("User: recovered"));
        assert_ne!(first_thread, second_thread);

        let state = shared.lock().unwrap();
        assert_eq!(state.thread_start_calls, 2);
        assert_eq!(state.thread_resume_calls, 1);
    }

    #[test]
    fn sanitized_codex_config_removes_omx_runtime_sections() {
        let source = r#"
notify = ["node", "notify.js"]
developer_instructions = "omx"
model_provider = "crs"
model = "gpt-5.4"
approval_policy = "never"
sandbox_mode = "danger-full-access"

[features]
codex_hooks = true

[model_providers.crs]
name = "crs"
base_url = "https://example.com/v1"
wire_api = "responses"
requires_openai_auth = true
env_key = "CRS_OAI_KEY"

[mcp_servers.omx_state]
command = "node"
"#;

        let sanitized = sanitize_codex_config(source, Path::new("/tmp/project")).unwrap();
        assert!(sanitized.contains("model_provider = \"crs\""));
        assert!(sanitized.contains("base_url = \"https://example.com/v1\""));
        assert!(sanitized.contains("/tmp/project"));
        assert!(sanitized.contains("trust_level = \"trusted\""));
        assert!(!sanitized.contains("notify ="));
        assert!(!sanitized.contains("developer_instructions"));
        assert!(!sanitized.contains("mcp_servers"));
        assert!(!sanitized.contains("codex_hooks"));
    }

    async fn run_mock_codex_server<R, W>(
        reader: R,
        mut writer: W,
        state: Arc<StdMutex<MockCodexState>>,
    ) where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let mut reader = BufReader::new(reader);

        loop {
            let mut line = String::new();
            let Ok(bytes_read) = reader.read_line(&mut line).await else {
                break;
            };
            if bytes_read == 0 {
                break;
            }

            let Ok(envelope) = serde_json::from_str::<JsonRpcEnvelope>(&line) else {
                continue;
            };

            let Some(id) = envelope.id else {
                continue;
            };
            let Some(method) = envelope.method.as_deref() else {
                continue;
            };

            match method {
                INITIALIZE_METHOD => {
                    write_json_line(
                        &mut writer,
                        &serde_json::json!({
                            "id": id,
                            "result": {
                                "platformOs": "linux",
                                "userAgent": "mock-codex/0.1.0"
                            }
                        }),
                    )
                    .await
                    .unwrap();
                }
                THREAD_START_METHOD => {
                    let thread_id = {
                        let mut state = state.lock().unwrap();
                        state.next_thread += 1;
                        state.thread_start_calls += 1;
                        let thread_id = format!("codex-thread-{}", state.next_thread);
                        state.threads.insert(thread_id.clone());
                        thread_id
                    };

                    write_json_line(
                        &mut writer,
                        &serde_json::json!({
                            "id": id,
                            "result": {
                                "thread": {
                                    "id": thread_id,
                                    "path": format!("/tmp/{thread_id}.jsonl")
                                }
                            }
                        }),
                    )
                    .await
                    .unwrap();
                }
                THREAD_RESUME_METHOD => {
                    let thread_id = envelope
                        .params
                        .as_ref()
                        .and_then(|params| params.get("threadId"))
                        .and_then(Value::as_str)
                        .unwrap()
                        .to_string();
                    let known = {
                        let mut state = state.lock().unwrap();
                        state.thread_resume_calls += 1;
                        state.threads.contains(&thread_id)
                    };

                    if known {
                        write_json_line(
                            &mut writer,
                            &serde_json::json!({
                                "id": id,
                                "result": {
                                    "thread": {
                                        "id": thread_id,
                                        "path": format!("/tmp/{thread_id}.jsonl")
                                    }
                                }
                            }),
                        )
                        .await
                        .unwrap();
                    } else {
                        write_json_line(
                            &mut writer,
                            &serde_json::json!({
                                "id": id,
                                "error": {
                                    "code": -32600,
                                    "message": format!("no rollout found for thread id {}", thread_id)
                                }
                            }),
                        )
                        .await
                        .unwrap();
                    }
                }
                TURN_START_METHOD => {
                    let thread_id = envelope
                        .params
                        .as_ref()
                        .and_then(|params| params.get("threadId"))
                        .and_then(Value::as_str)
                        .unwrap()
                        .to_string();
                    let prompt = envelope
                        .params
                        .as_ref()
                        .and_then(|params| params.get("input"))
                        .and_then(Value::as_array)
                        .and_then(|items| items.first())
                        .and_then(|item| item.get("text"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let turn_id = format!("turn-{}", thread_id);

                    {
                        let mut state = state.lock().unwrap();
                        state.turn_calls.push((thread_id.clone(), prompt.clone()));
                    }

                    write_json_line(
                        &mut writer,
                        &serde_json::json!({
                            "id": id,
                            "result": {
                                "turn": {
                                    "id": turn_id,
                                    "status": "inProgress",
                                    "items": []
                                }
                            }
                        }),
                    )
                    .await
                    .unwrap();

                    write_json_line(
                        &mut writer,
                        &serde_json::json!({
                            "method": AGENT_MESSAGE_DELTA_NOTIFICATION,
                            "params": {
                                "threadId": thread_id,
                                "turnId": turn_id,
                                "delta": format!("codex:{}:{}", thread_id, prompt)
                            }
                        }),
                    )
                    .await
                    .unwrap();

                    write_json_line(
                        &mut writer,
                        &serde_json::json!({
                            "method": TURN_COMPLETED_NOTIFICATION,
                            "params": {
                                "threadId": thread_id,
                                "turn": {
                                    "id": turn_id,
                                    "status": "completed",
                                    "error": null
                                }
                            }
                        }),
                    )
                    .await
                    .unwrap();
                }
                _ => {
                    write_json_line(
                        &mut writer,
                        &serde_json::json!({
                            "id": id,
                            "error": {
                                "code": -32601,
                                "message": "method not found"
                            }
                        }),
                    )
                    .await
                    .unwrap();
                }
            }
        }
    }
}
