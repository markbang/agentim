# AgentIM

一个用 Rust 编写的 IM bridge，用来把多个 AI agent 接到多个 IM 平台上，并统一管理 session、上下文和回复目标。

当前仓库有两层能力：

1. **库能力**：`AgentIM` 提供 agent/channel/session 抽象，适合二次开发。
2. **可运行二进制**：`agentim` 可以启动 webhook server，也可以直接用 Telegram long polling / Discord Gateway 在本地收消息，再把回复发回对应平台。

> 当前二进制会注册一个默认 agent，并支持两级路由：
> 1. 按平台覆盖：`--telegram-agent`、`--discord-agent`、`--feishu-agent`、`--qq-agent`
> 2. 按配置规则覆盖：`routing_rules`（可按用户/目标/前缀匹配，并支持 `priority`）
>
> 更复杂的 workspace / 组织级策略路由，仍然更适合直接用库层扩展。

## 当前支持

### Agents
- Claude（本地模拟）
- Codex（本地模拟）
- Pi（本地模拟）
- OpenAI-compatible HTTP backend（真实 `/chat/completions` 适配）

### IM Channels / Ingress Modes
- Telegram → `POST /telegram` 或 `getUpdates` long polling
- Discord → `POST /discord` 或 Gateway websocket
- Feishu / Lark → `POST /feishu`（支持 URL verification challenge）
- QQ → `POST /qq`
- Slack → `POST /slack`（支持 URL verification challenge）
- DingTalk (钉钉) → `POST /dingtalk`
- WeChat Work (企业微信) → `POST /wechatwork`
- LINE → `POST /line`

### Review / Ops Endpoints
- Health → `GET /healthz`
- Review snapshot → `GET /reviewz`
  - 包含平台 agent 绑定、routing rules、安全/持久化开关
  - 若使用 ACP backend，还会列出当前已建立的 ACP session 映射（本地 session、远端 session、backend、agent、stop reason）

## 关键特性

- 多平台 webhook 接入
- 默认 agent + 按平台 agent override + 按用户规则路由
- 统一 `Agent` / `Channel` trait
- 自动 session 创建、复用、可选持久化（`--state-file`）、可选历史裁剪（`--max-session-messages`）、独立的 agent 上下文窗口（`--context-message-limit`）和 agent 调用超时（`--agent-timeout-ms`）
- 会话级 `reply_target` 管理
  - Telegram / Feishu 用用户标识回发
  - Discord / QQ 用 channel 标识回发
- `DashMap` 驱动的并发 session 管理
- 可执行的 review / eval 回归测试

## 快速开始

生产模式现在要求使用真实 agent backend：内置 `claude` / `codex` / `pi` 仅保留给开发和 dry-run，不允许在真实 bot-server 进程里启动。

如果你的目标只是“本机跑一个 Telegram bridge bot”，现在最简单的是直接启用 long polling，不需要公网 webhook。

### 0. 本机 Telegram Long Polling

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

这个模式会在启动时自动关闭 Telegram webhook，然后用 `getUpdates` 长轮询拉消息，适合本机、内网、没有公网入口的场景。

### 1. 直接运行

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

也可以同时启用多个平台，并给不同平台绑定不同 agent：

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
  --discord-token "$DISCORD_TOKEN" \
  --feishu-app-id "$FEISHU_APP_ID" \
  --feishu-app-secret "$FEISHU_APP_SECRET" \
  --qq-bot-id "$QQ_BOT_ID" \
  --qq-bot-token "$QQ_BOT_TOKEN"
```

如果你想把默认 agent 直接接到真实 OpenAI-compatible backend：

```bash
cargo run -- \
  --agent openai \
  --openai-api-key "$OPENAI_API_KEY" \
  --openai-base-url "${OPENAI_BASE_URL:-https://api.openai.com/v1}" \
  --openai-model "${OPENAI_MODEL:-gpt-4o-mini}" \
  --openai-max-retries 1 \
  --telegram-token "$TELEGRAM_TOKEN"
```

### 2. 使用 `start.sh`

`start.sh` 是当前推荐的启动包装脚本，读取环境变量后拼出真实命令。
你也可以提供 `AGENTIM_CONFIG_FILE=agentim.json`，让 JSON 配置作为默认值，再由命令行 / 环境变量覆盖。

```bash
export AGENTIM_CONFIG_FILE=agentim.json
export AGENTIM_AGENT=openai
export AGENTIM_ADDR=127.0.0.1:8080
export AGENTIM_TELEGRAM_POLL=1
export AGENTIM_DISCORD_GATEWAY=1
export TELEGRAM_TOKEN=your-token
export DISCORD_TOKEN=your-discord-token
export OPENAI_API_KEY=your-api-key
./start.sh

