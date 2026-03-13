# AgentIM - Multi-Channel AI Agent Manager

一个用Rust编写的优雅的多channel AI agent管理系统。AgentIM提供了一个统一的框架来管理多个AI代理（Claude、Codex、Pi）和多个通讯渠道（Telegram、Discord、Feishu、QQ），同时优雅地管理用户会话和上下文。

## 核心设计理念

AgentIM是一个**纯粹的会话和上下文管理框架**，而不是一个API网关。它的职责是：

- ✅ 管理Agent和Channel的注册
- ✅ 维护用户会话和消息历史
- ✅ 提供优雅的上下文管理
- ✅ 支持并发会话处理

它**不负责**：
- ❌ 调用外部API（由使用者负责）
- ❌ 存储持久化（可扩展）
- ❌ 消息路由（由使用者实现）

## 架构设计

```
┌─────────────────────────────────────────────────────────┐
│                      AgentIM Core                        │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │
│  │   Agents     │  │  Channels    │  │  Sessions    │   │
│  ├──────────────┤  ├──────────────┤  ├──────────────┤   │
│  │ • Claude     │  │ • Telegram   │  │ • Context    │   │
│  │ • Codex      │  │ • Discord    │  │ • History    │   │
│  │ • Pi         │  │ • Feishu     │  │ • Metadata   │   │
│  │              │  │ • QQ         │  │              │   │
│  └──────────────┘  └──────────────┘  └──────────────┘   │
│                                                           │
│  ┌──────────────────────────────────────────────────┐   │
│  │         Session Manager (DashMap)                │   │
│  │  - 并发安全的session存储                          │   │
│  │  - 自动上下文管理                                │   │
│  │  - 消息历史追踪                                  │   │
│  └──────────────────────────────────────────────────┘   │
│                                                           │
└─────────────────────────────────────────────────────────┘
         │                          │
         ▼                          ▼
    ┌─────────────┐          ┌──────────────┐
    │   Your      │          │   Your       │
    │   Agent     │          │   Channel    │
    │   Logic     │          │   Logic      │
    └─────────────┘          └──────────────┘
```

## 核心特性

### 1. 多Agent支持
- **Claude**: 本地模拟实现（可扩展为真实API调用）
- **Codex**: 本地模拟实现（可扩展为真实API调用）
- **Pi**: 本地模拟实现（可扩展为真实API调用）
- 易于添加新的Agent类型

### 2. 多Channel支持
- **Telegram**: 本地模拟实现
- **Discord**: 本地模拟实现
- **Feishu/Lark**: 本地模拟实现
- **QQ**: 本地模拟实现
- 易于添加新的Channel类型

### 3. 优雅的Session管理
- 每个用户-agent-channel组合维护独立session
- 自动上下文管理（可配置历史消息数量）
- 消息持久化支持（可扩展）
- 元数据支持

### 4. 并发安全
- 使用`DashMap`实现无锁并发
- 支持高并发场景
- 线程安全的Agent和Channel trait

### 5. 易于扩展
- Trait-based设计
- 易于添加新的Agent和Channel类型
- 易于集成真实API

## 快速开始

### 方式1：交互式设置（推荐）

```bash
cargo build --release
./target/release/agentim interactive
```

这将引导你完成：
1. 注册AI agents（Claude、Codex、Pi）
2. 注册通讯渠道（Telegram、Discord、Feishu、QQ）
3. 创建agent和channel之间的会话
4. 测试消息流

### 方式2：配置文件

```bash
cp agentim.json.example agentim.json
# 编辑 agentim.json 配置你的agents和channels
./start.sh
```

详见 [SETUP.md](SETUP.md) 获取完整指南。

## 核心特性

