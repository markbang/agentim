pub mod discord;
pub mod feishu;
pub mod qq;
pub mod telegram;

pub use discord::{DiscordBotChannel, DiscordMessage, DISCORD_CHANNEL_ID};
pub use feishu::{FeishuBotChannel, FeishuMessage, FEISHU_CHANNEL_ID};
pub use qq::{QQBotChannel, QQMessage, QQ_CHANNEL_ID};
pub use telegram::{TelegramBotChannel, TelegramUpdate, TELEGRAM_CHANNEL_ID};