# 可选
# export OPENAI_BASE_URL=https://api.openai.com/v1
# export OPENAI_MODEL=gpt-4o-mini
# export OPENAI_MAX_RETRIES=1
```

如果你只是在本地主机上跑 bot bridge，启用 `AGENTIM_TELEGRAM_POLL=1` 或 `AGENTIM_DISCORD_GATEWAY=1` 就不需要先把 Telegram / Discord 暴露成公网 webhook。

如果希望 session 在重启后恢复，并分别控制“保存多少历史”“每次送进 agent 多少上下文”以及“agent 最多跑多久”，可以再加：

```bash
export AGENTIM_STATE_FILE=.agentim/sessions.json
export AGENTIM_STATE_BACKUP_COUNT=2
export AGENTIM_MAX_SESSION_MESSAGES=50
export AGENTIM_CONTEXT_MESSAGE_LIMIT=12
export AGENTIM_AGENT_TIMEOUT_MS=30000
```

当主状态文件损坏时，启动时会自动尝试从最近的 `.bak.N` 快照恢复。

如果希望所有受保护路由都需要共享密钥：

```bash
export AGENTIM_WEBHOOK_SECRET=change-me
# 请求时带上 x-agentim-secret: change-me
```

如果希望 webhook 使用带时间戳/nonce 的 HMAC 签名校验：

```bash
export AGENTIM_WEBHOOK_SIGNING_SECRET=change-me-signing
export AGENTIM_WEBHOOK_MAX_SKEW_SECONDS=300
# 请求头:
#   x-agentim-timestamp
#   x-agentim-nonce
#   x-agentim-signature=sha256(<hmac>)
```

如果你在 Telegram 上想启用原生 secret token 校验：

```bash
export TELEGRAM_WEBHOOK_SECRET_TOKEN=tg-native-secret
# Telegram 会发送 x-telegram-bot-api-secret-token
```

如果你在 Discord 上想启用原生 interaction 签名校验：

```bash
export DISCORD_INTERACTION_PUBLIC_KEY=discord-public-key-hex
# /discord 会校验 x-signature-ed25519 + x-signature-timestamp
```

如果你在 Feishu 上想启用平台自带的 payload token 校验：

```bash
export FEISHU_WEBHOOK_VERIFICATION_TOKEN=feishu-native-token
# /feishu 会校验请求体里的 token 字段
```

对外暴露生产 webhook 时，至少要配置一层鉴权：

- 全局 `AGENTIM_WEBHOOK_SECRET` 或 `AGENTIM_WEBHOOK_SIGNING_SECRET`
- Telegram: 也可额外配置 `TELEGRAM_WEBHOOK_SECRET_TOKEN`
- Discord: 也可额外配置 `DISCORD_INTERACTION_PUBLIC_KEY`
- Feishu: 也可额外配置 `FEISHU_WEBHOOK_VERIFICATION_TOKEN`
- Slack: 也可额外配置 `SLACK_SIGNING_SECRET`
- QQ / DingTalk: 依赖全局共享密钥或全局签名校验

先做 dry-run 看启动配置是否正确：

```bash
AGENTIM_DRY_RUN=1 ./start.sh
# 或直接
cargo run -- --dry-run --agent claude --telegram-agent pi
```

现在 dry-run 会跳过真实 channel 健康检查，因此可以离线验证多平台启动参数与凭证格式。

### 3. 凭证参数

```text
--telegram-token
--discord-token
--feishu-app-id --feishu-app-secret
--qq-bot-id --qq-bot-token
--slack-token
--slack-signing-secret
--dingtalk-token
--dingtalk-secret
--wechatwork-corp-id --wechatwork-agent-id --wechatwork-secret
--line-token
--line-secret
```

兼容旧格式：
- `--feishu-token app_id:app_secret`
- `--qq-token bot_id:bot_token`

额外的 provider-native webhook 校验参数：
- `--telegram-webhook-secret-token`
- `--discord-interaction-public-key`
- `--feishu-verification-token`
- `--slack-signing-secret`
- `--dingtalk-secret`
- `--line-secret`

### 4. Docker 部署

```bash
# 构建镜像
docker build -t agentim .

# 使用环境变量运行
docker run -d \
  -p 8080:8080 \
  -v $(pwd)/config:/app/config:ro \
  -v $(pwd)/state:/app/state \
  -e OPENAI_API_KEY=your-api-key \
  -e TELEGRAM_TOKEN=your-token \
  agentim

# 或使用 docker-compose
docker-compose up -d
```

### 5. 从 Release 安装

也可以直接从发布页下载对应平台的压缩包，而不是本地编译：

- Release 页面：`https://github.com/markbang/agentim/releases`
- Linux / macOS: `*.tar.gz` + 对应的 `*.sha256`
- Windows: `*.zip` + 对应的 `*.sha256`

Linux 示例：

```bash
curl -LO https://github.com/markbang/agentim/releases/download/v0.2.0/agentim-linux-x86_64.tar.gz
curl -LO https://github.com/markbang/agentim/releases/download/v0.2.0/agentim-linux-x86_64.tar.gz.sha256
shasum -a 256 -c agentim-linux-x86_64.tar.gz.sha256
tar -xzf agentim-linux-x86_64.tar.gz
./agentim-linux-x86_64/agentim --help
```

