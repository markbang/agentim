# AgentIM CLI 使用指南

## 命令概览

### Agent 管理

```bash
# 列出所有已注册的Agent
agentim agent list

# 注册Claude Agent
agentim agent register \
  --id claude-main \
  --agent-type claude \
  --api-key sk-ant-xxxxx \
  --model claude-3-5-sonnet-20241022

# 注册Codex Agent
agentim agent register \
  --id codex-main \
  --agent-type codex \
  --api-key sk-xxxxx

# 注册Pi Agent
agentim agent register \
  --id pi-main \
  --agent-type pi \
  --api-key pi-xxxxx

# 检查Agent健康状态
agentim agent health --id claude-main
```

### Channel 管理

```bash
# 列出所有已注册的Channel
agentim channel list

# 注册Telegram Channel
agentim channel register \
  --id tg-main \
  --channel-type telegram \
  --credentials '{"token":"123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11"}'

# 注册Discord Channel
agentim channel register \
  --id discord-main \
  --channel-type discord \
  --credentials '{"token":"YOUR_DISCORD_BOT_TOKEN"}'

# 注册Feishu Channel
agentim channel register \
  --id feishu-main \
  --channel-type feishu \
  --credentials '{"app_id":"YOUR_APP_ID","app_secret":"YOUR_APP_SECRET"}'

# 注册QQ Channel
agentim channel register \
  --id qq-main \
  --channel-type qq \
  --credentials '{"bot_id":"YOUR_BOT_ID","bot_token":"YOUR_BOT_TOKEN"}'

# 检查Channel健康状态
agentim channel health --id tg-main
```

### Session 管理

```bash
# 列出所有活跃Session
agentim session list

# 创建新Session
agentim session create \
  --agent-id claude-main \
  --channel-id tg-main \
  --user-id user123

# 获取Session详情
agentim session get --id <session-id>

# 删除Session
agentim session delete --id <session-id>

# 在Session中发送消息
agentim session send \
  --session-id <session-id> \
  --message "Hello, Claude!"
```

### 系统状态

```bash
# 查看系统状态
agentim status
```

### 交互模式

```bash
# 进入交互模式
agentim interactive
```

## 实际使用场景

### 场景1：设置Telegram + Claude

```bash
# 1. 注册Claude Agent
agentim agent register \
  --id claude-prod \
  --agent-type claude \
  --api-key $ANTHROPIC_API_KEY

# 2. 注册Telegram Channel
agentim channel register \
  --id tg-prod \
  --channel-type telegram \
  --credentials "{\"token\":\"$TELEGRAM_BOT_TOKEN\"}"

# 3. 创建Session
SESSION_ID=$(agentim session create \
  --agent-id claude-prod \
  --channel-id tg-prod \
  --user-id telegram_user_123 | grep -oP 'Session created: \K.*')

# 4. 发送消息
agentim session send \
  --session-id $SESSION_ID \
  --message "What is Rust?"
```

### 场景2：多Agent多Channel设置

```bash
# 注册多个Agent
agentim agent register --id claude-1 --agent-type claude --api-key $KEY1
agentim agent register --id codex-1 --agent-type codex --api-key $KEY2
agentim agent register --id pi-1 --agent-type pi --api-key $KEY3

# 注册多个Channel
agentim channel register --id tg-1 --channel-type telegram --credentials '{"token":"..."}'
agentim channel register --id discord-1 --channel-type discord --credentials '{"token":"..."}'
agentim channel register --id feishu-1 --channel-type feishu --credentials '{"app_id":"...","app_secret":"..."}'

# 为不同用户创建不同的Session
agentim session create --agent-id claude-1 --channel-id tg-1 --user-id alice
agentim session create --agent-id codex-1 --channel-id discord-1 --user-id bob
agentim session create --agent-id pi-1 --channel-id feishu-1 --user-id charlie
```

## 环境变量配置

创建 `.env` 文件：

```bash
# Anthropic (Claude)
ANTHROPIC_API_KEY=sk-ant-xxxxx

# OpenAI (Codex)
OPENAI_API_KEY=sk-xxxxx

# Pi
PI_API_KEY=pi-xxxxx

# Telegram
TELEGRAM_BOT_TOKEN=123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11

# Discord
DISCORD_BOT_TOKEN=YOUR_DISCORD_BOT_TOKEN

# Feishu
FEISHU_APP_ID=YOUR_APP_ID
FEISHU_APP_SECRET=YOUR_APP_SECRET

# QQ
QQ_BOT_ID=YOUR_BOT_ID
QQ_BOT_TOKEN=YOUR_BOT_TOKEN
```

然后在启动前加载：

```bash
source .env
agentim status
```

## 高级用法

### 使用脚本自动化

创建 `setup.sh`：

```bash
#!/bin/bash

set -e

echo "Setting up AgentIM..."

# Load environment
source .env

# Register agents
echo "Registering agents..."
agentim agent register \
  --id claude-main \
  --agent-type claude \
  --api-key $ANTHROPIC_API_KEY

agentim agent register \
  --id codex-main \
  --agent-type codex \
  --api-key $OPENAI_API_KEY

# Register channels
echo "Registering channels..."
agentim channel register \
  --id tg-main \
  --channel-type telegram \
  --credentials "{\"token\":\"$TELEGRAM_BOT_TOKEN\"}"

agentim channel register \
  --id discord-main \
  --channel-type discord \
  --credentials "{\"token\":\"$DISCORD_BOT_TOKEN\"}"

# Verify setup
echo "Verifying setup..."
agentim status

echo "Setup complete!"
```

运行：

```bash
chmod +x setup.sh
./setup.sh
```

### 监控系统

创建 `monitor.sh`：

```bash
#!/bin/bash

while true; do
  clear
  echo "=== AgentIM System Monitor ==="
  echo "Time: $(date)"
  echo ""
  agentim status
  echo ""
  echo "Sessions:"
  agentim session list
  echo ""
  sleep 10
done
```

## 故障排除

### Agent连接失败

```bash
# 检查Agent健康状态
agentim agent health --id claude-main

# 验证API Key
echo $ANTHROPIC_API_KEY
```

### Channel连接失败

```bash
# 检查Channel健康状态
agentim channel health --id tg-main

# 验证凭证
agentim channel list
```

### Session问题

```bash
# 列出所有Session
agentim session list

# 获取特定Session详情
agentim session get --id <session-id>

# 删除有问题的Session
agentim session delete --id <session-id>
```

## 性能优化建议

1. **Session清理**: 定期删除不活跃的Session
2. **Agent轮转**: 使用多个Agent实例进行负载均衡
3. **Channel缓存**: 缓存Channel连接以减少开销
4. **消息批处理**: 批量处理消息以提高吞吐量

## 安全建议

1. **API Key管理**: 使用环境变量或密钥管理系统
2. **权限控制**: 限制CLI访问权限
3. **日志审计**: 启用日志记录以追踪操作
4. **速率限制**: 配置API速率限制以防止滥用
