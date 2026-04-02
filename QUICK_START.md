# AgentIM Quick Start

## 最短路径

如果你只是想在本机把 bot 跑起来，优先用：

- Telegram: `--telegram-poll`
- Discord: `--discord-gateway`

这两种模式都不需要公网 webhook。

## 1. 安装

```bash
curl -fsSL https://raw.githubusercontent.com/markbang/agentim/main/install.sh | bash
```

或直接本地运行：

```bash
cargo run -- --help
```

## 2. 本机 Telegram

```bash
agentim \
  --agent openai \
  --openai-api-key "$OPENAI_API_KEY" \
  --openai-base-url "${OPENAI_BASE_URL:-https://api.openai.com/v1}" \
  --openai-model "${OPENAI_MODEL:-gpt-4o-mini}" \
  --telegram-token "$TELEGRAM_TOKEN" \
  --telegram-poll \
  --state-file .agentim/sessions.json \
  --state-backup-count 2
```

## 3. 本机 Discord

```bash
agentim \
  --agent openai \
  --openai-api-key "$OPENAI_API_KEY" \
  --openai-base-url "${OPENAI_BASE_URL:-https://api.openai.com/v1}" \
  --openai-model "${OPENAI_MODEL:-gpt-4o-mini}" \
  --discord-token "$DISCORD_TOKEN" \
  --discord-gateway \
  --state-file .agentim/sessions.json \
  --state-backup-count 2
```

## 4. 同时接 Telegram 和 Discord

```bash
agentim \
  --agent openai \
  --openai-api-key "$OPENAI_API_KEY" \
  --telegram-token "$TELEGRAM_TOKEN" \
  --telegram-poll \
  --discord-token "$DISCORD_TOKEN" \
  --discord-gateway \
  --state-file .agentim/sessions.json
```

## 5. 用 `start.sh`

```bash
cp agentim.json.example agentim.json

export AGENTIM_CONFIG_FILE=agentim.json
export AGENTIM_AGENT=openai
export OPENAI_API_KEY=your-api-key
export TELEGRAM_TOKEN=your-telegram-token
export DISCORD_TOKEN=your-discord-token
export AGENTIM_TELEGRAM_POLL=1
export AGENTIM_DISCORD_GATEWAY=1
./start.sh
```

## 6. Dry-run

```bash
AGENTIM_DRY_RUN=1 ./start.sh
```

这个模式会跳过真实平台健康检查，适合先验配置。

## 7. 基础验证

```bash
cargo test
cargo test --test review_bridge
```
