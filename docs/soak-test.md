# Soak Test Harness

This document describes the lightweight soak test harness for AgentIM.

## Goal

Validate long-running behavior around:

- listener supervisor retry/backoff
- checkpoint persistence
- session growth
- memory stability
- webhook auth/metrics path

## Current Harness

The repository contains an ignored test placeholder:

```bash
cargo test --test soak -- --ignored
```

The test intentionally runs for a short bounded duration by default so it can be used in local smoke checks. Real production soak should run outside CI for hours.

## Recommended Production Soak

Run a deployed instance for at least 24h with:

- Telegram listener enabled
- Discord listener enabled
- state file persistence enabled
- `/metrics` scraped
- repeated ACP backend restarts

Watch:

- `agentim_webhook_requests_total`
- `agentim_webhook_failures_total`
- `agentim_agent_latency_ms`
- `agentim_active_sessions`
- process RSS memory
- state/checkpoint file size
