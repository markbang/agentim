# AgentIM Bot Integration Guide

## 概述

AgentIM 当前通过一个统一的 webhook server 接入各 IM 平台，并把收到的消息桥接到默认 agent；也可以按平台覆盖 agent，或者通过 `routing_rules` 对特定用户做更细粒度路由。默认 agent 现在既可以是内置 mock agent，也可以是一个真实 OpenAI-compatible HTTP backend。

统一桥接步骤：

1. 接收平台 webhook
2. 解析平台 payload
3. `find_or_create_session(agent, channel, user)`
4. 在 session metadata 中记录 `reply_target`
5. 调用 agent
6. 把响应发回正确的平台目标

这套逻辑对 Telegram / Discord / Feishu / QQ 都适用，其中 Discord / QQ 的 `reply_target` 会是频道 ID，而不是用户 ID。

## 当前 webhook / ops 路由

- `POST /telegram`
- `POST /discord`
- `POST /feishu`
- `POST /qq`
- `GET /healthz`
- `GET /reviewz`

## 启动方式

### Telegram

```bash
cargo run -- --agent claude --telegram-token "$TELEGRAM_TOKEN"
```

如果要直接接一个真实 OpenAI-compatible backend：

```bash
cargo run -- \
  --agent openai \
  --openai-api-key "$OPENAI_API_KEY" \
  --openai-base-url "${OPENAI_BASE_URL:-https://api.openai.com/v1}" \
  --openai-model "${OPENAI_MODEL:-gpt-4o-mini}" \
  --openai-max-retries 1 \
  --telegram-token "$TELEGRAM_TOKEN"
```

如果希望 Telegram 单独走另一个 agent：

```bash
cargo run -- --agent claude --telegram-agent pi --telegram-token "$TELEGRAM_TOKEN"
```

### Discord

```bash
cargo run -- --agent claude --discord-token "$DISCORD_TOKEN"
```

如需启用 Discord 原生 interaction 签名校验：

```bash
cargo run -- \
  --agent claude \
  --discord-token "$DISCORD_TOKEN" \
  --discord-interaction-public-key "$DISCORD_INTERACTION_PUBLIC_KEY"
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

其中 `AGENTIM_DRY_RUN=1` 会调用二进制的 `--dry-run` 路径，离线验证 webhook / 凭证配置而不做真实健康检查。

如果需要一个统一的轻量保护层，可以设置共享密钥：

```bash
export AGENTIM_WEBHOOK_SECRET=change-me
# 所有 webhook 请求都需要 x-agentim-secret: change-me
```

如果需要更强一点的 webhook 防护，可以启用签名校验 + replay 保护：

```bash
export AGENTIM_WEBHOOK_SIGNING_SECRET=change-me-signing
export AGENTIM_WEBHOOK_MAX_SKEW_SECONDS=300
# x-agentim-signature = sha256(HMAC(secret, timestamp + "\n" + nonce + "\n" + raw_body))
# 并附带 x-agentim-timestamp / x-agentim-nonce
```

Telegram 还支持一个更原生的 secret token 校验头：

```bash
export TELEGRAM_WEBHOOK_SECRET_TOKEN=tg-native-secret
# 请求需要带 x-telegram-bot-api-secret-token: tg-native-secret
```

Discord 也支持按官方 interaction headers 做原生签名校验：

```bash
export DISCORD_INTERACTION_PUBLIC_KEY=discord-public-key-hex
# /discord 会校验 x-signature-ed25519 + x-signature-timestamp
```

Feishu 也支持按 payload 里的原生 token 做校验：

```bash
export FEISHU_WEBHOOK_VERIFICATION_TOKEN=feishu-native-token
# /feishu 会校验 JSON body 里的 token 字段
```

如果你希望 session 历史不会无限增长，并把送给 agent 的上下文窗口与本地保留历史解耦：

```bash
export AGENTIM_MAX_SESSION_MESSAGES=50
export AGENTIM_CONTEXT_MESSAGE_LIMIT=12
export AGENTIM_AGENT_TIMEOUT_MS=30000
export AGENTIM_STATE_BACKUP_COUNT=2
```

主快照损坏时，AgentIM 会在启动恢复阶段自动尝试最近的 `.bak.N` 备份。
`AGENTIM_MAX_SESSION_MESSAGES` 控制最终保留多少会话历史，`AGENTIM_CONTEXT_MESSAGE_LIMIT` 控制每轮调用 agent 时最多带多少条上下文，`AGENTIM_AGENT_TIMEOUT_MS` 控制单次 agent 调用的最长耗时。

如果需要把某个用户、某个回复目标，或者某类回复目标前缀路由到特殊 agent，可以在 `agentim.json` 中配置：

```json
{
  "routing_rules": [
    {"channel": "telegram", "user_id": "vip-user", "priority": 10, "agent": "pi"},
    {"channel": "discord", "reply_target_prefix": "review-", "priority": 1, "agent": "codex"}
  ]
}
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
- 支持 `type=url_verification` challenge 直返 challenge 响应，便于 webhook 首次接入
- 支持 `--feishu-verification-token` / `FEISHU_WEBHOOK_VERIFICATION_TOKEN` 校验 payload 里的 `token`

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
- 生产 webhook 至少启用一层鉴权：全局共享密钥、全局 HMAC 签名，或平台原生验签
- QQ / DingTalk 没有独立平台验签时，必须依赖全局共享密钥或全局签名校验
- session 快照已经支持后台异步落盘和 `.bak.N` 轮转；需要更强 durability 时再接外部存储
- 当前真实生产 agent 入口是 `openai` 或 `acp`；内置 `claude` / `codex` / `pi` 仍是本地模拟实现，仅适合开发和 dry-run
