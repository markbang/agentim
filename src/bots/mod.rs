pub mod discord;
pub mod feishu;
pub mod qq;
pub mod telegram;

pub use discord::{DiscordBotChannel, DISCORD_CHANNEL_ID};
pub use feishu::{FeishuBotChannel, FEISHU_CHANNEL_ID};
pub use qq::{QQBotChannel, QQ_CHANNEL_ID};
pub use telegram::{TelegramBotChannel, TELEGRAM_CHANNEL_ID};
