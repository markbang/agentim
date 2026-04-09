use crate::agent::Agent;
use crate::config::AgentType;
use crate::error::{AgentError, Result};
use crate::session::{Message, MessageRole, Session};
use agent_client_protocol_schema as acp;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

const ACP_SESSION_ID_METADATA_KEY: &str = "acp_session_id";
const ACP_BACKEND_METADATA_KEY: &str = "acp_backend";
const ACP_AGENT_METADATA_KEY: &str = "acp_agent";
const ACP_STOP_REASON_METADATA_KEY: &str = "acp_stop_reason";
const ACP_INITIALIZE_METHOD: &str = "initialize";
const ACP_NEW_SESSION_METHOD: &str = "session/new";
const ACP_LOAD_SESSION_METHOD: &str = "session/load";
const ACP_PROMPT_METHOD: &str = "session/prompt";
const ACP_SESSION_UPDATE_NOTIFICATION: &str = "session/update";
const ACP_RESOURCE_NOT_FOUND_CODE: i32 = -32002;

type BoxRead = Pin<Box<dyn AsyncRead + Send>>;
type BoxWrite = Pin<Box<dyn AsyncWrite + Send>>;

#[derive(Debug, Clone)]
pub struct AcpBackendConfig {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
}

impl AcpBackendConfig {
    pub fn describe(&self) -> String {
        if self.args.is_empty() {
            self.command.clone()
        } else {
            format!("{} {}", self.command, self.args.join(" "))
        }
    }
}

pub struct AcpAgent {
    id: String,
    client: Arc<AcpSessionClient>,
}

impl AcpAgent {
    pub fn new(id: String, config: AcpBackendConfig) -> Self {
        let session_cwd = config.cwd.clone();
        let factory: Arc<dyn AcpTransportFactory> = Arc::new(ProcessAcpTransportFactory { config });
        Self::from_factory(id, factory, session_cwd)
    }

    fn from_factory(
        id: String,
        factory: Arc<dyn AcpTransportFactory>,
        session_cwd: PathBuf,
    ) -> Self {
        let backend_description = factory.describe();
        Self {
            id,
            client: Arc::new(AcpSessionClient {
                factory,
                state: Mutex::new(None),
                session_cwd,
                backend_description,
            }),
        }
    }
}

