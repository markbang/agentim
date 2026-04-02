# AgentIM Setup Guide

## 启动模型

`agentim` 的当前运行模型很简单：

1. 选择一个默认 agent，当前推荐 `acp`
2. 注册你提供凭证的平台 channel
3. 按需启用 webhook、Telegram polling、Discord gateway
4. 自动创建和复用 session
5. 记录 `reply_target` 并把回复发回原平台

优先级从高到低：

1. CLI 参数
2. 环境变量经 `start.sh` 转成的参数
3. `agentim.json` / `--config-file`

## 推荐模式

### 本机 / 内网

- Telegram: `--telegram-poll`
- Discord: `--discord-gateway`

这种模式最省事，不需要公网入口。

### 公网 webhook

如果你要对外暴露 HTTP 路由，建议至少打开一层认证：

- `--webhook-secret`
- `--webhook-signing-secret`
- 平台原生验签参数

## 直接命令行启动

### Telegram polling

```bash
cargo run -- \
  --agent acp \
  --acp-command /path/to/your-coding-agent \
  --acp-cwd /path/to/your/workspace \
  --telegram-token "$TELEGRAM_TOKEN" \
  --telegram-poll
```

### Discord gateway

```bash
cargo run -- \
  --agent acp \
  --acp-command /path/to/your-coding-agent \
  --acp-cwd /path/to/your/workspace \
  --discord-token "$DISCORD_TOKEN" \
  --discord-gateway
```

### Webhook 模式

```bash
cargo run -- \
  --agent acp \
  --acp-command /path/to/your-coding-agent \
  --acp-cwd /path/to/your/workspace \
  --webhook-secret change-me \
  --telegram-token "$TELEGRAM_TOKEN" \
  --discord-token "$DISCORD_TOKEN" \
  --addr 0.0.0.0:8080
```

## 用 `start.sh`

```bash
export AGENTIM_CONFIG_FILE=agentim.json
export AGENTIM_AGENT=acp
export AGENTIM_ADDR=127.0.0.1:8080
export AGENTIM_TELEGRAM_POLL=1
export AGENTIM_DISCORD_GATEWAY=1
export AGENTIM_ACP_COMMAND=/path/to/your-coding-agent
export AGENTIM_ACP_CWD=/path/to/your/workspace
export TELEGRAM_TOKEN=your-telegram-token
export DISCORD_TOKEN=your-discord-token
./start.sh
```

## 常用环境变量

- `AGENTIM_CONFIG_FILE`
- `AGENTIM_AGENT`
- `AGENTIM_ADDR`
- `AGENTIM_TELEGRAM_POLL`
- `AGENTIM_DISCORD_GATEWAY`
- `AGENTIM_ACP_COMMAND`
- `AGENTIM_ACP_CWD`
- `OPENAI_API_KEY` / `OPENAI_BASE_URL` / `OPENAI_MODEL` / `OPENAI_MAX_RETRIES`
  仅在你显式使用 `--agent openai` 时才需要
- `AGENTIM_STATE_FILE`
- `AGENTIM_STATE_BACKUP_COUNT`
- `AGENTIM_MAX_SESSION_MESSAGES`
- `AGENTIM_CONTEXT_MESSAGE_LIMIT`
- `AGENTIM_AGENT_TIMEOUT_MS`
- `AGENTIM_WEBHOOK_SECRET`
- `AGENTIM_WEBHOOK_SIGNING_SECRET`
- `AGENTIM_WEBHOOK_MAX_SKEW_SECONDS`
- `TELEGRAM_TOKEN`
- `TELEGRAM_WEBHOOK_SECRET_TOKEN`
- `DISCORD_TOKEN`
- `DISCORD_INTERACTION_PUBLIC_KEY`
- `FEISHU_APP_ID`
- `FEISHU_APP_SECRET`
- `FEISHU_WEBHOOK_VERIFICATION_TOKEN`
- `QQ_BOT_ID`
- `QQ_BOT_TOKEN`
- `SLACK_TOKEN`
- `SLACK_SIGNING_SECRET`
- `DINGTALK_TOKEN`
- `DINGTALK_SECRET`

兼容旧格式：

- `FEISHU_TOKEN=app_id:app_secret`
- `QQ_TOKEN=bot_id:bot_token`

## 状态和上下文

如果需要保留会话状态并限制单次上下文：

```bash
--state-file .agentim/sessions.json
--state-backup-count 2
--max-session-messages 50
--context-message-limit 12
--agent-timeout-ms 30000
```

## Routing Rules

示例：

```json
{
  "agent": "acp",
  "telegram_agent": "acp",
  "discord_agent": "acp",
  "routing_rules": [
    {"channel": "telegram", "user_id": "vip-user", "priority": 10, "agent": "acp"},
    {"channel": "discord", "reply_target_prefix": "review-", "priority": 1, "agent": "acp"}
  ]
}
```

## Dry-run

```bash
AGENTIM_DRY_RUN=1 ./start.sh
```

如果你要的就是 “bridge 调外部 coding agent”，优先用 `acp`。`openai` 只是内置 HTTP backend 兼容路径。

## 验证

```bash
cargo test
cargo test --test review_bridge
```
