use crate::bots::telegram::{run_telegram_poll_once, TelegramBotChannel};
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

pub struct TelegramPollingListener {
    id: String,
    agentim: Arc<AgentIM>,
    channel: Arc<TelegramBotChannel>,
    agent_id: String,
    options: MessageHandlingOptions,
    state_file: Option<String>,
    state_backup_count: usize,
    checkpoint_path: Option<PathBuf>,
}

impl TelegramPollingListener {
    pub fn new(
        agentim: Arc<AgentIM>,
        channel: Arc<TelegramBotChannel>,
        agent_id: String,
        options: MessageHandlingOptions,
        state_file: Option<String>,
        state_backup_count: usize,
    ) -> Self {
        let id = "telegram-polling".to_string();
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
impl InboundListener for TelegramPollingListener {
    fn id(&self) -> &str {
        &self.id
    }

    async fn initialize(&self) -> Result<()> {
        self.channel.delete_webhook(true).await
    }

    async fn load_checkpoint(&self) -> Result<ListenerCheckpoint> {
        let mut checkpoint =
            load_checkpoint_from_path::<ListenerCheckpoint>(self.checkpoint_path.clone()).await?;
        if checkpoint.cursor.is_none() {
            let next_offset = self
                .channel
                .get_updates(None, 0, 100)
                .await?
                .into_iter()
                .map(|update| update.update_id + 1)
                .max();
            checkpoint.cursor = next_offset.map(|offset| offset.to_string());
        }
        Ok(checkpoint)
    }

    async fn run_once(&self, checkpoint: &mut ListenerCheckpoint) -> Result<bool> {
        let next_offset = checkpoint
            .cursor
            .as_deref()
            .map(str::parse::<i64>)
            .transpose()?;
        let updated_offset = run_telegram_poll_once(
            self.agentim.clone(),
            self.channel.clone(),
            &self.agent_id,
            self.options,
            next_offset,
            self.state_file.as_deref(),
            self.state_backup_count,
            30,
        )
        .await?;

        let processed_any = updated_offset != next_offset;
        checkpoint.cursor = updated_offset.map(|offset| offset.to_string());
        metrics::inc_webhook_request("telegram_listener");
        Ok(processed_any)
    }

    async fn persist_checkpoint(&self, checkpoint: &ListenerCheckpoint) -> Result<()> {
        save_checkpoint_to_path(self.checkpoint_path.clone(), checkpoint).await
    }
}
