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
- Telegram polling listener and Discord gateway listener under a shared listener runtime
- ACP-only backend runtime via a local process
- Session persistence, history trimming, and context limits
- Routing rules by channel, user, and reply target
- Shared-secret, signed-webhook, and platform-native verification support
- Health, readiness, review, and metrics endpoints
- Optional Redis-backed `SessionStore` / `LeaseStore` via `--features redis-store`
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
--metrics-secret SECRET
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

### Secrets Management

**Best practices for production:**

1. **Never commit secrets to git** - Add config files with tokens to `.gitignore`
2. **Use environment variables** - Pass tokens via `-e` (Docker) or shell exports
3. **Use secret managers** - Kubernetes secrets, Docker secrets, HashiCorp Vault
4. **Rotate regularly** - Update platform tokens on a schedule

```bash
# Environment variables (recommended)
export TELEGRAM_TOKEN="your-token"
export DISCORD_TOKEN="your-token"
./start.sh

# Docker with secrets
docker run -e TELEGRAM_TOKEN=$TOKEN agentim

# Kubernetes secrets
kubectl create secret generic agentim-secrets --from-literal=telegram-token=$TOKEN
```

See [docs/deployment.md](docs/deployment.md) for detailed deployment guide.

## Operations

| Endpoint | Purpose |
| --- | --- |
| `GET /healthz` | Liveness |
| `GET /readyz` | Readiness |
| `GET /reviewz` | Runtime config + session stats |
| `GET /metrics` | Prometheus-compatible runtime metrics |

### Deployment Constraints

AgentIM supports two deployment shapes:

1. **Single instance**
   - local file state
   - file lease store for listener ownership
   - simplest recommended default

2. **HA / multi-instance foundation**
   - shared `SessionStore`
   - shared `LeaseStore`
   - listener leader-only execution
   - Redis-backed implementations are available behind `--features redis-store`

For pure local deploys, keep using a single instance with a process supervisor.
For HA, use shared remote state + lease backends and ensure only one listener owner per platform token.

See [docs/deployment.md](docs/deployment.md) for details.

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

## Documentation

| Document | Purpose |
| --- | --- |
| [docs/deployment.md](docs/deployment.md) | Deployment modes, HA constraints, container setup |
| [docs/operations.md](docs/operations.md) | Startup/shutdown, session management, troubleshooting |
| [docs/recovery.md](docs/recovery.md) | Failure recovery procedures, disaster recovery |
| [docs/upgrade-guide.md](docs/upgrade-guide.md) | Version migration, compatibility, rollback |
| [docs/soak-test.md](docs/soak-test.md) | Soak test harness and production soak guidance |
| [docs/listener-runtime.md](docs/listener-runtime.md) | Inbound listener abstraction, supervisor, checkpoint model |
| [docs/acp-transport-review.md](docs/acp-transport-review.md) | ACP backend architecture |

### Optional Redis build

Enable Redis-backed state/lease implementations with:

```bash
cargo build --features redis-store
```

## License

[MIT](LICENSE)
