pub mod dingtalk;
pub mod discord;
pub mod feishu;
pub mod line;
pub mod qq;
pub mod slack;
pub mod telegram;
pub mod wechatwork;

pub use dingtalk::{DingTalkBotChannel, DINGTALK_CHANNEL_ID};
pub use discord::{DiscordBotChannel, DISCORD_CHANNEL_ID};
pub use feishu::{FeishuBotChannel, FEISHU_CHANNEL_ID};
#[allow(unused_imports)]
pub use line::{LineBotChannel, LINE_CHANNEL_ID};
pub use qq::{QQBotChannel, QQ_CHANNEL_ID};
pub use slack::{SlackBotChannel, SLACK_CHANNEL_ID};
pub use telegram::{TelegramBotChannel, TELEGRAM_CHANNEL_ID};
#[allow(unused_imports)]
pub use wechatwork::{WeChatWorkBotChannel, WECHATWORK_CHANNEL_ID};
