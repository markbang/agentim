# AgentIM

AgentIM 是一个用 Rust 写的 IM bridge。它负责接收机器人平台消息，维护按用户划分的会话，把上下文发给 AI backend，再把回复发回正确的聊天目标。

这个仓库当前更适合两类场景：

- 在本机或内网直接跑 Telegram / Discord bot，不想暴露公网 webhook
- 用一个统一进程接多个 IM 平台，并做会话持久化、路由和基础安全校验

## 当前定位

当前二进制 `agentim` 是一个可运行的 bridge 进程，不是完整的多租户 agent 编排系统。

- 真实 backend：`openai`、`acp`
- 开发 stub：`claude`、`codex`、`pi`
- 本地直连模式：Telegram long polling、Discord Gateway
- Webhook 模式：Telegram、Discord、Feishu/Lark、QQ、Slack、DingTalk

生产环境里，真实运行的 bot server 只应使用 `openai` 或 `acp`。`claude` / `codex` / `pi` 仅保留给开发和 dry-run。

## 支持范围

### Agent backends

- `openai`
  - 兼容 OpenAI `/chat/completions`
  - 支持 `--openai-base-url`、`--openai-model`、`--openai-max-retries`
- `acp`
  - 通过子进程接 ACP-compatible agent
- `claude` / `codex` / `pi`
  - 本地 stub，仅用于开发验证

### IM ingress

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

### 内置运维端点

- `GET /healthz`
- `GET /reviewz`

## 核心能力

- 本地优先：Telegram long polling 和 Discord Gateway 不需要公网 webhook
- 多平台统一会话：自动创建和复用 session，并维护 `reply_target`
- 路由能力：支持默认 agent、平台级 agent 覆盖、按规则路由
- 持久化：可选 `--state-file`，支持快照轮转和恢复
- 上下文控制：支持历史裁剪、独立 context window、agent timeout
- 安全校验：支持共享密钥、HMAC 签名、Telegram/Discord/Feishu/Slack 原生校验

## 安装

### 一键安装

安装最新 release：

```bash
curl -fsSL https://raw.githubusercontent.com/markbang/agentim/main/install.sh | bash
```

默认会安装到 `~/.local/bin/agentim`。

常见自定义方式：

```bash
# 指定安装目录
curl -fsSL https://raw.githubusercontent.com/markbang/agentim/main/install.sh | AGENTIM_INSTALL_DIR=/usr/local/bin bash

# 固定安装某个版本
curl -fsSL https://raw.githubusercontent.com/markbang/agentim/main/install.sh | AGENTIM_VERSION=v0.3.0 bash
```

安装脚本当前支持：

- Linux x86_64
- macOS x86_64
- macOS Apple Silicon

Windows 请直接从 release 页面下载 zip 包：

`https://github.com/markbang/agentim/releases`

### 从源码运行

```bash
cargo build --release
./target/release/agentim --help
```

开发时也可以直接：

```bash
cargo run -- --help
```

## 快速开始

### 1. 本机 Telegram bridge

这是最简单的本地模式，不需要公网 URL：

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

### 2. 本机 Discord bridge

Discord 本地模式走 Gateway websocket，同样不需要公网 webhook：

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

### 3. 同时接 Telegram 和 Discord

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

### 4. 用 `start.sh` 和配置文件启动

先复制示例配置：

```bash
cp agentim.json.example agentim.json
```

然后用环境变量补运行时凭证：

```bash
export AGENTIM_CONFIG_FILE=agentim.json
export AGENTIM_AGENT=openai
export OPENAI_API_KEY=your-api-key
export TELEGRAM_TOKEN=your-telegram-token
export DISCORD_TOKEN=your-discord-token
export AGENTIM_TELEGRAM_POLL=1
export AGENTIM_DISCORD_GATEWAY=1
./start.sh
```

`start.sh` 会优先读取 `agentim.json`，再叠加环境变量。CLI 参数仍然拥有最高优先级。

### 5. Dry-run 校验

在真正启动前，可以离线检查配置是否合理：

```bash
AGENTIM_DRY_RUN=1 ./start.sh
```

或：

```bash
agentim --dry-run --agent openai --openai-api-key dummy --telegram-token dummy --telegram-poll
```

Dry-run 会跳过真实 IM 健康检查，适合先确认参数、配置文件和凭证格式。

## Webhook 部署

如果你不是跑本地 polling / gateway，而是对外提供 webhook，当前路由是：

- `POST /telegram`
- `POST /discord`
- `POST /feishu`
- `POST /qq`
- `POST /slack`
- `POST /dingtalk`

建议至少打开一层安全校验：

- 全局共享密钥：`--webhook-secret`
- 全局 HMAC 签名：`--webhook-signing-secret`
- Telegram 原生校验：`--telegram-webhook-secret-token`
- Discord 原生校验：`--discord-interaction-public-key`
- Feishu 原生校验：`--feishu-verification-token`
- Slack 原生校验：`--slack-signing-secret`

本地 Telegram polling 和 Discord Gateway 不依赖这些 webhook 路由。

## 会话、路由和状态

### 会话持久化

```bash
--state-file .agentim/sessions.json
--state-backup-count 2
```

- 启动时自动恢复 session
- 写入时生成 `.bak.N` 轮转快照
- 主快照损坏时会尝试回退到最近可用备份

### 上下文控制

```bash
--max-session-messages 50
--context-message-limit 12
--agent-timeout-ms 30000
```

- `max-session-messages`：控制存储的历史上限
- `context-message-limit`：控制每次送进 backend 的上下文窗口
- `agent-timeout-ms`：限制单次 agent 调用耗时

### 路由规则

支持三层路由：

1. 默认 agent：`--agent`
2. 平台级 override：`--telegram-agent`、`--discord-agent` 等
3. `routing_rules`：按用户、前缀、reply target 和 priority 细分

示例：

```json
{
  "agent": "openai",
  "telegram_agent": "acp",
  "discord_agent": "openai",
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
      "agent": "openai"
    }
  ]
}
```

## 配置文件示例

完整示例见 [agentim.json.example](agentim.json.example)。

常见本地配置大致如下：

```json
{
  "agent": "openai",
  "openai_api_key": "YOUR_OPENAI_API_KEY",
  "openai_base_url": "https://api.openai.com/v1",
  "openai_model": "gpt-4o-mini",
  "telegram_token": "YOUR_TELEGRAM_TOKEN",
  "telegram_poll": true,
  "discord_token": "YOUR_DISCORD_TOKEN",
  "discord_gateway": true,
  "state_file": ".agentim/sessions.json",
  "state_backup_count": 2,
  "context_message_limit": 12,
  "agent_timeout_ms": 30000
}
```

## Release 资产

当前 release 会生成：

- `agentim-linux-x86_64.tar.gz`
- `agentim-macos-x86_64.tar.gz`
- `agentim-macos-aarch64.tar.gz`
- `agentim-windows-x86_64.zip`
- 每个包对应的 `sha256`

发布页：

`https://github.com/markbang/agentim/releases`

## 开发与验证

常用命令：

```bash
cargo test
cargo test --test review_bridge
AGENTIM_DRY_RUN=1 ./start.sh
./autoresearch.sh
```

其中：

- `cargo test`：单元测试和集成测试
- `review_bridge`：重点验证 webhook、session、reply target、鉴权和持久化
- `autoresearch.sh`：输出结构化 acceptance metrics

## 其他文档

- [QUICK_START.md](QUICK_START.md)
- [SETUP.md](SETUP.md)
- [BOT_INTEGRATION.md](BOT_INTEGRATION.md)
- [ARCHITECTURE.md](ARCHITECTURE.md)
