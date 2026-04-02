# AgentIM Quick Reference

## 当前定位

AgentIM 是一个用 Rust 编写的多平台 AI bridge：把 Telegram、Discord、Feishu、QQ 的 webhook 消息统一接入到 agent 层，也支持 Telegram long polling 和 Discord Gateway，本地就能直接跑 bot bridge，并负责 session、路由、上下文裁剪、回发目标与可选持久化。

当前仓库提供两层能力：

- **库能力**：`AgentIM`、`Agent`、`Channel`、`Session` 抽象，可用于二次开发
- **二进制能力**：`agentim` 启动 webhook server，自动处理桥接流程

## 最常用命令

```bash
# 一键安装最新 release
curl -fsSL https://raw.githubusercontent.com/markbang/agentim/main/install.sh | bash

# 查看参数
cargo run -- --help

# 本地构建
cargo build --release

# Dry-run 校验启动参数
cargo run -- --dry-run --agent claude --telegram-token "$TELEGRAM_TOKEN"
AGENTIM_DRY_RUN=1 ./start.sh
AGENTIM_DRY_RUN=1 ./setup.sh

# 运行主服务
cargo run -- \
  --agent openai \
  --openai-api-key "$OPENAI_API_KEY" \
  --telegram-token "$TELEGRAM_TOKEN" \
  --telegram-poll \
  --discord-token "$DISCORD_TOKEN" \
  --discord-gateway \
  --addr 127.0.0.1:8080

# 回归测试
cargo test --test review_bridge

# 结构化验收
./autoresearch.sh
```

## 启动脚本

- `start.sh`：推荐入口，读取环境变量并自动构建过期的 release 二进制
- `setup.sh`：兼容包装器，会加载 `.env`、兼容旧环境变量名，并委托到 `start.sh`

## 关键入口

- Telegram: `POST /telegram` 或 `--telegram-poll`
- Discord: `POST /discord` 或 `--discord-gateway`
- `POST /feishu`
- `POST /qq`
- `GET /healthz`
- `GET /reviewz`

## 关键参数

- Agent 选择：`--agent`、`--telegram-agent`、`--discord-agent`、`--feishu-agent`、`--qq-agent`
- 本地接入：`--telegram-poll`、`--discord-gateway`
- OpenAI backend：`--openai-api-key`、`--openai-base-url`、`--openai-model`、`--openai-max-retries`
- Session 控制：`--state-file`、`--state-backup-count`、`--max-session-messages`、`--context-message-limit`、`--agent-timeout-ms`
- 安全控制：`--webhook-secret`、`--webhook-signing-secret`、`--webhook-max-skew-seconds`
- 平台校验：`--telegram-webhook-secret-token`、`--discord-interaction-public-key`、`--feishu-verification-token`

## 项目结构

```text
src/
├── main.rs           # 启动入口，合并 CLI + JSON 配置并注册 agent/channel
├── bot_server.rs     # Axum 路由、鉴权、路由决策、health/review 端点
├── manager.rs        # AgentIM 核心管理器
├── session.rs        # 会话、历史裁剪、history summary
├── agent.rs          # Agent trait 与内置 agent 实现
├── channel.rs        # Channel trait 与通用 channel 抽象
├── bots/             # 各平台 webhook 解析与回发实现
├── cli.rs            # 当前扁平 flag CLI 定义
├── error.rs          # 统一错误类型
└── config.rs         # 公共配置类型

tests/
└── review_bridge.rs  # 关键桥接回归测试
```

## 技术栈

- Rust 2021
- Tokio
- Axum
- Reqwest
- Serde / Serde JSON
- DashMap
- Clap
- HMAC-SHA256 / Ed25519

## 当前维护重点

1. **保持文档与运行模型一致**：避免旧版子命令文档继续误导使用者
2. **守住桥接回归测试**：优先维护 `tests/review_bridge.rs` 覆盖的关键路径
3. **持续收敛会话/路由可观测性**：围绕 `reviewz`、持久化、签名校验，以及 ACP session 映射增加可见性
