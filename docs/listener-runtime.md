# Listener Runtime Design

AgentIM contains a reusable inbound listener runtime layer for long-lived bot ingress.

## Core Abstractions

- `src/listener.rs`
  - `InboundListener` trait
  - `ListenerCheckpoint`
  - `ListenerRuntimeConfig`
  - listener supervisor with retry/backoff
  - checkpoint load/save helpers

- `src/listeners/telegram_polling.rs`
  - Telegram polling listener
  - offset-based checkpoint cursor

- `src/listeners/discord_gateway.rs`
  - Discord gateway / websocket listener
  - sequence-based checkpoint cursor

- `src/lease.rs`
  - `LeaseStore` trait
  - `FileLeaseStore` reference implementation
  - optional `RedisLeaseStore` behind feature flag

## Runtime Behavior

Each listener follows:

1. initialize transport state
2. load checkpoint cursor
3. process one cycle (`run_once`)
4. persist checkpoint
5. on failure, back off and retry through supervisor
6. optionally acquire/renew a lease before processing

## Lease Model

`ListenerRuntimeConfig` can carry:

- `lease_store`
- `lease_key`
- `lease_owner`
- `lease_ttl_seconds`
- `max_backoff_ms`

If configured, the listener supervisor attempts leader-only execution through the lease store.

## Current Checkpoint Model

- Telegram: `update_id + 1`
- Discord: last seen gateway sequence

Checkpoint files are stored next to the configured state file using:

```text
<state_file>.<listener_id>.checkpoint.json
```

If a checkpoint file is corrupted, AgentIM renames it to a `.corrupt.<timestamp>.json` file and starts from the listener default checkpoint.

## Redis-backed Coordination

With `--features redis-store`, the codebase also exposes:

- `RedisSessionStore`
- `RedisLeaseStore`

This enables shared session snapshots plus leader-only listener ownership across instances.

## Current Limits

- listener checkpoint persistence is still file-based by default
- Redis-backed implementations are available but not wired into CLI/config yet
- no bundled Postgres-backed store/lease implementation yet
