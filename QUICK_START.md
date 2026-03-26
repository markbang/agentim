# AgentIM Quick Start

## 30 秒启动

### 方式 1：直接运行

```bash
cargo run -- \
  --agent claude \
  --telegram-token "$TELEGRAM_TOKEN" \
  --addr 127.0.0.1:8080
```

如果你希望不同平台走不同 agent：

```bash
cargo run -- \
  --agent claude \
  --telegram-agent pi \
  --discord-agent codex \
  --telegram-token "$TELEGRAM_TOKEN" \
  --discord-token "$DISCORD_TOKEN"
```

Server 启动后会监听：
- `POST /telegram`
- `POST /discord`
- `POST /feishu`
- `POST /qq`

### 方式 2：用 `start.sh`

```bash
export AGENTIM_AGENT=claude
export AGENTIM_ADDR=127.0.0.1:8080
export TELEGRAM_TOKEN=your-token
./start.sh
```

先 review 一下启动参数：

```bash
AGENTIM_DRY_RUN=1 ./start.sh
```

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
