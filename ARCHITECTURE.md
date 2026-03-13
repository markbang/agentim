# AgentIM 架构设计文档

## 系统架构

```
┌─────────────────────────────────────────────────────────────┐
│                     CLI Interface                            │
│  (clap-based command parser with colored output)             │
└────────────────────┬────────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────────┐
│                   AgentIM Manager                            │
│  (Central orchestrator using DashMap for concurrency)        │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────────┐  ┌──────────────────┐                 │
│  │  Agent Registry  │  │ Channel Registry │                 │
│  │  (Arc<dyn>)      │  │  (Arc<dyn>)      │                 │
│  └──────────────────┘  └───────────���──────┘                 │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │         Session Manager (DashMap)                    │   │
│  │  - Session ID -> Session mapping                     │   │
│  │  - Concurrent access without locks                   │   │
│  │  - Automatic cleanup support                         │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                               │
└─────────────────────────────────────────────────────────────┘
         │                          │
         ▼                          ▼
    ┌─────────────┐          ┌──────────────┐
    │   Agents    │          │   Channels   │
    ├─────────────┤          ├──────────────┤
    │ • Claude    │          │ • Telegram   │
    │ • Codex     │          │ • Discord    │
    │ • Pi        │          │ • Feishu     │
    │             │          │ • QQ         │
    └─────────────┘          └──────────────┘
         │                          │
         ▼                          ▼
    ┌─────────────┐          ┌──────────────┐
    │ External    │          │ External     │
    │ APIs        │          │ APIs         │
    └─────────────┘          └──────────────┘
```

## 核心模块

### 1. Agent Module (`agent.rs`)

**职责**: 定义Agent trait和具体实现

**关键组件**:
- `Agent` trait: 异步trait，定义Agent接口
  - `send_message()`: 发送消息到Agent
  - `health_check()`: 检查Agent健康状态
  - `agent_type()`: 返回Agent类型
  - `id()`: 返回Agent ID

- `ClaudeAgent`: Anthropic Claude实现
  - 使用Anthropic API
  - 支持自定义模型和base_url
  - 流式响应支持

- `CodexAgent`: OpenAI Codex实现
  - 使用OpenAI API
  - 支持自定义模型

- `PiAgent`: Pi AI实现
  - 使用Pi API
  - 基础实现

**设计模式**:
- Trait object: `Arc<dyn Agent>` 用于多态
- 异步优先: 所有I/O操作都是异步的

### 2. Channel Module (`channel.rs`)

**职责**: 定义Channel trait和具体实现

**关键组件**:
- `Channel` trait: 异步trait，定义Channel接口
  - `send_message()`: 发送消息到Channel
  - `receive_message()`: 接收来自Channel的消息
  - `health_check()`: 检查Channel健康状态
  - `channel_type()`: 返回Channel类型
  - `id()`: 返回Channel ID

- `TelegramChannel`: Telegram Bot实现
- `DiscordChannel`: Discord Bot实现
- `FeishuChannel`: Feishu/Lark实现
- `QQChannel`: QQ Bot实现

**设计模式**:
- Trait object: `Arc<dyn Channel>` 用于多态
- 异步优先: 所有I/O操作都是异步的

### 3. Session Module (`session.rs`)

**职责**: 管理用户与Agent的对话上下文

**关键组件**:
- `Message`: 单条消息
  - `id`: 唯一标识
  - `role`: 消息角色 (User/Assistant/System)
  - `content`: 消息内容
  - `timestamp`: 时间戳

- `Session`: 对话会话
  - `id`: 唯一标识
  - `agent_id`: 关联的Agent
  - `channel_id`: 关联的Channel
  - `user_id`: 用户ID
  - `messages`: VecDeque<Message> 消息历史
  - `metadata`: 自定义元数据

**关键方法**:
- `add_message()`: 添加消息到历史
- `get_context()`: 获取最近N条消息作为上下文
- `clear_history()`: 清空消息历史

**设计模式**:
- VecDeque: 高效的消息队列
- 可配置的上下文窗口

### 4. Manager Module (`manager.rs`)

**职责**: 核心管理器，协调所有组件

**关键组件**:
- `AgentIM`: 主管理器
  - `agents`: DashMap<String, Arc<dyn Agent>>
  - `channels`: DashMap<String, Arc<dyn Channel>>
  - `sessions`: DashMap<String, Session>

**关键方法**:
- Agent管理: register, get, list
- Channel管理: register, get, list
- Session管理: create, get, update, delete, list
- 消息流: send_to_agent, send_to_channel
- 系统: health_check

