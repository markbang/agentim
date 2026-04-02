# AgentIM Quick Start

## 最短路径

如果你只是想在本机把 bot 跑起来，并把消息桥接给 ACP coding agent，优先用：

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
  --agent acp \
  --acp-command /path/to/your-coding-agent \
  --acp-cwd /path/to/your/workspace \
  --telegram-token "$TELEGRAM_TOKEN" \
  --telegram-poll \
  --state-file .agentim/sessions.json \
  --state-backup-count 2
```

## 3. 本机 Discord

```bash
agentim \
  --agent acp \
  --acp-command /path/to/your-coding-agent \
  --acp-cwd /path/to/your/workspace \
  --discord-token "$DISCORD_TOKEN" \
  --discord-gateway \
  --state-file .agentim/sessions.json \
  --state-backup-count 2
```

## 4. 同时接 Telegram 和 Discord

```bash
agentim \
  --agent acp \
  --acp-command /path/to/your-coding-agent \
  --acp-cwd /path/to/your/workspace \
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
export AGENTIM_AGENT=acp
export AGENTIM_ACP_COMMAND=/path/to/your-coding-agent
export AGENTIM_ACP_CWD=/path/to/your/workspace
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

如果你确实不走 ACP，而是要让 AgentIM 自己直连 OpenAI-compatible HTTP，再参考 [README.md](README.md) 里的可选内置 backend 部分。
