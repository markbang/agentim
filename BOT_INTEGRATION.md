# AgentIM Bot Integration Guide

## 概述

AgentIM现在支持真实的IM平台Bot集成。通过Webhook接收来自各平台的消息，并自动路由到AI Agent处理，然后将响应发送回用户。

## 支持的平台

### 1. Telegram Bot
- **API**: Telegram Bot API
- **认证**: Bot Token
- **Webhook**: POST /telegram

### 2. Discord Bot
- **API**: Discord API v10
- **认证**: Bot Token
- **Webhook**: POST /discord

### 3. Feishu (飞书) Bot
- **API**: Feishu Open API
- **认证**: App ID + App Secret
- **Webhook**: POST /feishu

### 4. QQ Bot
- **API**: QQ Bot API
- **认证**: Bot ID + Bot Token
- **Webhook**: POST /qq

## 快速开始

### 1. 启动Bot服务器

```bash
# 使用Telegram Bot Token启动
./target/release/agentim bot-server --telegram-token "YOUR_BOT_TOKEN"

# 指定自定义地址
./target/release/agentim bot-server --telegram-token "YOUR_BOT_TOKEN" --addr "0.0.0.0:8080"
```

### 2. 配置Webhook

#### Telegram
```bash
# 设置Webhook URL
curl -X POST https://api.telegram.org/botYOUR_BOT_TOKEN/setWebhook \
  -H "Content-Type: application/json" \
  -d '{"url": "https://your-domain.com/telegram"}'
```

#### Discord
在Discord Developer Portal中配置Interactions Endpoint URL

#### Feishu
在飞书开放平台配置事件回调地址

#### QQ
在QQ Bot管理后台配置Webhook地址

### 3. 创建Session

```bash
# 创建Agent和Channel
./target/release/agentim agent register --id claude-1 --agent-type claude
./target/release/agentim channel register --id telegram-bot --channel-type telegram

# 创建Session（user_id是Telegram的chat_id）
./target/release/agentim session create \
  --agent-id claude-1 \
  --channel-id telegram-bot \
  --user-id "123456789"
```

### 4. 测试消息流

在Telegram中给Bot发送消息，消息将：
1. 通过Webhook到达AgentIM
2. 被路由到对应的Session
3. 发送给Claude Agent处理
4. 响应自动发送回Telegram用户

## 消息流程

```
User Message (Telegram)
    ↓
Webhook POST /telegram
    ↓
Parse Message & Find Session
    ↓
Send to Agent (Claude)
    ↓
Agent Response
    ↓
Send Message via Telegram API
    ↓
User Receives Response
```

## 配置示例

### Telegram Bot

```bash
# 1. 创建Bot (通过 @BotFather)
# 获得: YOUR_BOT_TOKEN

# 2. 启动AgentIM
./target/release/agentim bot-server --telegram-token "YOUR_BOT_TOKEN"

# 3. 设置Webhook
curl -X POST https://api.telegram.org/botYOUR_BOT_TOKEN/setWebhook \
  -H "Content-Type: application/json" \
  -d '{"url": "https://your-server.com/telegram"}'

# 4. 创建Session
./target/release/agentim agent register --id claude-1 --agent-type claude
./target/release/agentim channel register --id tg-bot --channel-type telegram
./target/release/agentim session create \
  --agent-id claude-1 \
  --channel-id tg-bot \
  --user-id "YOUR_CHAT_ID"
```

### Discord Bot

```bash
# 1. 创建Bot (Discord Developer Portal)
# 获得: BOT_TOKEN

# 2. 启动AgentIM
./target/release/agentim bot-server --discord-token "BOT_TOKEN"

# 3. 在Discord Developer Portal配置:
#    - Interactions Endpoint URL: https://your-server.com/discord
#    - 启用 MESSAGE_CONTENT Intent

# 4. 创建Session
./target/release/agentim agent register --id claude-1 --agent-type claude
./target/release/agentim channel register --id discord-bot --channel-type discord
./target/release/agentim session create \
  --agent-id claude-1 \
  --channel-id discord-bot \
  --user-id "YOUR_USER_ID"
```