#[async_trait]
impl Agent for AcpAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Acp
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, _messages: Vec<Message>) -> Result<String> {
        Err(AgentError::ConfigError(
            "ACP agents require session-aware dispatch".to_string(),
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

struct AcpSessionClient {
    factory: Arc<dyn AcpTransportFactory>,
    state: Mutex<Option<AcpClientState>>,
    session_cwd: PathBuf,
    backend_description: String,
}

impl AcpSessionClient {
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
                    "ACP transport failed; reconnecting once"
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
    ) -> std::result::Result<String, AcpClientError> {
        let mut state = self.state.lock().await;
        self.ensure_connected(&mut state).await?;
        let state = state.as_mut().expect("acp state initialized");

        let remote_session = state
            .ensure_remote_session(session, &self.session_cwd, &self.backend_description)
            .await?;

        let prompt_text = if remote_session.is_fresh {
            build_bootstrap_prompt(messages)
        } else {
            latest_user_message(messages)
        };

        match state
            .prompt_session(session, &remote_session.id, prompt_text)
            .await
        {
            Ok(response) => Ok(response),
            Err(error) if error.is_session_not_found() => {
                warn!(
                    session_id = %session.id,
                    remote_session_id = %remote_session.id,
                    error = %error,
                    "ACP session expired; creating a replacement session"
                );
                state.remote_sessions.remove(&session.id);
                session.metadata.remove(ACP_SESSION_ID_METADATA_KEY);

                let replacement = state
                    .create_remote_session(session, &self.session_cwd, &self.backend_description)
                    .await?;

                state
                    .prompt_session(session, &replacement.id, build_bootstrap_prompt(messages))
                    .await
            }
            Err(error) => Err(error),
        }
    }

    async fn ensure_connected(
        &self,
        state: &mut Option<AcpClientState>,
    ) -> std::result::Result<(), AcpClientError> {
        let needs_connect = match state.as_mut() {
            Some(existing) => existing.transport_has_exited()?,
            None => true,
        };

        if needs_connect {
            *state = Some(AcpClientState::connect(self.factory.clone()).await?);
        }

        Ok(())
    }

    async fn reset(&self) {
        let mut state = self.state.lock().await;
        *state = None;
    }
}

struct AcpClientState {
    transport: AcpTransport,
    next_request_id: i64,
    capabilities: acp::AgentCapabilities,
    agent_info: Option<acp::Implementation>,
    remote_sessions: HashMap<String, String>,
}

impl AcpClientState {
    async fn connect(
        factory: Arc<dyn AcpTransportFactory>,
    ) -> std::result::Result<Self, AcpClientError> {
        let mut state = Self {
            transport: factory.connect().await?,
            next_request_id: 0,
            capabilities: acp::AgentCapabilities::default(),
            agent_info: None,
            remote_sessions: HashMap::new(),
        };

        let response: acp::InitializeResponse = state
            .request(
                ACP_INITIALIZE_METHOD,
                &acp::InitializeRequest::new(acp::ProtocolVersion::V1).client_info(
                    acp::Implementation::new("agentim", env!("CARGO_PKG_VERSION")).title("AgentIM"),
                ),
                None,
                None,
            )
            .await?;

        state.capabilities = response.agent_capabilities;
        state.agent_info = response.agent_info;

        let agent_label = state
            .agent_info
            .as_ref()
            .map(|info| info.title.clone().unwrap_or_else(|| info.name.clone()))
            .unwrap_or_else(|| "unknown-acp-agent".to_string());
        info!(agent = %agent_label, "ACP backend initialized");

        Ok(state)
    }

    fn transport_has_exited(&mut self) -> std::result::Result<bool, AcpClientError> {
        self.transport.has_exited()
    }

    async fn ensure_remote_session(
        &mut self,
        session: &mut Session,
        session_cwd: &Path,
        backend_description: &str,
    ) -> std::result::Result<RemoteSession, AcpClientError> {
        if let Some(remote_session_id) = self.remote_sessions.get(&session.id) {
            return Ok(RemoteSession {
                id: remote_session_id.clone(),
                is_fresh: false,
            });
        }

        if let Some(saved_session_id) = session.metadata.get(ACP_SESSION_ID_METADATA_KEY).cloned() {
            if self.capabilities.load_session {
                match self
                    .request::<_, acp::LoadSessionResponse>(
                        ACP_LOAD_SESSION_METHOD,
                        &acp::LoadSessionRequest::new(saved_session_id.clone(), session_cwd),
                        None,
                        None,
                    )
                    .await
                {
                    Ok(_) => {
                        self.remote_sessions
                            .insert(session.id.clone(), saved_session_id.clone());
                        apply_session_metadata(
                            session,
                            backend_description,
                            &saved_session_id,
                            self.agent_info.as_ref(),
                        );
                        debug!(
                            session_id = %session.id,
                            remote_session_id = %saved_session_id,
                            "ACP session loaded"
                        );
                        return Ok(RemoteSession {
                            id: saved_session_id,
                            is_fresh: false,
                        });
                    }
                    Err(error) if error.is_session_not_found() => {
                        warn!(
                            session_id = %session.id,
                            remote_session_id = %saved_session_id,
                            error = %error,
                            "Saved ACP session is unavailable; creating a new session"
                        );
                    }
                    Err(error) => return Err(error),
                }
            } else {
                debug!(
                    session_id = %session.id,
                    remote_session_id = %saved_session_id,
                    "ACP backend does not support session/load; creating a fresh session"
                );
            }
        }

        self.create_remote_session(session, session_cwd, backend_description)
            .await
    }

    async fn create_remote_session(
        &mut self,
        session: &mut Session,
        session_cwd: &Path,
        backend_description: &str,
    ) -> std::result::Result<RemoteSession, AcpClientError> {
        let response: acp::NewSessionResponse = self
            .request(
                ACP_NEW_SESSION_METHOD,
                &acp::NewSessionRequest::new(session_cwd),
                None,
                None,
            )
            .await?;
        let remote_session_id = response.session_id.0.to_string();
        self.remote_sessions
            .insert(session.id.clone(), remote_session_id.clone());
        apply_session_metadata(
            session,
            backend_description,
            &remote_session_id,
            self.agent_info.as_ref(),
        );

        debug!(
            session_id = %session.id,
            remote_session_id = %remote_session_id,
            "ACP session created"
        );

        Ok(RemoteSession {
            id: remote_session_id,
            is_fresh: true,
        })
    }

    async fn prompt_session(
        &mut self,
        session: &mut Session,
        remote_session_id: &str,
        prompt_text: String,
    ) -> std::result::Result<String, AcpClientError> {
        let mut response_text = String::new();
        let response: acp::PromptResponse = self
            .request(
                ACP_PROMPT_METHOD,
                &acp::PromptRequest::new(
                    acp::SessionId::new(remote_session_id.to_string()),
                    vec![acp::ContentBlock::Text(acp::TextContent::new(prompt_text))],
                ),
                Some(remote_session_id),
                Some(&mut response_text),
            )
            .await?;

        session.metadata.insert(
            ACP_STOP_REASON_METADATA_KEY.to_string(),
            format!("{:?}", response.stop_reason),
        );

        if response_text.trim().is_empty() {
            return Err(AcpClientError::Protocol(
                "ACP prompt completed without any text response".to_string(),
            ));
        }

        Ok(response_text)
    }

    async fn request<Req, Resp>(
        &mut self,
        method: &str,
        params: &Req,
        streamed_session_id: Option<&str>,
        mut streamed_response: Option<&mut String>,
    ) -> std::result::Result<Resp, AcpClientError>
    where
        Req: Serialize,
        Resp: DeserializeOwned,
    {
        let request_id = self.next_request_id;
        self.next_request_id += 1;

        write_json_line(
            &mut self.transport.writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
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
                return Err(AcpClientError::TransportClosed);
            }

            let envelope: JsonRpcEnvelope = serde_json::from_str(&line)?;

            if let Some(id) = &envelope.id {
                if envelope.method.is_some() {
                    reject_agent_request(&mut self.transport.writer, id.clone()).await?;
                    continue;
                }

                if !matches_request_id(id, request_id) {
                    continue;
                }

                if let Some(error) = envelope.error {
                    return Err(error.into());
                }

                let result = envelope.result.ok_or_else(|| {
                    AcpClientError::Protocol(format!(
                        "ACP response for '{}' is missing a result payload",
                        method
                    ))
                })?;
                return serde_json::from_value(result).map_err(AcpClientError::Serialization);
            }

            let Some(notification_method) = envelope.method.as_deref() else {
                continue;
            };

            if notification_method != ACP_SESSION_UPDATE_NOTIFICATION {
                continue;
            }

            let (Some(expected_session_id), Some(buffer)) =
                (streamed_session_id, streamed_response.as_mut())
            else {
                continue;
            };

            let notification: acp::SessionNotification =
                serde_json::from_value(envelope.params.unwrap_or(serde_json::Value::Null))?;
            if notification.session_id.0.as_ref() == expected_session_id {
                append_session_update(buffer, notification.update);
            }
        }
    }
}

struct RemoteSession {
    id: String,
    is_fresh: bool,
}

struct AcpTransport {
    reader: BufReader<BoxRead>,
    writer: BoxWrite,
    child: Option<Child>,
}

impl AcpTransport {
    fn new(reader: BoxRead, writer: BoxWrite, child: Option<Child>) -> Self {
        Self {
            reader: BufReader::new(reader),
            writer,
            child,
        }
    }

    fn has_exited(&mut self) -> std::result::Result<bool, AcpClientError> {
        let Some(child) = self.child.as_mut() else {
            return Ok(false);
        };

        match child.try_wait()? {
            Some(status) => {
                warn!(status = %status, "ACP subprocess exited");
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

#[async_trait]
trait AcpTransportFactory: Send + Sync {
    async fn connect(&self) -> std::result::Result<AcpTransport, AcpClientError>;
    fn describe(&self) -> String;
}

#[derive(Debug)]
struct ProcessAcpTransportFactory {
    config: AcpBackendConfig,
}

#[async_trait]
impl AcpTransportFactory for ProcessAcpTransportFactory {
    async fn connect(&self) -> std::result::Result<AcpTransport, AcpClientError> {
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

        let mut child = command.spawn()?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AcpClientError::Protocol("ACP subprocess did not expose stdout".to_string())
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            AcpClientError::Protocol("ACP subprocess did not expose stdin".to_string())
        })?;

        Ok(AcpTransport::new(
            Box::pin(stdout),
            Box::pin(stdin),
            Some(child),
        ))
    }

    fn describe(&self) -> String {
        self.config.describe()
    }
}

#[derive(Debug, Deserialize)]
struct JsonRpcEnvelope {
    #[serde(default)]
    id: Option<serde_json::Value>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    params: Option<serde_json::Value>,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<AcpRpcErrorPayload>,
}

#[derive(Debug, Clone, Deserialize)]
struct AcpRpcErrorPayload {
    code: i32,
    message: String,
    #[serde(default)]
    data: Option<serde_json::Value>,
}

#[derive(Debug, Error)]
enum AcpClientError {
    #[error("ACP transport closed while awaiting a response")]
    TransportClosed,

    #[error("ACP protocol error: {0}")]
    Protocol(String),

    #[error("ACP JSON serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("ACP I/O failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("ACP RPC error {0}: {1}")]
    Rpc(i32, String),
}

impl AcpClientError {
    fn is_transport(&self) -> bool {
        matches!(self, Self::TransportClosed | Self::Io(_))
    }

    fn is_session_not_found(&self) -> bool {
        matches!(self, Self::Rpc(code, message) if *code == ACP_RESOURCE_NOT_FOUND_CODE || message.to_ascii_lowercase().contains("session"))
    }
}

impl From<AcpRpcErrorPayload> for AcpClientError {
    fn from(value: AcpRpcErrorPayload) -> Self {
        let message = match value.data {
            Some(data) => format!("{} ({})", value.message, data),
            None => value.message,
        };
        Self::Rpc(value.code, message)
    }
}

impl From<AcpClientError> for AgentError {
    fn from(value: AcpClientError) -> Self {
        match value {
            AcpClientError::TransportClosed => {
                AgentError::ApiError("ACP transport closed unexpectedly".to_string())
            }
            AcpClientError::Protocol(message) => AgentError::ApiError(message),
            AcpClientError::Serialization(error) => AgentError::SerializationError(error),
            AcpClientError::Io(error) => AgentError::IoError(error),
            AcpClientError::Rpc(code, message) => {
                AgentError::ApiError(format!("ACP RPC error {}: {}", code, message))
            }
        }
    }
}

fn apply_session_metadata(
    session: &mut Session,
    backend_description: &str,
    remote_session_id: &str,
    agent_info: Option<&acp::Implementation>,
) {
    session.metadata.insert(
        ACP_SESSION_ID_METADATA_KEY.to_string(),
        remote_session_id.to_string(),
    );
    session.metadata.insert(
        ACP_BACKEND_METADATA_KEY.to_string(),
        backend_description.to_string(),
    );
    if let Some(agent_info) = agent_info {
        let title = agent_info
            .title
            .clone()
            .unwrap_or_else(|| agent_info.name.clone());
        session.metadata.insert(
            ACP_AGENT_METADATA_KEY.to_string(),
            format!("{}@{}", title, agent_info.version),
        );
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

fn append_session_update(buffer: &mut String, update: acp::SessionUpdate) {
    if let acp::SessionUpdate::AgentMessageChunk(chunk) = update {
        append_content_block(buffer, chunk.content);
    }
}

fn append_content_block(buffer: &mut String, content: acp::ContentBlock) {
    match content {
        acp::ContentBlock::Text(text) => buffer.push_str(&text.text),
        acp::ContentBlock::ResourceLink(link) => buffer.push_str(&link.uri),
        acp::ContentBlock::Image(_) => buffer.push_str("[image]"),
        acp::ContentBlock::Audio(_) => buffer.push_str("[audio]"),
        acp::ContentBlock::Resource(_) => buffer.push_str("[resource]"),
        _ => buffer.push_str("[unsupported-content]"),
    }
}

async fn write_json_line<W, T>(writer: &mut W, value: &T) -> std::result::Result<(), AcpClientError>
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

async fn reject_agent_request<W>(
    writer: &mut W,
    id: serde_json::Value,
) -> std::result::Result<(), AcpClientError>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    write_json_line(
        writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32601,
                "message": "AgentIM ACP client does not support agent-to-client requests yet"
            }
        }),
    )
    .await
}

fn matches_request_id(value: &serde_json::Value, request_id: i64) -> bool {
    value.as_i64() == Some(request_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Mutex as StdMutex;
    use tokio::io::split;

    #[derive(Default)]
    struct MockAcpState {
        next_session: usize,
        sessions: HashSet<String>,
        new_session_calls: usize,
        load_session_calls: usize,
        prompt_calls: Vec<(String, String)>,
    }

    #[derive(Clone)]
    struct InMemoryAcpTransportFactory {
        state: Arc<StdMutex<MockAcpState>>,
    }

    #[async_trait]
    impl AcpTransportFactory for InMemoryAcpTransportFactory {
        async fn connect(&self) -> std::result::Result<AcpTransport, AcpClientError> {
            let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
            let (client_reader, client_writer) = split(client_stream);
            let (server_reader, server_writer) = split(server_stream);
            let state = self.state.clone();

            tokio::spawn(async move {
                run_mock_acp_agent(server_reader, server_writer, state).await;
            });

            Ok(AcpTransport::new(
                Box::pin(client_reader),
                Box::pin(client_writer),
                None,
            ))
        }

        fn describe(&self) -> String {
            "in-memory-acp".to_string()
        }
    }

    #[tokio::test]
    async fn acp_agent_creates_and_reuses_remote_sessions() {
        let shared = Arc::new(StdMutex::new(MockAcpState::default()));
        let factory: Arc<dyn AcpTransportFactory> = Arc::new(InMemoryAcpTransportFactory {
            state: shared.clone(),
        });
        let mut session = Session::new(
            "acp-agent".to_string(),
            "telegram-bot".to_string(),
            "user-1".to_string(),
        );
        let agent = AcpAgent::from_factory(
            "acp-agent".to_string(),
            factory,
            std::env::current_dir().unwrap(),
        );

        session.add_message(MessageRole::User, "hello".to_string());
        let first_context = session.get_context(10);
        let first = agent
            .send_message_with_session(&mut session, first_context)
            .await
            .unwrap();
        assert_eq!(first, "acp:acp-session-1:hello");
        assert_eq!(
            session.metadata.get(ACP_SESSION_ID_METADATA_KEY),
            Some(&"acp-session-1".to_string())
        );

        session.add_message(MessageRole::Assistant, first);
        session.add_message(MessageRole::User, "second".to_string());
        let second_context = session.get_context(10);
        let second = agent
            .send_message_with_session(&mut session, second_context)
            .await
            .unwrap();
        assert_eq!(second, "acp:acp-session-1:second");

        let state = shared.lock().unwrap();
        assert_eq!(state.new_session_calls, 1);
        assert_eq!(state.load_session_calls, 0);
        assert_eq!(
            state.prompt_calls,
            vec![
                ("acp-session-1".to_string(), "hello".to_string()),
                ("acp-session-1".to_string(), "second".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn acp_agent_loads_saved_sessions_after_reconnect() {
        let shared = Arc::new(StdMutex::new(MockAcpState::default()));
        let factory: Arc<dyn AcpTransportFactory> = Arc::new(InMemoryAcpTransportFactory {
            state: shared.clone(),
        });
        let mut session = Session::new(
            "acp-agent".to_string(),
            "telegram-bot".to_string(),
            "user-1".to_string(),
        );
        let agent = AcpAgent::from_factory(
            "acp-agent".to_string(),
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

        assert_eq!(second, "acp:acp-session-1:after reconnect");

        let state = shared.lock().unwrap();
        assert_eq!(state.new_session_calls, 1);
        assert_eq!(state.load_session_calls, 1);
        assert_eq!(
            state.prompt_calls,
            vec![
                ("acp-session-1".to_string(), "hello".to_string()),
                ("acp-session-1".to_string(), "after reconnect".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn acp_agent_falls_back_to_new_session_when_saved_session_is_gone() {
        let shared = Arc::new(StdMutex::new(MockAcpState::default()));
        let factory: Arc<dyn AcpTransportFactory> = Arc::new(InMemoryAcpTransportFactory {
            state: shared.clone(),
        });
        let mut session = Session::new(
            "acp-agent".to_string(),
            "telegram-bot".to_string(),
            "user-1".to_string(),
        );
        let agent = AcpAgent::from_factory(
            "acp-agent".to_string(),
            factory,
            std::env::current_dir().unwrap(),
        );

        session.add_message(MessageRole::User, "hello".to_string());
        let first_context = session.get_context(10);
        let first = agent
            .send_message_with_session(&mut session, first_context)
            .await
            .unwrap();
        let first_remote_session = session
            .metadata
            .get(ACP_SESSION_ID_METADATA_KEY)
            .cloned()
            .unwrap();
        session.add_message(MessageRole::Assistant, first);

        {
            let mut state = shared.lock().unwrap();
            state.sessions.clear();
        }
        agent.client.reset().await;

        session.add_message(MessageRole::User, "recovered".to_string());
        let second_context = session.get_context(10);
        let second = agent
            .send_message_with_session(&mut session, second_context)
            .await
            .unwrap();
        let second_remote_session = session
            .metadata
            .get(ACP_SESSION_ID_METADATA_KEY)
            .cloned()
            .unwrap();

        assert!(second
            .starts_with("acp:acp-session-2:AgentIM is reconnecting an existing conversation"));
        assert!(second.contains("User: recovered"));
        assert_ne!(first_remote_session, second_remote_session);

        let state = shared.lock().unwrap();
        assert_eq!(state.new_session_calls, 2);
        assert_eq!(state.load_session_calls, 1);
    }

    async fn run_mock_acp_agent<R, W>(reader: R, mut writer: W, state: Arc<StdMutex<MockAcpState>>)
    where
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
                ACP_INITIALIZE_METHOD => {
                    let mut response = acp::InitializeResponse::new(acp::ProtocolVersion::V1);
                    response.agent_capabilities = acp::AgentCapabilities::new().load_session(true);
                    response.agent_info =
                        Some(acp::Implementation::new("mock-acp", "0.1.0").title("Mock ACP"));
                    write_json_line(
                        &mut writer,
                        &serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": response,
                        }),
                    )
                    .await
                    .unwrap();
                }
                ACP_NEW_SESSION_METHOD => {
                    let remote_session_id = {
                        let mut state = state.lock().unwrap();
                        state.next_session += 1;
                        state.new_session_calls += 1;
                        let remote_session_id = format!("acp-session-{}", state.next_session);
                        state.sessions.insert(remote_session_id.clone());
                        remote_session_id
                    };

                    write_json_line(
                        &mut writer,
                        &serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": acp::NewSessionResponse::new(remote_session_id),
                        }),
                    )
                    .await
                    .unwrap();
                }
                ACP_LOAD_SESSION_METHOD => {
                    let request: acp::LoadSessionRequest =
                        serde_json::from_value(envelope.params.unwrap()).unwrap();
                    let known = {
                        let mut state = state.lock().unwrap();
                        state.load_session_calls += 1;
                        state.sessions.contains(request.session_id.0.as_ref())
                    };

                    if known {
                        write_json_line(
                            &mut writer,
                            &serde_json::json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": acp::LoadSessionResponse::new(),
                            }),
                        )
                        .await
                        .unwrap();
                    } else {
                        write_json_line(
                            &mut writer,
                            &serde_json::json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "error": {
                                    "code": ACP_RESOURCE_NOT_FOUND_CODE,
                                    "message": "session not found"
                                }
                            }),
                        )
                        .await
                        .unwrap();
                    }
                }
                ACP_PROMPT_METHOD => {
                    let request: acp::PromptRequest =
                        serde_json::from_value(envelope.params.unwrap()).unwrap();
                    let prompt_text = request
                        .prompt
                        .iter()
                        .filter_map(|content| match content {
                            acp::ContentBlock::Text(text) => Some(text.text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    {
                        let mut state = state.lock().unwrap();
                        state
                            .prompt_calls
                            .push((request.session_id.0.to_string(), prompt_text.clone()));
                    }

                    let reply = format!("acp:{}:{}", request.session_id.0, prompt_text);
                    let notification = acp::SessionNotification::new(
                        request.session_id.clone(),
                        acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                            acp::ContentBlock::Text(acp::TextContent::new(reply)),
                        )),
                    );
                    write_json_line(
                        &mut writer,
                        &serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": ACP_SESSION_UPDATE_NOTIFICATION,
                            "params": notification,
                        }),
                    )
                    .await
                    .unwrap();

                    write_json_line(
                        &mut writer,
                        &serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": acp::PromptResponse::new(acp::StopReason::EndTurn),
                        }),
                    )
                    .await
                    .unwrap();
                }
                _ => {
                    write_json_line(
                        &mut writer,
                        &serde_json::json!({
                            "jsonrpc": "2.0",
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
