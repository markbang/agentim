# AgentIM Setup Guide

## 运行模型

当前 `agentim` 二进制会：

1. 选择一个默认 agent（`--agent`）
2. 注册你提供凭证的 IM channel
3. 启动 webhook server
4. 对收到的消息自动创建/复用 session，并把回复发回原平台

如果你需要更复杂的“多 agent 动态分流”，建议在库层基于 `AgentIM` 扩展。

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

- `AGENTIM_AGENT`
- `AGENTIM_ADDR`
- `TELEGRAM_TOKEN`
- `DISCORD_TOKEN`
- `FEISHU_APP_ID`
- `FEISHU_APP_SECRET`
- `QQ_BOT_ID`
- `QQ_BOT_TOKEN`

兼容旧变量：
- `FEISHU_TOKEN=app_id:app_secret`
- `QQ_TOKEN=bot_id:bot_token`

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