### Feishu Bot

```bash
# 1. 创建应用 (飞书开放平台)
# 获得: APP_ID, APP_SECRET

# 2. 启动AgentIM
./target/release/agentim bot-server \
  --feishu-app-id "APP_ID" \
  --feishu-app-secret "APP_SECRET"

# 3. 配置事件回调:
#    - 请求URL: https://your-server.com/feishu
#    - 订阅事件: im:message:receive_v1

# 4. 创建Session
./target/release/agentim agent register --id claude-1 --agent-type claude
./target/release/agentim channel register --id feishu-bot --channel-type feishu
./target/release/agentim session create \
  --agent-id claude-1 \
  --channel-id feishu-bot \
  --user-id "YOUR_USER_ID"
```

### QQ Bot

```bash
# 1. 创建Bot (QQ Bot管理后台)
# 获得: BOT_ID, BOT_TOKEN

# 2. 启动AgentIM
./target/release/agentim bot-server \
  --qq-bot-id "BOT_ID" \
  --qq-bot-token "BOT_TOKEN"

# 3. 配置Webhook:
#    - 地址: https://your-server.com/qq

# 4. 创建Session
./target/release/agentim agent register --id claude-1 --agent-type claude
./target/release/agentim channel register --id qq-bot --channel-type qq
./target/release/agentim session create \
  --agent-id claude-1 \
  --channel-id qq-bot \
  --user-id "YOUR_USER_ID"
```

## 部署到生产环境

### 使用Nginx反向代理

```nginx
server {
    listen 443 ssl;
    server_name your-domain.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### 使用Systemd服务

```ini
[Unit]
Description=AgentIM Bot Server
After=network.target

[Service]
Type=simple
User=agentim
WorkingDirectory=/opt/agentim
ExecStart=/opt/agentim/target/release/agentim bot-server \
  --telegram-token "YOUR_BOT_TOKEN" \
  --addr "127.0.0.1:8080"
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
```

启动服务：
```bash
sudo systemctl start agentim
sudo systemctl enable agentim
```

## 故障排除

### Bot无法接收消息

1. 检查Webhook URL是否正确
2. 确认SSL证书有效
3. 查看AgentIM日志：`journalctl -u agentim -f`

### 消息无法发送回用户

1. 检查Bot Token是否有效
2. 确认Session已创建
3. 检查user_id是否正确

### 性能问题

- 增加服务器资源
- 使用负载均衡
- 启用消息队列（可选）

## 高级配置

### 多Agent支持

```bash
# 创建多个Agent
./target/release/agentim agent register --id claude-1 --agent-type claude
./target/release/agentim agent register --id codex-1 --agent-type codex

# 为不同用户创建不同的Session
./target/release/agentim session create \
  --agent-id claude-1 \
  --channel-id tg-bot \
  --user-id "user1"

./target/release/agentim session create \
  --agent-id codex-1 \
  --channel-id tg-bot \
  --user-id "user2"
```

### 多Channel支持

```bash
# 同一个Agent支持多个Channel
./target/release/agentim channel register --id tg-bot --channel-type telegram
./target/release/agentim channel register --id discord-bot --channel-type discord

./target/release/agentim session create \
  --agent-id claude-1 \
  --channel-id tg-bot \
  --user-id "user1"

./target/release/agentim session create \
  --agent-id claude-1 \
  --channel-id discord-bot \
  --user-id "user2"
```

## 监控和日志

启用详细日志：
```bash
RUST_LOG=debug ./target/release/agentim bot-server --telegram-token "YOUR_BOT_TOKEN"
```

查看系统状态：
```bash
./target/release/agentim status
```

## 安全建议

1. **使用HTTPS**: 所有Webhook必须使用HTTPS
2. **验证签名**: 验证来自平台的请求签名
3. **限制访问**: 使用防火墙限制Bot服务器访问
4. **定期更新**: 保持依赖库最新
5. **监控日志**: 定期检查异常日志

## 下一步

- 实现消息队列以提高吞吐量
- 添加数据库持久化
- 实现更复杂的路由规则
- 添加用户认证和授权
