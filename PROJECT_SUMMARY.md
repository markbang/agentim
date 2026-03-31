# AgentIM 项目总结

## 项目概述

AgentIM 是一个用 Rust 编写的多平台 AI bridge。它把来自 Telegram、Discord、Feishu/Lark、QQ 的 webhook 消息统一送入 agent 层，并在服务端管理：

- 默认 agent 与平台级 agent 覆盖
- 基于 `routing_rules` 的用户级 / reply-target 级路由
- session 自动创建与复用
- 历史裁剪与上下文窗口控制
- agent 超时
- 可选 JSON 持久化与备份恢复
- webhook 共享密钥、HMAC 签名与平台原生验签

## 当前运行模型

当前二进制 `agentim` 是一个**启动型 bridge 进程**，而不是带大量资源管理子命令的 CLI。典型启动流程是：

1. 读取命令行与 JSON 配置
2. 注册默认 agent 与平台级 agent
3. 注册已配置凭证的平台 channel
4. 创建 Axum router
5. 暴露 `/telegram`、`/discord`、`/feishu`、`/qq`、`/healthz`、`/reviewz`
6. 处理 webhook → session → agent → channel 回发
7. 通过 `/reviewz` 暴露运行态摘要；ACP backend 启用时额外展示 ACP session 映射与 stop reason

## 技术栈

- **Rust 2021**
- **Tokio**：异步运行时
- **Axum**：HTTP / webhook server
- **Reqwest**：调用 OpenAI-compatible backend 与平台 API
- **Serde / serde_json**：配置与状态序列化
- **DashMap**：并发 session / registry 管理
- **Clap**：参数解析
- **HMAC-SHA256 / Ed25519**：webhook 验签

## 核心模块

```text
src/
├── main.rs           # 启动入口，负责合并配置与注册组件
├── bot_server.rs     # webhook 路由、鉴权、review/health 端点
├── manager.rs        # AgentIM 管理器：agent/channel/session 编排
├── session.rs        # session、上下文窗口、trim/history summary
├── agent.rs          # Agent trait 与内置 agent 实现
├── channel.rs        # Channel trait 与通用 channel 实现
├── bots/             # Telegram/Discord/Feishu/QQ 平台适配
├── cli.rs            # 当前 flag CLI 定义
├── error.rs          # 统一错误类型
└── config.rs         # 公共配置结构
```

## 本地开发与验证

### 构建

```bash
cargo build --release
```

### 查看可用参数

```bash
cargo run -- --help
```

### Dry-run

```bash
cargo run -- --dry-run --agent claude --telegram-token "$TELEGRAM_TOKEN"
AGENTIM_DRY_RUN=1 ./start.sh
```

### 运行核心回归测试

```bash
cargo test --test review_bridge
```

### 运行 acceptance 检查

```bash
./autoresearch.sh
```

## 推荐启动方式

### 方式 1：直接启动

```bash
cargo run -- \
  --agent claude \
  --telegram-token "$TELEGRAM_TOKEN" \
  --addr 127.0.0.1:8080
```

### 方式 2：环境变量 + `start.sh`

```bash
export AGENTIM_CONFIG_FILE=agentim.json
export AGENTIM_AGENT=claude
export AGENTIM_ADDR=127.0.0.1:8080
export TELEGRAM_TOKEN=your-token
./start.sh
```

### 方式 3：兼容包装器 `setup.sh`

```bash
AGENTIM_DRY_RUN=1 ./setup.sh
./setup.sh
```

`setup.sh` 会加载 `.env`、兼容旧变量名，并转调 `start.sh`。

## 项目现状

### 已具备能力

- 多平台 webhook 接入
- 默认 agent + 平台覆盖 + `routing_rules`
- `reply_target` 正确回发 Discord / QQ 等频道型平台
- session 持久化、备份轮转与损坏恢复
- 上下文窗口限制、历史裁剪、超时控制
- webhook 共享密钥、HMAC 签名、Telegram/Discord/Feishu 原生校验
- `review_bridge` 回归测试与 `autoresearch.sh` 验收脚本

### 当前维护风险

- 仓库内仍有部分历史文档描述旧版“子命令式 CLI”模型
- 依赖版本若继续漂移，可能再次踩到本地工具链兼容性问题
- session 与路由行为已较丰富，后续改动应优先依赖回归测试而非人工验证

## 维护建议

1. **优先维护 `README.md`、`SETUP.md`、`CLI_GUIDE.md` 的一致性**
2. **每次改路由/会话逻辑先跑 `cargo test --test review_bridge`**
3. **新增平台或 agent 时同步补充 `reviewz` 可观测信息与测试覆盖**
