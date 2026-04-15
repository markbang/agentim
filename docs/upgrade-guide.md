# Upgrade Guide

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

Current runtime state is file-based JSON session persistence.
Any future schema changes should include migration notes and fallback guidance.
