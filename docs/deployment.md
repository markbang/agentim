# Deployment Guide

## Recommended Mode

AgentIM currently targets **single-instance deployment**.

Reasons:

- in-memory active session registry
- optional local file-based session persistence
- Telegram long polling must be leader-only

## Single Instance Deployment

Recommended production shape:

- one AgentIM instance
- one ACP backend process or reachable ACP service
- one writable local state path
- process supervisor or container restart policy

## Multi-Instance / HA Constraints

Current constraints:

- do not share the same `state_file` across multiple replicas
- do not run multiple Telegram polling replicas for the same bot token
- webhook-only replicas still do not coordinate session state

If HA is required, future work should externalize session state to a shared store such as Redis or a database.

## Container Notes

- mount config and state directories explicitly
- inject secrets through environment variables or secret mounts
- expose `/healthz`, `/readyz`, `/reviewz`, and `/metrics`

## Dry Run Before Deploy

```bash
cargo run -- --dry-run --agent acp --acp-command acp
```