**设计模式**:
- DashMap: 无锁并发哈希表
- Arc: 原子引用计数，支持多所有权
- Clone: 支持跨线程共享

### 5. Config Module (`config.rs`)

**职责**: 配置类型定义

**关键组件**:
- `AgentType`: 枚举 (Claude, Codex, Pi)
- `ChannelType`: 枚举 (Telegram, Discord, Feishu, QQ)
- `AgentConfig`: Agent配置
- `ChannelConfig`: Channel配置
- `AppConfig`: 应用全局配置

### 6. Error Module (`error.rs`)

**职责**: 统一的错误处理

**关键组件**:
- `AgentError`: 自定义错误类型
  - 使用`thiserror`宏
  - 详细的错误信息
  - 自动From实现

### 7. CLI Module (`cli.rs`)

**职责**: 命令行接口

**关键组件**:
- `Cli`: 主命令结构
- `Commands`: 顶级命令枚举
- `AgentAction`: Agent子命令
- `ChannelAction`: Channel子命令
- `SessionAction`: Session子命令
- 输出格式化函数

## 数据流

### 消息发送流程

```
User Input (CLI/API)
    │
    ▼
┌─────────────────────────────────────┐
│  AgentIM.send_to_agent()            │
│  1. 获取Session                      │
│  2. 添加用户消息到Session            │
│  3. 获取上下文 (最近N条消息)         │
│  4. 调用Agent.send_message()        │
│  5. 添加Agent响应到Session          │
│  6. 更新Session                     │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  Agent.send_message()               │
│  1. 格式化消息                       │
│  2. 调用外部API                      │
│  3. 解析响应                         │
│  4. 返回文本                         │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  AgentIM.send_to_channel()          │
│  1. 获取Session                      │
│  2. 获取Channel                      │
│  3. 调用Channel.send_message()      │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  Channel.send_message()             │
│  1. 格式化消息                       │
│  2. 调用外部API                      │
│  3. 返回结果                         │
└─────────────────────────────────────┘
    │
    ▼
User Output (Telegram/Discord/etc)
```

## 并发模型

### 无锁并发

使用DashMap实现无锁并发：

```rust
// 多个线程可以同时访问不同的Session
let agentim = Arc::new(AgentIM::new());

// 线程1
let agentim1 = agentim.clone();
tokio::spawn(async move {
    agentim1.send_to_agent("session1", "msg1").await
});

// 线程2
let agentim2 = agentim.clone();
tokio::spawn(async move {
    agentim2.send_to_agent("session2", "msg2").await
});
```

### 异步优先

所有I/O操作都是异步的：

```rust
// 异步Agent调用
pub async fn send_message(&self, messages: Vec<Message>) -> Result<String>

// 异步Channel调用
pub async fn send_message(&self, user_id: &str, content: &str) -> Result<()>

// 异步Manager方法
pub async fn send_to_agent(&self, session_id: &str, message: String) -> Result<String>
```

## 扩展性设计

### 添加新Agent类型

1. 在`config.rs`中添加新的`AgentType`
2. 在`agent.rs`中实现新的Agent struct
3. 实现`Agent` trait
4. 在CLI中添加支持

### 添加新Channel类型

1. 在`config.rs`中添加新的`ChannelType`
2. 在`channel.rs`中实现新的Channel struct
3. 实现`Channel` trait
4. 在CLI中添加支持

## 性能考虑

### 内存优化

- **VecDeque**: 高效的消息队列，支持O(1)的头尾操作
- **DashMap**: 分片哈希表，减少锁竞争
- **Arc**: 共享所有权，避免数据复制

### 并发优化

- **无锁设计**: DashMap避免全局锁
- **异步I/O**: Tokio支持高并发
- **消息批处理**: 支持批量操作

### 可扩展性

- **Trait-based**: 易于添加新的Agent和Channel
- **Plugin architecture**: 支持动态加载
- **配置驱动**: 支持运行时配置

## 安全考虑

### API Key管理

- 使用环境变量存储敏感信息
- 支持密钥轮转
- 日志中自动脱敏

### 权限控制

- Session隔离
- 用户级别的访问控制
- 审计日志

### 错误处理

- 详细的错误信息
- 优雅的错误传播
- 自动重试机制（可选）

## 测试策略

### 单元测试

- Session管理测试
- Agent trait测试
- Channel trait测试

### 集成测试

- 完整的消息流测试
- 多Agent多Channel测试
- 并发测试

### 性能测试

- 高并发Session测试
- 大消息历史测试
- 内存泄漏检测
