# AgentIM Bot Integration Guide

## 总览

AgentIM 当前支持两种接入形态：

- 本地直连
  - Telegram `getUpdates` long polling
  - Discord Gateway
- Webhook
  - Telegram
  - Discord
  - Feishu / Lark
  - QQ
  - Slack
  - DingTalk

统一处理流程是：

1. 接收平台消息
2. 解析用户和 `reply_target`
3. 查找或创建 session
4. 调用 agent backend
5. 把回复发回原平台目标

当前推荐 backend 是 `acp`。也就是 AgentIM 只负责 bridge，provider / model / key 都由外部 coding agent 自己管理。

## 当前路由

- `POST /telegram`
- `POST /discord`
- `POST /feishu`
- `POST /qq`
- `POST /slack`
- `POST /dingtalk`
- `GET /healthz`
- `GET /reviewz`

## Telegram

### 本地模式

```bash
agentim \
  --agent acp \
  --acp-command /path/to/your-coding-agent \
  --acp-cwd /path/to/your/workspace \
  --telegram-token "$TELEGRAM_TOKEN" \
  --telegram-poll
```

### Webhook 模式

```bash
agentim \
  --agent acp \
  --acp-command /path/to/your-coding-agent \
  --acp-cwd /path/to/your/workspace \
  --telegram-token "$TELEGRAM_TOKEN" \
  --webhook-secret change-me
```

可选原生校验：

- `--telegram-webhook-secret-token`

回复目标：

- `chat.id`

## Discord

### 本地模式

```bash
agentim \
  --agent acp \
  --acp-command /path/to/your-coding-agent \
  --acp-cwd /path/to/your/workspace \
  --discord-token "$DISCORD_TOKEN" \
  --discord-gateway
```

### Webhook 模式

```bash
agentim \
  --agent acp \
  --acp-command /path/to/your-coding-agent \
  --acp-cwd /path/to/your/workspace \
  --discord-token "$DISCORD_TOKEN" \
  --discord-interaction-public-key "$DISCORD_INTERACTION_PUBLIC_KEY"
```

回复目标：

- `channel_id`

## Feishu / Lark

```bash
agentim \
  --agent acp \
  --acp-command /path/to/your-coding-agent \
  --acp-cwd /path/to/your/workspace \
  --feishu-app-id "$FEISHU_APP_ID" \
  --feishu-app-secret "$FEISHU_APP_SECRET"
```

可选原生校验：

- `--feishu-verification-token`

回复目标：

- `sender_id.user_id`

## QQ

```bash
agentim \
  --agent acp \
  --acp-command /path/to/your-coding-agent \
  --acp-cwd /path/to/your/workspace \
  --qq-bot-id "$QQ_BOT_ID" \
  --qq-bot-token "$QQ_BOT_TOKEN"
```

回复目标：

- `channel_id`

兼容旧格式：

- `--qq-token bot_id:bot_token`

## Slack

```bash
agentim \
  --agent acp \
  --acp-command /path/to/your-coding-agent \
  --acp-cwd /path/to/your/workspace \
  --slack-token "$SLACK_TOKEN" \
  --slack-signing-secret "$SLACK_SIGNING_SECRET"
```

## DingTalk

```bash
agentim \
  --agent acp \
  --acp-command /path/to/your-coding-agent \
  --acp-cwd /path/to/your/workspace \
  --dingtalk-token "$DINGTALK_TOKEN" \
  --dingtalk-secret "$DINGTALK_SECRET"
```

## 安全建议

如果走 webhook，至少启用以下之一：

- `--webhook-secret`
- `--webhook-signing-secret`
- 平台原生验签

其中：

- Telegram 支持 `--telegram-webhook-secret-token`
- Discord 支持 `--discord-interaction-public-key`
- Feishu 支持 `--feishu-verification-token`
- Slack 支持 `--slack-signing-secret`

## 状态和恢复

推荐同时开启：

```bash
--state-file .agentim/sessions.json
--state-backup-count 2
```

这样 session 会在重启后恢复，并保留最近快照备份。

## 回归检查

```bash
cargo test
cargo test --test review_bridge
```
