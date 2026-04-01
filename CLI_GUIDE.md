# AgentIM CLI 使用指南

## 当前运行模型

当前 `agentim` 不是“资源注册型”多子命令 CLI，而是一个**参数驱动的 webhook bridge 进程**：

1. 选择默认 agent（`--agent`）
2. 可按平台覆盖 agent（`--telegram-agent` / `--discord-agent` / `--feishu-agent` / `--qq-agent`）
3. 可通过 `routing_rules` 做用户级 / reply-target 级路由
4. 注册你提供凭证的平台 channel
5. 启动 HTTP server，接收各平台 webhook
6. 自动创建 / 复用 session，并把回复回发到对应平台

如果你要扩展更复杂的组织级策略、鉴权或平台适配，请直接使用库层 API，而不是寻找旧版 `agent register` / `channel register` 子命令。

## 最常用命令

### 查看参数

```bash
cargo run -- --help
```

### 本地 dry-run 校验启动参数

```bash
cargo run -- --dry-run --agent claude --telegram-token "$TELEGRAM_TOKEN"

# 或使用环境变量包装脚本
AGENTIM_DRY_RUN=1 ./start.sh
AGENTIM_DRY_RUN=1 ./setup.sh
```

### 启动 Telegram bridge

```bash
cargo run -- \
  --agent claude \
  --telegram-token "$TELEGRAM_TOKEN" \
  --addr 127.0.0.1:8080
```

### 启动多平台 bridge

```bash
cargo run -- \
  --agent claude \
  --telegram-agent pi \
  --discord-agent codex \
  --telegram-token "$TELEGRAM_TOKEN" \
  --discord-token "$DISCORD_TOKEN" \
  --feishu-app-id "$FEISHU_APP_ID" \
  --feishu-app-secret "$FEISHU_APP_SECRET" \
  --qq-bot-id "$QQ_BOT_ID" \
  --qq-bot-token "$QQ_BOT_TOKEN"
```

### 使用 OpenAI-compatible backend

```bash
cargo run -- \
  --agent openai \
  --openai-api-key "$OPENAI_API_KEY" \
  --openai-base-url "${OPENAI_BASE_URL:-https://api.openai.com/v1}" \
  --openai-model "${OPENAI_MODEL:-gpt-4o-mini}" \
  --openai-max-retries 1 \
  --telegram-token "$TELEGRAM_TOKEN"
```

## `start.sh` 与 `setup.sh`

### `start.sh`

推荐的启动包装脚本，读取环境变量后拼出真实命令，并在二进制缺失或过期时自动执行 `cargo build --release`。

```bash
export AGENTIM_CONFIG_FILE=agentim.json
export AGENTIM_AGENT=claude
export AGENTIM_ADDR=127.0.0.1:8080
export TELEGRAM_TOKEN=your-token
./start.sh
```

### `setup.sh`

当前 `setup.sh` 是一个**兼容包装器**，会：

- 加载 `.env`（如果存在）
- 兼容旧变量名 `TELEGRAM_BOT_TOKEN` / `DISCORD_BOT_TOKEN`
- 在存在 `agentim.json` 时自动设置 `AGENTIM_CONFIG_FILE`
- 委托给 `./start.sh`

适合把旧文档或旧本地环境平滑迁移到当前启动方式。

## 主要参数

### Agent 选择

- `--agent`
- `--telegram-agent`
- `--discord-agent`
- `--feishu-agent`
- `--qq-agent`

支持值：`claude`、`codex`、`pi`、`openai`

### OpenAI-compatible backend

- `--openai-api-key`
- `--openai-base-url`
- `--openai-model`
- `--openai-max-retries`

### 平台凭证

- `--telegram-token`
- `--telegram-webhook-secret-token`
- `--discord-token`
- `--discord-interaction-public-key`
- `--feishu-app-id`
- `--feishu-app-secret`
- `--feishu-verification-token`
- `--qq-bot-id`
- `--qq-bot-token`

兼容旧格式：

- `--feishu-token app_id:app_secret`
- `--qq-token bot_id:bot_token`

### Session / 运行时控制

- `--state-file`
- `--state-backup-count`
- `--max-session-messages`
- `--context-message-limit`
- `--agent-timeout-ms`
- `--config-file`
- `--dry-run`
- `--addr`

### Webhook 安全

- `--webhook-secret`
- `--webhook-signing-secret`
- `--webhook-max-skew-seconds`
- `--telegram-webhook-secret-token`
- `--discord-interaction-public-key`
- `--feishu-verification-token`

## 配置文件

可以通过 `--config-file agentim.json` 或 `AGENTIM_CONFIG_FILE=agentim.json` 提供 JSON 配置，命令行参数优先级高于配置文件。

示例：

```json
{
  "agent": "claude",
  "telegram_agent": "pi",
  "discord_agent": "codex",
  "routing_rules": [
    {"channel": "telegram", "user_id": "vip-user", "priority": 10, "agent": "claude"},
    {"channel": "discord", "reply_target_prefix": "review-", "priority": 1, "agent": "pi"}
  ],
  "telegram_token": "YOUR_TELEGRAM_TOKEN",
  "state_file": ".agentim/sessions.json",
  "state_backup_count": 2,
  "max_session_messages": 50,
  "context_message_limit": 12,
  "agent_timeout_ms": 30000,
  "addr": "127.0.0.1:8080"
}
```

## Webhook 与运维端点

启动后默认会暴露：

- `POST /telegram`
- `POST /discord`
- `POST /feishu`
- `POST /qq`
- `GET /healthz`
- `GET /reviewz`

如果配置了 `--webhook-secret`，访问受保护端点时需要携带 `x-agentim-secret`。

## 测试与巡检

```bash
cargo test --test review_bridge
./autoresearch.sh
```

- `review_bridge`：验证 webhook、路由、session、reply-target、安全和持久化关键路径
- `autoresearch.sh`：输出结构化 acceptance metrics，适合首轮巡检或回归

## 排障建议

### `cargo` 构建失败

优先确认：

```bash
rustc --version
cargo --version
cargo run -- --help
```

当前仓库通过 `rust-toolchain.toml` 固定到 Rust/Cargo 1.85.0，并应在该工具链下完成解析与构建。

### dry-run 通过，但真实启动失败

重点检查：

- 平台 token / app secret 是否正确
- webhook 回调地址是否可被平台访问
- 是否缺少原生平台签名校验参数
- 是否误把 `reply_target` 路由规则配置成用户 ID 规则

### 收到 webhook，但没有正确回发

重点检查：

- `routing_rules` 是否把消息路由到了预期 agent
- 是否配置了 `state_file` 导致旧 session 被恢复
- Discord / QQ 的 `reply_target` 是否按频道维度匹配
