# AgentIM

AgentIM is a Rust bridge that connects chat platforms to ACP-compatible agent backends.

## Install

macOS / Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/markbang/agentim/main/install.sh | sh
```

This installs `agentim` into `~/.local/bin` by default. Override with:

```bash
curl -fsSL https://raw.githubusercontent.com/markbang/agentim/main/install.sh | AGENTIM_INSTALL_DIR=/your/bin sh
```

By default, AgentIM talks to a local `acp` process, so the minimal local setup is close to:

- install your ACP-compatible agent runtime
- provide a bot token
- start AgentIM

## Features

- Telegram, Discord, Feishu/Lark, Slack, DingTalk, LINE, QQ, and WeChat Work webhook support
- ACP-only backend runtime via a local process
- Session persistence, history trimming, and context limits
- Routing rules by channel, user, and reply target
- Shared-secret, signed-webhook, and platform-native verification support
- Health, readiness, and review endpoints
- Docker support

## Supported Platforms

| Platform | Delivery | Verification |
| --- | --- | --- |
| Telegram | Long polling (`getUpdates`) | Bot token |
| Discord | `POST /discord` | Ed25519 |
| Feishu/Lark | `POST /feishu` | Verification token + URL challenge |
| Slack | `POST /slack` | HMAC-SHA256 |
| DingTalk | `POST /dingtalk` | HMAC-SHA256 |
| LINE | `POST /line` | HMAC-SHA256 |
| QQ | `POST /qq` | - |
| WeChat Work | `POST /wechatwork` | - |

## Requirements

- Rust 1.70+
- Local ACP-compatible agent backend installed

## Quick Start

### Minimal local run

```bash
agentim --telegram-token YOUR_TELEGRAM_BOT_TOKEN
```

Telegram does not require webhook setup in AgentIM.

### Override the backend launch

```bash
./target/release/agentim \
  --acp-command your-agent \
  --acp-arg serve \
  --acp-cwd /path/to/worktree \
  --telegram-token YOUR_TELEGRAM_BOT_TOKEN
```

### Config-file run

```bash
cp agentim.json.example config.json
# edit config.json
./target/release/agentim --config-file config.json
```

### Docker

```bash
mkdir -p config state
cp agentim.json.example config/config.json
# edit config/config.json
docker compose up -d
```

### Environment-driven startup

```bash
export TELEGRAM_TOKEN=your-telegram-token
./start.sh
```

## Configuration

CLI flags override config-file values.

Core flags:

```bash
--agent acp
--acp-command CMD
--acp-arg ARG
--acp-cwd PATH
--acp-env KEY=VALUE
--config-file PATH
--addr HOST:PORT
--dry-run
```

Platform flags:

```bash
--telegram-token TOKEN
--discord-token TOKEN
--feishu-app-id ID --feishu-app-secret SECRET
--slack-token TOKEN
--dingtalk-token TOKEN
--line-channel-token TOKEN --line-channel-secret SECRET
--wechatwork-corp-id ID --wechatwork-agent-id ID --wechatwork-secret SECRET
--qq-bot-id ID --qq-bot-token TOKEN
```

See [agentim.json.example](agentim.json.example) for the full config shape.

### Routing rules

Routing rules can override the selected agent by:

- `channel`
- `user_id`
- `user_prefix`
- `reply_target`
- `reply_target_prefix`

Higher priority wins.

## Security

AgentIM supports:

- shared secret auth via `x-agentim-secret`
- signed webhooks via timestamp + nonce + HMAC
- native platform verification for Discord, Feishu, Slack, and DingTalk

## Operations

| Endpoint | Purpose |
| --- | --- |
| `GET /healthz` | Liveness |
| `GET /readyz` | Readiness |
| `GET /reviewz` | Runtime config + session stats |

## Session Behavior

- sessions are created per user + channel
- `--max-session-messages` trims history after replies
- `--context-message-limit` bounds context sent to the ACP agent
- `--session-ttl-seconds` expires idle sessions
- `--state-file` persists sessions across restarts
- `--state-backup-count` keeps rotated backups

## Development

```bash
cargo build --release
cargo test
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```

## License

[MIT](LICENSE)
