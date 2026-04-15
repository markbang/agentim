# Upgrade Guide

## Session State Schema

Session state files include a `schema_version` field (introduced in v0.4.3).

- **Version 1** (current): Added `schema_version` field; existing sessions without it default to `0` and are upgraded in-memory on load.
- **Version 0** (legacy): Sessions written before schema versioning was introduced. These are automatically upgraded to the current version when loaded.

When loading a state file, AgentIM logs a warning if any sessions have an older schema version and upgrades them transparently.

## Before Upgrade

1. back up the current session state file
2. keep the previous binary available for rollback
3. run dry-run validation for the new release
4. review config changes in `README.md` and example config

## Upgrade Procedure

```bash
cargo build --release
./target/release/agentim --dry-run --config-file /path/to/config.json
```

Then restart the service with the new binary.

## Rollback Procedure

1. stop the new version
2. preserve the current state snapshot
3. restore the previous binary
4. restart with the previous configuration
5. verify `/readyz` and `/reviewz`

## Compatibility Notes

- Session state is file-based JSON persistence.
- Schema version mismatches are handled via in-memory upgrade; the on-disk file is not rewritten until the next persistence cycle.
- If rolling back to an older version that doesn't understand `schema_version`, sessions will still load correctly (the field is ignored via `#[serde(default)]`).
