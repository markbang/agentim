# CLI Agent 使用指南

## 概述

CLI Agent是一个交互式Agent，允许用户通过命令行直接与AgentIM系统通讯。用户可以在CLI中输入响应，而不需要连接到外部AI服务。

## 快速开始

### 1. 注册CLI Agent

```bash
# 使用CLI命令注册
./target/release/agentim agent register --id cli-agent --agent-type cli

# 或在交互模式中选择
./target/release/agentim interactive
# 选择 "1. Register Agent"
# 输入 Agent ID: cli-agent
# 选择 Agent Type: 4. CLI (Interactive)
```

### 2. 注册Channel

```bash
# 注册一个Channel（例如Telegram）
./target/release/agentim channel register --id my-channel --channel-type telegram
```

### 3. 创建Session

```bash
# 创建一个Session连接CLI Agent和Channel
./target/release/agentim session create \
  --agent-id cli-agent \
  --channel-id my-channel \
  --user-id user123
```

### 4. 发送消息

```bash
# 发送消息到Session
./target/release/agentim session send \
  --session-id <session-id> \
  --message "Hello, CLI Agent!"
```

系统会显示消息历史，然后提示你输入响应。

## 交互模式使用

最简单的方式是使用交互模式：

```bash
./target/release/agentim interactive
```

然后按照菜单操作：

1. **Register Agent** - 选择 "4. CLI (Interactive)"
2. **Register Channel** - 选择你想要的Channel类型
3. **Create Session** - 连接Agent和Channel
4. **Send Message** - 输入消息并在CLI中响应

## CLI Agent工作流程

```
用户输入消息
    ↓
消息发送到Session
    ↓
CLI Agent接收消息
    ↓
显示消息历史
    ↓
提示用户输入响应
    ↓
用户在CLI中输入响应
    ↓
响应发送回Channel
    ↓
用户通过Channel接收响应
```

## 示例场景

### 场景1: 本地测试

```bash
# 1. 启动交互模式
./target/release/agentim interactive

# 2. 注册CLI Agent
# 选择: 1 (Register Agent)
# Agent ID: test-agent
# Agent Type: 4 (CLI)

# 3. 注册本地Channel
# 选择: 2 (Register Channel)
# Channel ID: local-channel
# Channel Type: 1 (Telegram) - 或任何其他类型

# 4. 创建Session
# 选择: 3 (Create Session)
# 选择 Agent: test-agent
# 选择 Channel: local-channel
# User ID: testuser

# 5. 发送消息
# 选择: 4 (Send Message)
# 选择 Session: test-agent-local-channel-testuser
# 输入消息: "What is 2+2?"
#
# 系统显示:
# 📋 Message History:
#   1. 👤 User: What is 2+2?
#
# 💬 Enter your response (or 'skip' to skip):
# > 2+2 equals 4
#
# 响应被发送回Channel
```

### 场景2: 多Agent协作

```bash
# 创建多个Agent
./target/release/agentim agent register --id cli-1 --agent-type cli
./target/release/agentim agent register --id cli-2 --agent-type cli

# 创建多个Channel
./target/release/agentim channel register --id channel-1 --channel-type telegram
./target/release/agentim channel register --id channel-2 --channel-type discord

# 创建多个Session
./target/release/agentim session create --agent-id cli-1 --channel-id channel-1 --user-id user1
./target/release/agentim session create --agent-id cli-2 --channel-id channel-2 --user-id user2

# 现在可以在不同的Session中进行交互
```

## 命令参考

### 注册CLI Agent
```bash
./target/release/agentim agent register --id <agent-id> --agent-type cli
```

### 列出所有Agent
```bash
./target/release/agentim agent list
```

### 创建Session
```bash
./target/release/agentim session create \
  --agent-id <agent-id> \
  --channel-id <channel-id> \
  --user-id <user-id>
```

### 发送消息
```bash
./target/release/agentim session send \
  --session-id <session-id> \
  --message "<message>"
```

### 查看Session详情
```bash
./target/release/agentim session get --id <session-id>
```

### 列出所有Session
```bash
./target/release/agentim session list
```

## 特殊命令

在CLI Agent提示输入响应时，你可以使用以下特殊命令：

- **skip** - 跳过当前消息，不发送响应
- **quit** - 退出（在某些情况下）

## 配置示例

在 `agentim.json` 中配置CLI Agent：

```json
{
  "agents": [
    {
      "id": "cli-agent",
      "agent_type": "cli",
      "model": null
    }
  ],
  "channels": [
    {
      "id": "my-channel",
      "channel_type": "telegram"
    }
  ],
  "sessions": [
    {
      "agent_id": "cli-agent",
      "channel_id": "my-channel",
      "user_id": "user123"
    }
  ]
}
```

然后运行：
```bash
./start.sh
```

## 使用场景

### 1. 本地开发和测试
- 测试消息流程
- 调试Session管理
- 验证Channel集成

### 2. 交互式演示
- 展示AgentIM功能
- 实时演示消息处理
- 用户交互演示

### 3. 手动消息处理
- 需要人工审核的消息
- 复杂的决策流程
- 需要用户输入的场景

### 4. 混合Agent系统
- 结合Claude、Codex等AI Agent
- 需要人工干预的工作流
- 多Agent协作

## 故障排除

### 问题: CLI Agent无法接收消息
**解决方案**:
1. 检查Session是否正确创建
2. 确认Agent ID和Channel ID匹配
3. 查看系统状态: `./target/release/agentim status`

### 问题: 消息无法发送回Channel
**解决方案**:
1. 检查Channel是否正确注册
2. 确认Channel类型是否支持
3. 查看Channel健康状态: `./target/release/agentim channel health --id <channel-id>`

### 问题: 交互提示不显示
**解决方案**:
1. 确保在终端中运行（不是后台）
2. 检查stdin是否正确连接
3. 尝试重新启动应用

## 高级用法

### 与其他Agent混合使用

```bash
# 注册多个Agent
./target/release/agentim agent register --id claude-1 --agent-type claude
./target/release/agentim agent register --id cli-1 --agent-type cli

# 为不同的用户创建不同的Session
./target/release/agentim session create --agent-id claude-1 --channel-id tg-1 --user-id user1
./target/release/agentim session create --agent-id cli-1 --channel-id tg-1 --user-id user2

# 现在user1的消息由Claude处理，user2的消息由CLI Agent处理
```

### 配置持久化

```bash
# 编辑agentim.json
vim agentim.json

# 启动时自动加载配置
./start.sh
```

## 最佳实践

1. **使用有意义的ID** - 使用描述性的Agent和Channel ID
2. **定期检查状态** - 使用 `status` 命令监控系统
3. **测试消息流** - 在生产前充分测试
4. **记录Session** - 保存重要的Session ID以便后续查询
5. **错误处理** - 在CLI中输入有效的响应

## 下一步

- 集成真实的AI Agent（Claude、Codex等）
- 添加数据库持久化
- 实现消息队列
- 添加Web API接口
