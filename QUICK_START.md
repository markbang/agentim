# AgentIM Quick Start

## 30 秒启动

生产模式只支持真实 backend：`openai` 或 `acp`。内置 `claude` / `codex` / `pi` 仅用于开发和 dry-run。

### 方式 0：本机 Telegram Long Polling

```bash
cargo run -- \
  --agent openai \
  --openai-api-key "$OPENAI_API_KEY" \
  --openai-base-url "${OPENAI_BASE_URL:-https://api.openai.com/v1}" \
  --openai-model "${OPENAI_MODEL:-gpt-4o-mini}" \
  --telegram-token "$TELEGRAM_TOKEN" \
  --telegram-poll \
  --state-file .agentim/sessions.json \
  --state-backup-count 2
```

这个模式不需要公网 URL，会自动切回 `getUpdates` 长轮询。

### 方式 1：直接运行

```bash
cargo run -- \
  --agent openai \
  --openai-api-key "$OPENAI_API_KEY" \
  --openai-base-url "${OPENAI_BASE_URL:-https://api.openai.com/v1}" \
  --openai-model "${OPENAI_MODEL:-gpt-4o-mini}" \
  --webhook-secret "change-me" \
  --telegram-token "$TELEGRAM_TOKEN" \
  --addr 127.0.0.1:8080
```

如果你希望不同平台走不同 agent：

```bash
cargo run -- \
  --agent openai \
  --openai-api-key "$OPENAI_API_KEY" \
  --openai-base-url "${OPENAI_BASE_URL:-https://api.openai.com/v1}" \
  --openai-model "${OPENAI_MODEL:-gpt-4o-mini}" \
  --telegram-agent acp \
  --acp-command /path/to/acp-agent \
  --webhook-signing-secret "change-me-signing" \
  --telegram-token "$TELEGRAM_TOKEN" \
  --discord-token "$DISCORD_TOKEN"
```

如果你希望直接接一个真实 OpenAI-compatible agent backend：

```bash
cargo run -- \
  --agent openai \
  --openai-api-key "$OPENAI_API_KEY" \
  --openai-base-url "${OPENAI_BASE_URL:-https://api.openai.com/v1}" \
  --openai-model "${OPENAI_MODEL:-gpt-4o-mini}" \
  --openai-max-retries 1 \
  --telegram-token "$TELEGRAM_TOKEN"
```

Server 启动后会监听：
- `POST /telegram`
- `POST /discord`
- `POST /feishu`
- `POST /qq`

### 方式 2：用 `start.sh`

```bash
export AGENTIM_CONFIG_FILE=agentim.json
export AGENTIM_AGENT=openai
export AGENTIM_ADDR=127.0.0.1:8080
export AGENTIM_TELEGRAM_POLL=1
export OPENAI_API_KEY=...
export OPENAI_BASE_URL=https://api.openai.com/v1
export OPENAI_MODEL=gpt-4o-mini
export OPENAI_MAX_RETRIES=1
export AGENTIM_STATE_FILE=.agentim/sessions.json
export AGENTIM_STATE_BACKUP_COUNT=2
export AGENTIM_MAX_SESSION_MESSAGES=50
export AGENTIM_CONTEXT_MESSAGE_LIMIT=12
export AGENTIM_AGENT_TIMEOUT_MS=30000
export AGENTIM_WEBHOOK_SECRET=change-me
export AGENTIM_WEBHOOK_SIGNING_SECRET=change-me-signing
export AGENTIM_WEBHOOK_MAX_SKEW_SECONDS=300
export TELEGRAM_WEBHOOK_SECRET_TOKEN=tg-native-secret
export DISCORD_INTERACTION_PUBLIC_KEY=discord-public-key-hex
export FEISHU_WEBHOOK_VERIFICATION_TOKEN=feishu-native-token
export TELEGRAM_TOKEN=your-token
./start.sh
```

如果你要把某个用户路由到特殊 agent，在 `agentim.json` 里加：

```json
{
  "routing_rules": [
    {"channel": "telegram", "user_id": "vip-user", "priority": 10, "agent": "acp"},
    {"channel": "discord", "reply_target_prefix": "review-", "priority": 1, "agent": "openai"}
  ]
}
```

先 review 一下启动参数：

```bash
AGENTIM_DRY_RUN=1 ./start.sh
cargo run -- --dry-run --agent claude --telegram-agent pi
```

`--dry-run` / `AGENTIM_DRY_RUN=1` 会跳过真实 IM 健康检查，适合离线验证配置。
需要控制 prompt 窗口时，可额外设置 `AGENTIM_CONTEXT_MESSAGE_LIMIT` 或 `--context-message-limit`。

## 常用凭证

```bash
export TELEGRAM_TOKEN=...
export DISCORD_TOKEN=...
export FEISHU_APP_ID=...
export FEISHU_APP_SECRET=...
export QQ_BOT_ID=...
export QQ_BOT_TOKEN=...
```

兼容旧格式：

```bash
export FEISHU_TOKEN="app_id:app_secret"
export QQ_TOKEN="bot_id:bot_token"
```

## 快速验证

```bash
cargo test --test review_bridge
./autoresearch.sh
```

这两个命令分别做：
- **review**：验证核心 webhook/session/reply-target 行为
- **eval**：输出结构化 acceptance metrics