macOS 示例：

```bash
curl -LO https://github.com/markbang/agentim/releases/download/v0.2.0/agentim-macos-aarch64.tar.gz
curl -LO https://github.com/markbang/agentim/releases/download/v0.2.0/agentim-macos-aarch64.tar.gz.sha256
shasum -a 256 -c agentim-macos-aarch64.tar.gz.sha256
tar -xzf agentim-macos-aarch64.tar.gz
./agentim-macos-aarch64/agentim --help
```

Windows PowerShell 示例：

```powershell
curl.exe -LO https://github.com/markbang/agentim/releases/download/v0.2.0/agentim-windows-x86_64.zip
curl.exe -LO https://github.com/markbang/agentim/releases/download/v0.2.0/agentim-windows-x86_64.zip.sha256
$expected = (Get-Content .\agentim-windows-x86_64.zip.sha256).Split()[0]
$actual = (Get-FileHash .\agentim-windows-x86_64.zip -Algorithm SHA256).Hash.ToLower()
if ($expected -ne $actual) { throw "SHA256 mismatch" }
Expand-Archive .\agentim-windows-x86_64.zip -DestinationPath .
.\agentim-windows-x86_64\agentim.exe --help
```

### 6. 配置文件

使用 JSON 配置文件可以管理更复杂的设置：

```json
{
  "agent": "openai",
  "openai_api_key": "your-api-key",
  "openai_base_url": "https://api.openai.com/v1",
  "openai_model": "gpt-4o-mini",
  "telegram_token": "your-telegram-token",
  "telegram_poll": true,
  "discord_token": "your-discord-token",
  "discord_gateway": true,
  "slack_token": "xoxb-your-slack-token",
  "dingtalk_token": "your-dingtalk-token",
  "state_file": "/app/state/sessions.json",
  "context_message_limit": 20,
  "agent_timeout_ms": 30000
}
```

## 消息桥接流程

```text
Incoming webhook
  -> parse platform payload
  -> find_or_create_session(agent, channel, user)
  -> store reply_target in session metadata
  -> send context to agent
  -> send agent response back through the correct channel target
```

这意味着 Discord / QQ 这类“用户 ID 和回复 channel ID 不同”的平台，也能走统一桥接路径。
另外，运行时 JSON 配置里的 `routing_rules` 可以把特定平台上的特定用户、特定 `reply_target`（如 Discord / QQ 频道），或者一组带相同前缀的 `reply_target` 定向到不同 agent。

## Review / Eval

用户要求这个 bridge 在迭代过程中持续被 review 和 eval，所以仓库内置了两层检查：

### 1. Reviewer tests

```bash
cargo test --test review_bridge
```

覆盖点：
- 四个平台 webhook 都可进入统一路由
- `reply_target` 对 Discord / QQ 生效
- 同一用户+平台复用 session
- 平台默认 agent 可被用户级 routing rule 覆盖

### 2. Autoresearch acceptance loop

```bash
./autoresearch.sh
```

它会输出结构化 `METRIC ...` 行，用于跟踪 bridge 的可用性、路由覆盖和 review 覆盖，而不是只看“代码是不是存在”。

## 最小库示例

```rust
use agentim::{AgentIM, agent::ClaudeAgent, channel::TelegramChannel};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let agentim = AgentIM::new();

    agentim.register_agent(
        "claude-main".to_string(),
        Arc::new(ClaudeAgent::new("claude-main".to_string(), None)),
    )?;

    agentim.register_channel(
        "telegram-main".to_string(),
        Arc::new(TelegramChannel::new("telegram-main".to_string())),
    )?;

    let session_id = agentim.create_session(
        "claude-main".to_string(),
        "telegram-main".to_string(),
        "user123".to_string(),
    )?;

    let response = agentim
        .send_to_agent(&session_id, "Hello!".to_string())
        .await?;
    agentim.send_to_channel(&session_id, response).await?;

    Ok(())
}
```

## 测试

```bash
cargo test
cargo test --test review_bridge
cargo run --example basic
cargo run --example session_management
```

## 当前边界

- 内置 agent 仍是本地模拟实现
- 当前二进制是“单默认 agent”模式，不是完整的多 agent 动态路由器
- session 持久化还没有接到真正的存储后端
- 生产默认值包含 `agent_timeout_ms=30000`、请求体大小限制（256 KiB）、后台异步 session 快照和按 session 串行化的消息处理
- 真实生产流量请使用 `openai` 或 `acp`；`claude` / `codex` / `pi` 仍然只是开发 stub

## 相关文档

- `QUICK_START.md` — 最快启动方式
- `SETUP.md` — 环境变量和部署方式
- `BOT_INTEGRATION.md` — 各平台 webhook / 凭证说明
- `ARCHITECTURE.md` — 模块结构
