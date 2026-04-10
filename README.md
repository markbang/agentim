# AgentIM

AgentIM is a Rust bridge that connects chat platforms to a local Codex backend through webhooks.

By default, AgentIM talks to `codex app-server`, so the minimal local setup is close to:

- install and log into `codex`
- provide a bot token
- start AgentIM

For transport details, see [docs/codex-app-server-transport-review.md](docs/codex-app-server-transport-review.md).

## Features

- Telegram, Discord, Feishu/Lark, Slack, DingTalk, LINE, QQ, and WeChat Work webhook support
- Local Codex backend by default via `codex app-server`
- Session persistence, history trimming, and context limits
- Routing rules by channel, user, and reply target
- Shared-secret, signed-webhook, and platform-native verification support
- Health, readiness, and review endpoints
- Docker support

## Supported Platforms

| Platform | Endpoint | Verification |
| --- | --- | --- |
| Telegram | `POST /telegram` | Native secret token |
| Discord | `POST /discord` | Ed25519 |
| Feishu/Lark | `POST /feishu` | Verification token + URL challenge |
| Slack | `POST /slack` | HMAC-SHA256 |
| DingTalk | `POST /dingtalk` | HMAC-SHA256 |
| LINE | `POST /line` | HMAC-SHA256 |
| QQ | `POST /qq` | - |
| WeChat Work | `POST /wechatwork` | - |

## Requirements

- Rust 1.70+
- Local `codex` CLI installed and authenticated

## Quick Start

### Minimal local run

```bash
cargo build --release
./target/release/agentim \
  --telegram-token YOUR_TELEGRAM_BOT_TOKEN
```

### Override the backend launch

```bash
./target/release/agentim \
  --codex-command codex \
  --codex-arg app-server \
  --codex-cwd /path/to/worktree \
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
--agent codex
--codex-command CMD
--codex-arg ARG
--codex-cwd PATH
--codex-env KEY=VALUE
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
- native platform verification for Telegram, Discord, Feishu, Slack, and DingTalk

## Operations

| Endpoint | Purpose |
| --- | --- |
| `GET /healthz` | Liveness |
| `GET /readyz` | Readiness |
| `GET /reviewz` | Runtime config + session stats |

## Session Behavior

- sessions are created per user + channel
- `--max-session-messages` trims history after replies
- `--context-message-limit` bounds context sent to Codex
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
