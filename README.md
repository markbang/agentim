# AgentIM - Multi-Channel AI Agent Manager

一个用Rust编写的优雅的多channel AI agent管理系统，支持Claude、Codex、Pi等多个AI平台，以及Telegram、Discord、Feishu、QQ等多个通讯渠道。

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
```

## 核心特性

### 1. 多Agent支持
- **Claude**: 通过Anthropic API集成
- **Codex**: 通过OpenAI API集成
- **Pi**: 通过Pi API集成
- 易于扩展新的Agent类型

### 2. 多Channel支持
- **Telegram**: 通过Bot API
- **Discord**: 通过Discord API
- **Feishu/Lark**: 通过Feishu Open API
- **QQ**: 通过QQ Bot API
- 易于扩展新的Channel类型

### 3. 优雅的Session管理
- 每个用户-agent-channel组合维护独立session
- 自动上下文管理（可配置历史消息数量）
- 消息持久化和恢复
- 元数据支持

### 4. 并发安全
- 使用`DashMap`实现无锁并发
- 支持高并发场景
- 线程安全的Agent和Channel trait

### 5. 错误处理
- 统一的错误类型
- 详细的错误信息
- 优雅的错误传播

## 项目结构

```
src/
├── main.rs           # 主程序入口
├── lib.rs            # 库导出
├── agent.rs          # Agent trait和实现
├── channel.rs        # Channel trait和实现
├── session.rs        # Session和Message定义
├── manager.rs        # AgentIM核心管理器
├── config.rs         # 配置类型定义
└── error.rs          # 错误类型定义

examples/
├── basic.rs          # 基础使用示例
└── session_management.rs  # Session管理示例
```

## 快速开始

### 安装依赖

```bash
cargo build
```

### 基础使用

```rust
use agentim::{AgentIM, agent::ClaudeAgent, channel::TelegramChannel};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let agentim = AgentIM::new();

    // 注册Agent
    let claude = Arc::new(ClaudeAgent::new(
        "claude-1".to_string(),
        "your-api-key".to_string(),
        None,
        None,
    ));
    agentim.register_agent("claude-1".to_string(), claude)?;

    // 注册Channel
    let telegram = Arc::new(TelegramChannel::new(
        "tg-1".to_string(),
        "your-bot-token".to_string(),
    ));
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

### 运行示例

```bash
# 基础示例
cargo run --example basic

# Session管理示例
cargo run --example session_management
```

## API文档

### AgentIM

核心管理器，负责Agent、Channel和Session的生命周期管理。

#### 主要方法

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

### 添加新的Agent

```rust
use async_trait::async_trait;
use agentim::agent::Agent;
use agentim::config::AgentType;

pub struct MyAgent {
    id: String,
    api_key: String,
}

#[async_trait]
impl Agent for MyAgent {
    fn agent_type(&self) -> AgentType {
        // 需要在config.rs中添加新的AgentType
        AgentType::Claude
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, messages: Vec<Message>) -> Result<String> {
        // 实现消息发送逻辑
        Ok("response".to_string())
    }

    async fn health_check(&self) -> Result<()> {
        // 实现健康检查
        Ok(())
    }
}
```

### 添加新的Channel

```rust
use async_trait::async_trait;
use agentim::channel::{Channel, ChannelMessage};
use agentim::config::ChannelType;

pub struct MyChannel {
    id: String,
    credentials: String,
}

#[async_trait]
impl Channel for MyChannel {
    fn channel_type(&self) -> ChannelType {
        // 需要在config.rs中添加新的ChannelType
        ChannelType::Telegram
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        // 实现消息发送逻辑
        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        // 实现消息接收逻辑
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        // 实现健康检查
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
