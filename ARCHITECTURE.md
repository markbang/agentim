# AgentIM Architecture

## Goal

AgentIM is an IM bridge, not a model-hosting layer.

The primary runtime model is:

```text
Telegram / Discord / Feishu / QQ / Slack / DingTalk
                  -> AgentIM
                  -> ACP-compatible coding agent
```

In this model:

- AgentIM owns ingress, session state, routing, reply targets, persistence, and webhook verification.
- The external ACP agent owns provider selection, model selection, API keys, tools, and inference behavior.
- The built-in `openai` backend still exists, but only as an optional compatibility path.

## High-Level Flow

```text
platform message
  -> channel adapter
  -> session lookup or creation
  -> routing resolution
  -> selected agent backend
  -> platform-specific reply target
```

Each incoming event is normalized into a per-user session. AgentIM then:

1. Resolves which agent variant should handle the message.
2. Builds the context window from stored session history.
3. Sends the prompt to the chosen backend.
4. Persists the assistant reply and any backend metadata.
5. Sends the response back to the correct platform target.

## Main Components

### Ingress

Supported platform ingress paths:

- Telegram webhook: `POST /telegram`
- Telegram local mode: `--telegram-poll`
- Discord webhook/interactions: `POST /discord`
- Discord local mode: `--discord-gateway`
- Feishu/Lark webhook: `POST /feishu`
- QQ webhook: `POST /qq`
- Slack webhook: `POST /slack`
- DingTalk webhook: `POST /dingtalk`

Telegram polling and Discord Gateway are the local-first paths. They do not require a public webhook endpoint.

### Routing

Routing is resolved in three layers:

1. Default agent via `--agent` or config `agent`
2. Channel override via `telegram_agent`, `discord_agent`, and similar fields
3. `routing_rules` for user-specific or reply-target-specific overrides

Rules can match on channel, user id, user prefix, reply target, reply-target prefix, and priority.

### Session State

Each conversation is stored as a session with:

- channel id
- user id
- message history
- reply target metadata
- backend metadata such as ACP session identifiers

Session history is trimmed with:

- `max_session_messages`
- `context_message_limit`

State persistence is optional and file-based:

- `state_file`
- `state_backup_count`

Backups are rotated, and startup recovery can fall back to the latest valid snapshot.

### Agent Backends

Supported backend types:

- `acp`
  - recommended path
  - launches an ACP-compatible agent subprocess
  - can reuse remote ACP sessions across turns
  - stores ACP metadata for reconnect and observability
- `openai`
  - optional built-in HTTP backend
  - only used when AgentIM itself should call an OpenAI-compatible API
- `claude`, `codex`, `pi`
  - local stubs kept for development and dry-run validation
  - rejected for production bot-server runtime

### Review and Health Endpoints

Operational endpoints:

- `GET /healthz`
- `GET /reviewz`

`reviewz` surfaces runtime status, channel readiness, persistence configuration, and ACP session observability. Sensitive ACP identifiers are redacted unless explicitly authorized.

## Security Model

Webhook-facing deployments should enable at least one protection layer:

- shared secret: `--webhook-secret`
- signed webhook verification: `--webhook-signing-secret`
- platform-native verification where available

Current native verification support includes:

- Telegram secret token
- Discord interaction signature
- Feishu verification token
- Slack signing secret

For local-only Telegram polling and Discord Gateway, these webhook protections are not required.

## Configuration Surfaces

AgentIM supports three configuration surfaces:

1. CLI flags
2. JSON config file via `--config-file`
3. Wrapper scripts `start.sh` / `setup.sh` that translate environment variables into CLI flags

Precedence is:

```text
CLI flags > config file > binary defaults
```

The wrapper scripts are ACP-aware. If `AGENTIM_ACP_COMMAND` is set and `AGENTIM_AGENT` is omitted, they infer `acp` automatically.

## Current Production Boundary

AgentIM is production-oriented for the bridge layer when you use:

- a real backend: `acp` or `openai`
- authenticated webhook ingress, or local polling/gateway modes
- persistent session storage if restart recovery matters

AgentIM is not intended to replace the coding agent's own runtime, provider management, or tool orchestration.
