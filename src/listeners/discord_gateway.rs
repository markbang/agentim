use crate::bots::discord::{run_discord_gateway_once, DiscordBotChannel};
use crate::channel::Channel;
use crate::error::Result;
use crate::listener::{
    default_checkpoint_path, load_checkpoint_from_path, save_checkpoint_to_path, InboundListener,
    ListenerCheckpoint,
};
use crate::manager::{AgentIM, MessageHandlingOptions};
use crate::metrics;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

pub struct DiscordGatewayListener {
    id: String,
    agentim: Arc<AgentIM>,
    channel: Arc<DiscordBotChannel>,
    agent_id: String,
    options: MessageHandlingOptions,
    state_file: Option<String>,
    state_backup_count: usize,
    checkpoint_path: Option<PathBuf>,
}

impl DiscordGatewayListener {
    pub fn new(
        agentim: Arc<AgentIM>,
        channel: Arc<DiscordBotChannel>,
        agent_id: String,
        options: MessageHandlingOptions,
        state_file: Option<String>,
        state_backup_count: usize,
    ) -> Self {
        let id = "discord-gateway".to_string();
        let checkpoint_path = default_checkpoint_path(state_file.as_deref(), &id);
        Self {
            id,
            agentim,
            channel,
            agent_id,
            options,
            state_file,
            state_backup_count,
            checkpoint_path,
        }
    }
}

#[async_trait]
impl InboundListener for DiscordGatewayListener {
    fn id(&self) -> &str {
        &self.id
    }

    async fn initialize(&self) -> Result<()> {
        self.channel.health_check().await
    }

    async fn load_checkpoint(&self) -> Result<ListenerCheckpoint> {
        load_checkpoint_from_path::<ListenerCheckpoint>(self.checkpoint_path.clone()).await
    }

    async fn run_once(&self, checkpoint: &mut ListenerCheckpoint) -> Result<bool> {
        let gateway_url = self.channel.get_gateway_url().await?;
        let before = checkpoint.cursor.clone();
        let updated_cursor = run_discord_gateway_once(
            self.agentim.clone(),
            self.channel.clone(),
            &self.agent_id,
            self.options,
            &gateway_url,
            state_file_as_deref(&self.state_file),
            self.state_backup_count,
            before
                .as_deref()
                .and_then(|value| value.parse::<u64>().ok()),
        )
        .await?;
        checkpoint.cursor = updated_cursor.map(|value| value.to_string());
        metrics::inc_webhook_request("discord_listener");
        Ok(checkpoint.cursor != before)
    }

    async fn persist_checkpoint(&self, checkpoint: &ListenerCheckpoint) -> Result<()> {
        save_checkpoint_to_path(self.checkpoint_path.clone(), checkpoint).await
    }
}

fn state_file_as_deref(value: &Option<String>) -> Option<&str> {
    value.as_deref()
}
