# Operations Guide

## Deployment Model

Current production model is **single-writer / single-instance preferred**.

Important constraints:

- Session state is stored in-process and optionally persisted to a local state file.
- Telegram uses long polling when configured, so only one active polling instance should run per bot token.
- Multiple instances must **not** share the same local state file path.

## Recommended Deployment Patterns

### Single instance

Recommended for most deployments:

- one AgentIM process
- one ACP backend process or service
- local state file persistence enabled

### HA / multi-instance

Not fully supported with local file persistence alone.

If you run multiple replicas:

- disable Telegram polling on all but one leader instance
- do not point multiple replicas at the same state file
- use an external shared session/state store in a future architecture revision

## Build Verification

Run before release:

```bash
cargo build
cargo build --release
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```

## Dry Run Validation

Validate startup configuration without contacting live backends:

```bash
cargo run -- --dry-run --agent acp --acp-command acp
```

## Metrics

Metrics are exposed at:

- `GET /metrics`

Protect with `x-agentim-secret` when `webhook_secret` is enabled.

Current metrics include:

- `agentim_webhook_requests_total`
- `agentim_webhook_failures_total`
- `agentim_agent_latency_ms`
- `agentim_channel_send_latency_ms`
- `agentim_active_sessions`
- `agentim_session_cleanup_total`
- `agentim_auth_reject_total`

## Logging Guidance

Use structured logs and avoid printing:

- bot tokens
- signing secrets
- full raw payloads in production

## Secret Management

Recommended production practice:

- inject secrets via environment variables or secret mounts
- do not commit tokens into source control
- rotate platform tokens and signing secrets periodically

## Upgrade Guidance

Before upgrading:

- back up the session state file
- run dry-run validation with the new binary
- confirm only one Telegram polling instance is active
