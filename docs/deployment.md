# Deployment Guide

## Recommended Modes

AgentIM supports two practical deployment modes.

### 1. Single-instance

Recommended default:

- one AgentIM instance
- one ACP backend process or reachable ACP service
- one writable local state path
- file-backed `SessionStore`
- file-backed `LeaseStore` for local listener ownership
- process supervisor or container restart policy

### 2. HA / multi-instance foundation

Supported by abstraction layer:

- shared `SessionStore`
- shared `LeaseStore`
- leader-only listeners through `ListenerRuntimeConfig`
- Redis-backed implementations available with `--features redis-store`

## Single Instance Deployment

Use when:

- local disk persistence is acceptable
- only one polling/gateway owner should exist
- operational simplicity is preferred

## Multi-Instance / HA Constraints

Current constraints:

- do not share the same local `state_file` across multiple replicas
- do not run multiple Telegram polling replicas for the same bot token without a shared lease backend
- webhook-only replicas still need shared session persistence if you want coherent cross-instance session continuity

The codebase exposes:

- `SessionStore` abstraction for pluggable session persistence
- `LeaseStore` abstraction for listener ownership / leader-only execution
- file-based reference implementations
- Redis-backed implementations behind the optional `redis-store` feature

## Redis-backed HA Notes

Build with:

```bash
cargo build --features redis-store
```

Recommended Redis usage:

- one key namespace for session snapshots
- one key namespace for listener leases
- distinct lease keys per listener type / bot identity
- short lease TTL with regular renewals

Redis is appropriate for:

- shared listener ownership
- shared session snapshots
- active/passive or horizontally scaled webhook workers

## Container Notes

- mount config and state directories explicitly
- inject secrets through environment variables or secret mounts
- expose `/healthz`, `/readyz`, `/reviewz`, and `/metrics`
- if scraping metrics externally, consider separate ingress/auth policy for `/metrics`

## Dry Run Before Deploy

```bash
cargo run -- --dry-run --agent acp --acp-command acp
```
