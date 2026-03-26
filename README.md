# AgentIM

一个用 Rust 编写的 IM bridge，用来把多个 AI agent 接到多个 IM 平台上，并统一管理 session、上下文和回复目标。

当前仓库有两层能力：

1. **库能力**：`AgentIM` 提供 agent/channel/session 抽象，适合二次开发。
2. **可运行二进制**：`agentim` 启动一个 webhook server，把收到的 IM 消息转给一个默认 agent，再把回复发回对应平台。

> 当前二进制会注册一个默认 agent，并支持两级路由：
> 1. 按平台覆盖：`--telegram-agent`、`--discord-agent`、`--feishu-agent`、`--qq-agent`
> 2. 按配置规则覆盖：`routing_rules`（可按平台 + 用户精确匹配）
>
> 更复杂的 workspace / 组织级策略路由，仍然更适合直接用库层扩展。

## 当前支持

### Agents
- Claude（本地模拟）
- Codex（本地模拟）
- Pi（本地模拟）

### IM Channels / Webhook Routes
- Telegram → `POST /telegram`
- Discord → `POST /discord`
- Feishu / Lark → `POST /feishu`
- QQ → `POST /qq`

### Review / Ops Endpoints
- Health → `GET /healthz`
- Review snapshot → `GET /reviewz`

## 关键特性

- 多平台 webhook 接入
- 默认 agent + 按平台 agent override + 按用户规则路由
- 统一 `Agent` / `Channel` trait
- 自动 session 创建、复用、可选持久化（`--state-file`）与可选历史裁剪（`--max-session-messages`）
- 会话级 `reply_target` 管理
  - Telegram / Feishu 用用户标识回发
  - Discord / QQ 用 channel 标识回发
- `DashMap` 驱动的并发 session 管理
- 可执行的 review / eval 回归测试

## 快速开始

### 1. 直接运行

```bash
cargo run -- \
  --agent claude \
  --telegram-token "$TELEGRAM_TOKEN" \
  --addr 127.0.0.1:8080
```

也可以同时启用多个平台，并给不同平台绑定不同 agent：

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

### 2. 使用 `start.sh`

`start.sh` 是当前推荐的启动包装脚本，读取环境变量后拼出真实命令。
你也可以提供 `AGENTIM_CONFIG_FILE=agentim.json`，让 JSON 配置作为默认值，再由命令行 / 环境变量覆盖。

```bash
export AGENTIM_CONFIG_FILE=agentim.json
export AGENTIM_AGENT=claude
export AGENTIM_ADDR=127.0.0.1:8080
export TELEGRAM_TOKEN=your-token
./start.sh
```

如果希望 session 在重启后恢复，并控制历史长度，可以再加：

```bash
export AGENTIM_STATE_FILE=.agentim/sessions.json
export AGENTIM_MAX_SESSION_MESSAGES=50
```

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

先做 dry-run 看启动配置是否正确：

```bash
AGENTIM_DRY_RUN=1 ./start.sh
# 或直接
cargo run -- --dry-run --agent claude --telegram-agent pi
```

### 3. 凭证参数

```text
--telegram-token
--discord-token
--feishu-app-id --feishu-app-secret
--qq-bot-id --qq-bot-token
```

兼容旧格式：
- `--feishu-token app_id:app_secret`
- `--qq-token bot_id:bot_token`

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
- webhook 签名校验 / 更完整的生产安全配置还没做完

## 相关文档

- `QUICK_START.md` — 最快启动方式
- `SETUP.md` — 环境变量和部署方式
- `BOT_INTEGRATION.md` — 各平台 webhook / 凭证说明
- `ARCHITECTURE.md` — 模块结构
