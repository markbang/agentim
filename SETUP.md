# AgentIM Setup Guide

## 运行模型

当前 `agentim` 二进制会：

1. 选择一个默认 agent（`--agent`，支持 `claude` / `codex` / `pi` / `openai`）
2. 可选地为不同平台设置不同 agent（`--telegram-agent` / `--discord-agent` / `--feishu-agent` / `--qq-agent`）
3. 可选地通过 `routing_rules` 为特定平台上的特定用户覆盖 agent
4. 注册你提供凭证的 IM channel
5. 启动 webhook server
6. 对收到的消息自动创建/复用 session，并把回复发回原平台

如果你需要更复杂的 workspace / 组织级策略路由，建议在库层基于 `AgentIM` 扩展。

## 方式 1：直接命令行启动

### Telegram

```bash
cargo run -- \
  --agent claude \
  --telegram-token "$TELEGRAM_TOKEN" \
  --addr 0.0.0.0:8080
```

### OpenAI-compatible backend

```bash
cargo run -- \
  --agent openai \
  --openai-api-key "$OPENAI_API_KEY" \
  --openai-base-url "${OPENAI_BASE_URL:-https://api.openai.com/v1}" \
  --openai-model "${OPENAI_MODEL:-gpt-4o-mini}" \
  --openai-max-retries 1 \
  --telegram-token "$TELEGRAM_TOKEN"
```

### Discord

```bash
cargo run -- \
  --agent codex \
  --discord-token "$DISCORD_TOKEN" \
  --discord-interaction-public-key "$DISCORD_INTERACTION_PUBLIC_KEY" \
  --addr 0.0.0.0:8080
```

### Feishu

```bash
cargo run -- \
  --agent pi \
  --feishu-app-id "$FEISHU_APP_ID" \
  --feishu-app-secret "$FEISHU_APP_SECRET"
```

首次接 webhook 时，`/feishu` 现在会直接处理 `type=url_verification` challenge。
如需额外校验，也可以配置 `--feishu-verification-token` / `FEISHU_WEBHOOK_VERIFICATION_TOKEN`。

### QQ

```bash
cargo run -- \
  --agent claude \
  --qq-bot-id "$QQ_BOT_ID" \
  --qq-bot-token "$QQ_BOT_TOKEN"
```

## 方式 2：环境变量 + `start.sh`

```bash
export AGENTIM_CONFIG_FILE=agentim.json
export AGENTIM_AGENT=claude
export AGENTIM_ADDR=127.0.0.1:8080
export TELEGRAM_TOKEN=your-token
./start.sh
```

### Dry-run

```bash
AGENTIM_DRY_RUN=1 ./start.sh
```

这个模式适合先 review 启动参数，不真正拉起 server，并且会跳过真实 IM 健康检查。

## 支持的环境变量

- `AGENTIM_CONFIG_FILE`
- `AGENTIM_AGENT`
- `AGENTIM_ADDR`
- `OPENAI_API_KEY`
- `OPENAI_BASE_URL`
- `OPENAI_MODEL`
- `OPENAI_MAX_RETRIES`
- `AGENTIM_STATE_FILE`
- `AGENTIM_STATE_BACKUP_COUNT`
- `AGENTIM_MAX_SESSION_MESSAGES`
- `AGENTIM_CONTEXT_MESSAGE_LIMIT`
- `AGENTIM_AGENT_TIMEOUT_MS`
- `AGENTIM_WEBHOOK_SECRET`
- `AGENTIM_WEBHOOK_SIGNING_SECRET`
- `AGENTIM_WEBHOOK_MAX_SKEW_SECONDS`
- `TELEGRAM_WEBHOOK_SECRET_TOKEN`
- `DISCORD_INTERACTION_PUBLIC_KEY`
- `FEISHU_WEBHOOK_VERIFICATION_TOKEN`
- `TELEGRAM_AGENT`
- `DISCORD_AGENT`
- `FEISHU_AGENT`
- `QQ_AGENT`
- `TELEGRAM_TOKEN`
- `DISCORD_TOKEN`
- `FEISHU_APP_ID`
- `FEISHU_APP_SECRET`
- `QQ_BOT_ID`
- `QQ_BOT_TOKEN`

兼容旧变量：
- `FEISHU_TOKEN=app_id:app_secret`
- `QQ_TOKEN=bot_id:bot_token`

## JSON 配置里的用户级路由规则

示例：

```json
{
  "agent": "claude",
  "telegram_agent": "codex",
  "routing_rules": [
    {"channel": "telegram", "user_id": "vip-user", "priority": 10, "agent": "pi"},
    {"channel": "discord", "reply_target_prefix": "review-", "priority": 1, "agent": "codex"}
  ]
}
```

优先级：
1. `routing_rules` 命中（先比 `priority`，再比规则具体度；可按 `channel` + `user_id`、`reply_target`，或 prefix 规则匹配）
2. 平台级 agent override
3. 默认 `agent`

## Session 历史控制

你可以在 JSON 配置或 CLI 中设置：

```json
{
  "max_session_messages": 50,
  "context_message_limit": 12,
  "agent_timeout_ms": 30000,
  "state_backup_count": 2
}
```

`max_session_messages` 控制 session 最终保留多少历史；`context_message_limit` 控制每次实际送进 agent 的上下文窗口；`agent_timeout_ms` 控制单次 agent 调用的最长耗时。
这样可以保留较长的本地会话历史，同时避免每轮都把全部历史塞给 agent，也避免真实 agent backend 把 webhook 长时间挂死。
如果主状态文件损坏，启动时还会尝试从最近的备份快照恢复。

## Webhook 路由

- `/telegram`
- `/discord`
- `/feishu`
- `/qq`

## Setup 后建议立刻执行的检查

```bash
cargo test --test review_bridge
./autoresearch.sh
```

这两个检查分别负责：
- **review**：功能回归
- **eval**：结构化完成度评估

## 生产建议

- 使用 HTTPS 暴露 webhook
- 为各平台增加签名校验
- 把 session 持久化接到真实存储
- 为真实 agent API 调用增加超时和重试
