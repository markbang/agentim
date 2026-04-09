# AgentIM

A Rust IM bridge that routes messages from multiple messaging platforms to local agent backends via webhooks.

> Codex bridge note: AgentIM now targets `codex app-server` JSON-RPC for local Codex integration.
> The legacy `src/acp.rs` client still exists for ACP transport experiments/tests, but the verified
> Codex runtime surface is `thread/start`, `thread/resume`, and `turn/start`. See
> [`docs/codex-app-server-transport-review.md`](docs/codex-app-server-transport-review.md).

## Features

- **8 IM platforms** with real webhook handlers and message delivery
- **Local Codex backend by default** via `codex app-server`
- **Advanced local backend overrides** for command, args, cwd, and env
- **Per-platform agent routing** with priority-based routing rules
- **Session management** with auto-creation, history trimming, and context windowing
- **Security** - shared secret auth, HMAC-SHA256 signed webhooks, platform-native signature verification (Telegram, Discord ed25519, Feishu, Slack, DingTalk, LINE)
- **Persistence** - JSON session state with atomic writes and backup rotation
- **Production-ready** - graceful shutdown, request body limits, session TTL cleanup, health/readiness endpoints
- **Docker support** with compose file included

## Supported Platforms

| Platform | Webhook Endpoint | Signature Verification |
|----------|-----------------|----------------------|
| Telegram | `POST /telegram` | Native secret token |
| Discord | `POST /discord` | Ed25519 signatures |
| Feishu/Lark | `POST /feishu` | Verification token + URL challenge |
| Slack | `POST /slack` | HMAC-SHA256 |
| DingTalk | `POST /dingtalk` | HMAC-SHA256 |
| LINE | `POST /line` | HMAC-SHA256 |
| QQ | `POST /qq` | - |
| WeChat Work | `POST /wechatwork` | - |

## Quick Start

### Prerequisites

- Rust 1.70+ (for building from source)
- Local `codex` CLI installed and authenticated

### Run directly

```bash
cargo build --release

# Minimal: just Telegram
./target/release/agentim \
  --telegram-token YOUR_TELEGRAM_BOT_TOKEN

# Override the backend command if needed
./target/release/agentim \
  --codex-command codex \
  --codex-arg app-server \
  --codex-cwd /path/to/worktree \
  --telegram-token YOUR_TELEGRAM_TOKEN \
  --discord-token YOUR_DISCORD_TOKEN \
  --slack-token xoxb-YOUR-SLACK-TOKEN \
  --addr 0.0.0.0:8080
```

### Run with config file

```bash
cp agentim.json.example config.json
# Edit config.json with your credentials
./target/release/agentim --config-file config.json
```

### Run with Docker

```bash
mkdir -p config state
cp agentim.json.example config/config.json
# Edit config/config.json with your credentials
docker compose up -d
```

### Run with environment variables

```bash
export TELEGRAM_TOKEN=your-telegram-token
./start.sh
```

## Configuration

AgentIM supports three configuration methods (CLI flags take precedence over config file):

### CLI Flags

```
--agent codex               Default agent type
--codex-command CMD         Backend command (default: codex)
--codex-arg ARG             Repeat to pass backend args (default path uses app-server)
--codex-cwd PATH            Backend working directory (default: current directory)
--codex-env KEY=VALUE       Repeat to pass backend env vars

--telegram-token TOKEN      Enable Telegram bot
--discord-token TOKEN       Enable Discord bot
--feishu-app-id ID          Enable Feishu bot (requires --feishu-app-secret)
--feishu-app-secret SECRET
--slack-token TOKEN         Enable Slack bot
--dingtalk-token TOKEN      Enable DingTalk bot
--qq-bot-id ID              Enable QQ bot (requires --qq-bot-token)
--qq-bot-token TOKEN

--config-file PATH          Load config from JSON file
--addr HOST:PORT            Server address (default: 127.0.0.1:8080)
--dry-run                   Validate config and exit
```

### Config File (JSON)

See [`agentim.json.example`](agentim.json.example) for a complete example.

### Routing Rules

Route different users or channels to different agent configurations:

```json
{
  "routing_rules": [
    {
      "channel": "telegram",
      "user_id": "12345",
      "priority": 10,
      "agent": "codex"
    },
    {
      "channel": "discord",
      "reply_target_prefix": "support-",
      "priority": 1,
      "agent": "codex"
    }
  ]
}
```

Rules match on: `channel`, `user_id`, `user_prefix`, `reply_target`, `reply_target_prefix`. Higher priority wins when multiple rules match.

## Security

### Shared Secret

Require a secret header on all webhook requests:

```bash
--webhook-secret YOUR_SECRET
# Clients must send: x-agentim-secret: YOUR_SECRET
```

### Signed Webhooks (HMAC)

Verify webhook authenticity with timestamp + nonce + HMAC-SHA256:

```bash
--webhook-signing-secret YOUR_SIGNING_SECRET
--webhook-max-skew-seconds 300
```

### Platform-Native Verification

```bash
--telegram-webhook-secret-token TOKEN
--discord-interaction-public-key HEX_KEY
--feishu-verification-token TOKEN
--slack-signing-secret SECRET
--dingtalk-secret SECRET
```

## Operations

| Endpoint | Description |
|----------|-------------|
| `GET /healthz` | Liveness probe (always 200 if running) |
| `GET /readyz` | Readiness probe (checks agents and channels are registered) |
| `GET /reviewz` | Runtime configuration and session stats |

## Session Management

- Sessions are auto-created per user+channel combination
- `--max-session-messages N` trims history after each turn
- `--context-message-limit N` limits messages sent to the agent
- `--session-ttl-seconds N` cleans up idle sessions automatically
- `--state-file PATH` persists sessions across restarts
- `--state-backup-count N` keeps rotated backup snapshots

## Architecture

```
Webhook Request (Telegram/Discord/Feishu/...)
    |
    v
Axum Router (bot_server.rs)
    |-- Security verification (shared secret / HMAC / platform-native)
    |-- Routing rules (priority-based agent selection)
    v
AgentIM Manager (manager.rs)
    |-- Find or create session
    |-- Build context window from history
    v
Codex app-server Backend (codex.rs)
    |-- thread/start or thread/resume
    |-- turn/start + streamed agentMessage deltas
    v
Channel Reply (bots/*.rs)
    |-- Platform-specific message delivery
    v
Persist session state (if configured)
```

## Building & Testing

```bash
cargo build --release     # build
cargo test                # run all tests
cargo clippy              # lint
cargo fmt                 # format
```

## License

[MIT](LICENSE)
