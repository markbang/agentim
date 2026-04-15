# Recovery Guide

## Session State File Recovery

AgentIM supports state snapshot rotation.

If the primary state file is corrupted:

1. stop the process
2. inspect the primary state file
3. inspect rotated backups:
   - `sessions.json.bak.1`
   - `sessions.json.bak.2`
   - etc.
4. restart AgentIM and allow fallback loading to select the latest valid snapshot

## Manual Recovery Procedure

Example:

```bash
cp state/sessions.json state/sessions.json.manual-backup
ls -la state/
```

If needed, replace the broken file with the newest good backup.

## ACP Backend Failures

Symptoms:

- timeout errors
- bad gateway responses
- reconnect warnings in logs

Actions:

1. verify the ACP backend process is running
2. verify command/args/cwd/env are correct
3. run dry-run to validate startup config
4. inspect backend stderr/stdout logs

## Webhook Authentication Failures

Check:

- shared secret mismatch
- signed webhook timestamp skew
- replayed nonce
- platform-specific signature configuration

Use `/metrics` and logs to inspect auth rejects.

## Telegram Polling Recovery

If Telegram long polling stalls or duplicates appear:

- ensure only one polling instance is running
- verify no second replica is using the same token
- inspect state persistence and startup restore logs

## Rollback Guidance

Before rollback:

- stop the new process
- back up the current state file
- restore the previous binary
- restart with the same config
- confirm `/readyz` and `/reviewz` look healthy