```rust
use agentim::{AgentIM, agent::ClaudeAgent, channel::TelegramChannel};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let agentim = AgentIM::new();

    // 注册Agent
    let claude = Arc::new(ClaudeAgent::new("claude-1".to_string(), None));
    agentim.register_agent("claude-1".to_string(), claude)?;

    // 注册Channel
    let telegram = Arc::new(TelegramChannel::new("tg-1".to_string()));
    agentim.register_channel("tg-1".to_string(), telegram)?;

    // 创建Session
    let session_id = agentim.create_session(
        "claude-1".to_string(),
        "tg-1".to_string(),
        "user123".to_string(),
    )?;

    // 发送消息到Agent
    let response = agentim.send_to_agent(&session_id, "Hello!".to_string()).await?;

    // 发送响应到Channel
    agentim.send_to_channel(&session_id, response).await?;

    Ok(())
}
```

### CLI使用

```bash
# 注册Agent
./target/release/agentim agent register --id claude-1 --agent-type claude

# 注册Channel
./target/release/agentim channel register --id tg-1 --channel-type telegram

# 创建Session
./target/release/agentim session create \
  --agent-id claude-1 \
  --channel-id tg-1 \
  --user-id user123

# 查看系统状态
./target/release/agentim status
```

## API文档

### AgentIM

核心管理器，负责Agent、Channel和Session的生命周期管理。

```rust
// Agent管理
pub fn register_agent(&self, id: String, agent: Arc<dyn Agent>) -> Result<()>
pub fn get_agent(&self, id: &str) -> Result<Arc<dyn Agent>>
pub fn list_agents(&self) -> Vec<String>

// Channel管理
pub fn register_channel(&self, id: String, channel: Arc<dyn Channel>) -> Result<()>
pub fn get_channel(&self, id: &str) -> Result<Arc<dyn Channel>>
pub fn list_channels(&self) -> Vec<String>

// Session管理
pub fn create_session(&self, agent_id: String, channel_id: String, user_id: String) -> Result<String>
pub fn get_session(&self, id: &str) -> Result<Session>
pub fn update_session(&self, id: &str, session: Session) -> Result<()>
pub fn list_sessions(&self) -> Vec<Session>
pub fn delete_session(&self, id: &str) -> Result<()>

// 消息流
pub async fn send_to_agent(&self, session_id: &str, user_message: String) -> Result<String>
pub async fn send_to_channel(&self, session_id: &str, message: String) -> Result<()>

// 健康检查
pub async fn health_check(&self) -> Result<()>
```

### Session

维护用户与Agent的对话上下文。

```rust
pub struct Session {
    pub id: String,
    pub agent_id: String,
    pub channel_id: String,
    pub user_id: String,
    pub messages: VecDeque<Message>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

impl Session {
    pub fn new(agent_id: String, channel_id: String, user_id: String) -> Self
    pub fn add_message(&mut self, role: MessageRole, content: String)
    pub fn get_context(&self, max_messages: usize) -> Vec<Message>
    pub fn clear_history(&mut self)
}
```

## 扩展指南

### 添加真实的Agent实现

```rust
use async_trait::async_trait;
use agentim::agent::Agent;
use agentim::config::AgentType;
use agentim::session::Message;
use agentim::error::Result;

pub struct MyRealAgent {
    id: String,
    api_key: String,
}

#[async_trait]
impl Agent for MyRealAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Claude
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, messages: Vec<Message>) -> Result<String> {
        // 调用真实API
        let client = reqwest::Client::new();
        // ... 实现API调用
        Ok("response".to_string())
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}
```

### 添加真实的Channel实现

```rust
use async_trait::async_trait;
use agentim::channel::{Channel, ChannelMessage};
use agentim::config::ChannelType;
use agentim::error::Result;

pub struct MyRealChannel {
    id: String,
    credentials: String,
}

#[async_trait]
impl Channel for MyRealChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Telegram
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        // 调用真实API
        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}
```

## 性能特性

- **并发安全**: 使用DashMap实现无锁并发，支持高并发场景
- **内存高效**: VecDeque用于消息历史，支持自动清理
- **异步优先**: 所有I/O操作都是异步的，基于Tokio
- **可扩展**: 易于添加新的Agent和Channel类型

## 测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test session::tests

# 运行示例
cargo run --example basic
cargo run --example session_management
```

## 许可证

MIT

## 贡献

欢迎提交Issue和Pull Request！
