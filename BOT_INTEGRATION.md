# AgentIM Bot Integration Guide

## 概述

AgentIM 当前通过一个统一的 webhook server 接入各 IM 平台，并把收到的消息桥接到默认 agent；也可以按平台覆盖 agent。

统一桥接步骤：

1. 接收平台 webhook
2. 解析平台 payload
3. `find_or_create_session(agent, channel, user)`
4. 在 session metadata 中记录 `reply_target`
5. 调用 agent
6. 把响应发回正确的平台目标

这套逻辑对 Telegram / Discord / Feishu / QQ 都适用，其中 Discord / QQ 的 `reply_target` 会是频道 ID，而不是用户 ID。

## 当前 webhook

- `POST /telegram`
- `POST /discord`
- `POST /feishu`
- `POST /qq`

## 启动方式

### Telegram

```bash
cargo run -- --agent claude --telegram-token "$TELEGRAM_TOKEN"
```

如果希望 Telegram 单独走另一个 agent：

```bash
cargo run -- --agent claude --telegram-agent pi --telegram-token "$TELEGRAM_TOKEN"
```

### Discord

```bash
cargo run -- --agent claude --discord-token "$DISCORD_TOKEN"
```

### Feishu

```bash
cargo run -- \
  --agent claude \
  --feishu-app-id "$FEISHU_APP_ID" \
  --feishu-app-secret "$FEISHU_APP_SECRET"
```

### QQ

```bash
cargo run -- \
  --agent claude \
  --qq-bot-id "$QQ_BOT_ID" \
  --qq-bot-token "$QQ_BOT_TOKEN"
```

也可以用：

```bash
AGENTIM_DRY_RUN=1 ./start.sh
./start.sh
```

如果需要一个统一的轻量保护层，可以设置共享密钥：

```bash
export AGENTIM_WEBHOOK_SECRET=change-me
# 所有 webhook 请求都需要 x-agentim-secret: change-me
```

## 平台说明

### Telegram
- 回复目标：`chat.id`
- 路由：`/telegram`
- 参数：`--telegram-token`

### Discord
- 回复目标：`channel_id`
- 路由：`/discord`
- 参数：`--discord-token`

### Feishu
- 回复目标：`sender_id.user_id`
- 路由：`/feishu`
- 参数：`--feishu-app-id --feishu-app-secret`
- 兼容旧格式：`--feishu-token app_id:app_secret`

### QQ
- 回复目标：`channel_id`
- 路由：`/qq`
- 参数：`--qq-bot-id --qq-bot-token`
- 兼容旧格式：`--qq-token bot_id:bot_token`

## Review / Eval

为了持续 review 和 eval 这个 bridge 的可用性，建议每次改动后跑：

```bash
cargo test --test review_bridge
./autoresearch.sh
```

`review_bridge` 关注：
- 四个平台 webhook 是否都能进入统一桥接逻辑
- reply target 是否正确
- session 是否按用户和平台复用

`autoresearch.sh` 关注：
- cargo test / help / startup dry-run
- webhook 路由覆盖
- review 覆盖是否存在

## 生产部署提醒

- webhook 入口必须走 HTTPS
- 需要补齐平台签名校验
- 真实部署建议把 session 存储外置
- 当前内置 agent 仍是本地模拟实现，接真实模型前要补 API 适配层
