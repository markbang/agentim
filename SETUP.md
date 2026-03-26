# AgentIM Setup Guide

## 运行模型

当前 `agentim` 二进制会：

1. 选择一个默认 agent（`--agent`）
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

### Discord

```bash
cargo run -- \
  --agent codex \
  --discord-token "$DISCORD_TOKEN" \
  --addr 0.0.0.0:8080
```

### Feishu

```bash
cargo run -- \
  --agent pi \
  --feishu-app-id "$FEISHU_APP_ID" \
  --feishu-app-secret "$FEISHU_APP_SECRET"
```

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

这个模式适合先 review 启动参数，不真正拉起 server。

## 支持的环境变量

- `AGENTIM_CONFIG_FILE`
- `AGENTIM_AGENT`
- `AGENTIM_ADDR`
- `AGENTIM_STATE_FILE`
- `AGENTIM_MAX_SESSION_MESSAGES`
- `AGENTIM_WEBHOOK_SECRET`
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
    {"channel": "telegram", "user_id": "vip-user", "agent": "pi"},
    {"channel": "discord", "reply_target": "review-room", "agent": "codex"}
  ]
}
```

优先级：
1. `routing_rules` 命中（可按 `channel` + `user_id` 或 `reply_target` 匹配）
2. 平台级 agent override
3. 默认 `agent`

## Session 历史控制

你可以在 JSON 配置或 CLI 中设置：

```json
{
  "max_session_messages": 50
}
```

超过上限后，旧消息会被裁掉，只保留最近的消息窗口，避免 session 快速膨胀。

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
