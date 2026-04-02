# AgentIM

AgentIM 是一个 IM bridge。它负责接收 Telegram / Discord / Feishu / QQ / Slack / DingTalk 的 bot 消息，维护 session，把消息转给 agent，再把回复发回原平台。

这个项目的主目标不是自己管理模型 provider 或 API key，而是桥接到外部 agent，尤其是支持 ACP 的 coding agent。

## 当前主模型

当前推荐路径：

- `agentim` 负责 IM 接入、session、reply target、状态恢复
- 外部 ACP agent 负责 provider、model、key、工具调用和实际推理

也就是说，推荐拓扑是：

```text
IM platform -> AgentIM -> ACP coding agent
```

`openai` 仍然保留为内置 HTTP backend，但它只是兼容后备选项，不是这个仓库的主路径。

## 支持的 ingress

- Telegram
  - `POST /telegram`
  - `--telegram-poll`
- Discord
  - `POST /discord`
  - `--discord-gateway`
- Feishu / Lark
  - `POST /feishu`
- QQ
  - `POST /qq`
- Slack
  - `POST /slack`
- DingTalk
  - `POST /dingtalk`

运维端点：

- `GET /healthz`
- `GET /reviewz`

## 安装

```bash
curl -fsSL https://raw.githubusercontent.com/markbang/agentim/main/install.sh | bash
```

默认安装到 `~/.local/bin/agentim`。

## ACP 快速开始

### 本机 Telegram

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

### 本机 Discord

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

### 同时接 Telegram 和 Discord

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

## 用 `start.sh`

先复制示例配置：

```bash
cp agentim.json.example agentim.json
```

然后补运行时环境：

```bash
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

如果设置了 `AGENTIM_ACP_COMMAND` 但没显式给 `AGENTIM_AGENT`，`start.sh` 也会自动推断成 `acp`。

## Dry-run

```bash
AGENTIM_DRY_RUN=1 \
AGENTIM_AGENT=acp \
AGENTIM_ACP_COMMAND=/bin/true \
AGENTIM_TELEGRAM_POLL=1 \
TELEGRAM_TOKEN=dummy \
./start.sh
```

Dry-run 会跳过真实 IM 健康检查，适合先验证 bridge 配置。

## 状态与上下文

常用参数：

```bash
--state-file .agentim/sessions.json
--state-backup-count 2
--max-session-messages 50
--context-message-limit 12
--agent-timeout-ms 30000
```

## Webhook 安全

如果你对外暴露 webhook，至少开启一层校验：

- `--webhook-secret`
- `--webhook-signing-secret`
- `--telegram-webhook-secret-token`
- `--discord-interaction-public-key`
- `--feishu-verification-token`
- `--slack-signing-secret`

本地 Telegram polling 和 Discord Gateway 不依赖这些 webhook 入口。

## Routing Rules

示例：

```json
{
  "agent": "acp",
  "telegram_agent": "acp",
  "discord_agent": "acp",
  "routing_rules": [
    {
      "channel": "telegram",
      "user_id": "vip-user",
      "priority": 10,
      "agent": "acp"
    },
    {
      "channel": "discord",
      "reply_target_prefix": "review-",
      "priority": 1,
      "agent": "acp"
    }
  ]
}
```

## 可选内置 backend

如果你确实想让 AgentIM 自己直连 OpenAI-compatible HTTP，也仍然支持：

只有显式选择 `--agent openai` 时，才需要给 AgentIM 配 `--openai-api-key` 这类参数。ACP 主路径下这些都不需要，provider / model / key 继续由外部 agent 自己管理。

```bash
agentim \
  --agent openai \
  --openai-api-key "$OPENAI_API_KEY" \
  --openai-base-url "${OPENAI_BASE_URL:-https://api.openai.com/v1}" \
  --openai-model "${OPENAI_MODEL:-gpt-4o-mini}" \
  --telegram-token "$TELEGRAM_TOKEN" \
  --telegram-poll
```

但这不是当前仓库的主目标。

## 开发与验证

先按 `prek` 官方 quickstart 安装本地命令：

`https://prek.j178.dev/quickstart/`

常用命令：

```bash
prek install
prek run --all-files
prek run --all-files --hook-stage pre-push
cargo test
cargo test --test review_bridge
AGENTIM_DRY_RUN=1 ./start.sh
./autoresearch.sh
```

## 其他文档

- [QUICK_START.md](QUICK_START.md)
- [SETUP.md](SETUP.md)
- [BOT_INTEGRATION.md](BOT_INTEGRATION.md)
- [ARCHITECTURE.md](ARCHITECTURE.md)
