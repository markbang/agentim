# Codex Transport Review: app-server JSON-RPC vs ACP session/*

## Summary

The current `src/acp.rs` client speaks an ACP session transport built around:

- `initialize`
- `session/new`
- `session/load`
- `session/prompt`
- `session/update`

That transport does **not** match the Codex CLI transport exposed today.

## Evidence

### 1. Codex app-server schema exposes thread/turn JSON-RPC methods, not ACP session methods

Local verification:

```bash
tmpdir=$(mktemp -d)
codex app-server generate-json-schema --out "$tmpdir"
rg -n 'session/new|session/load|session/prompt|thread/start|turn/start|mcpServerStatus/list' "$tmpdir"
```

Observed matches:

- `thread/start`
- `turn/start`
- `mcpServerStatus/list`

Observed non-matches:

- no `session/new`
- no `session/load`
- no `session/prompt`

### 2. Codex CLI surfaces app-server and mcp-server as distinct products

Local verification:

```bash
codex app-server --help
codex mcp-server --help
```

The CLI advertises:

- `codex app-server` for the Codex app-server protocol
- `codex mcp-server` for Codex as an MCP server

Neither help surface claims ACP `session/*` compatibility.

### 3. `src/acp.rs` is hard-coded to the ACP session protocol

`src/acp.rs` currently depends on:

- `ACP_NEW_SESSION_METHOD: "session/new"`
- `ACP_LOAD_SESSION_METHOD: "session/load"`
- `ACP_PROMPT_METHOD: "session/prompt"`
- `ACP_SESSION_UPDATE_NOTIFICATION: "session/update"`

That is a protocol-level mismatch, not a naming-only issue.

## Implication

The repository should **not** present current Codex integration as “ACP-compatible” unless a real Codex transport that implements `session/new|load|prompt` is verified.

Today, the evidence supports this correction:

- **Codex bridge target:** `codex app-server` JSON-RPC
- **Not yet validated as Codex target:** `src/acp.rs` ACP session transport

## Recommended product/documentation correction

Until runtime wiring is rewritten against the Codex app-server protocol:

1. Do **not** describe `src/acp.rs` as the confirmed Codex bridge path.
2. Do **not** promise “Telegram -> Codex via ACP session/new” behavior.
3. Describe the current blocker explicitly: Codex transport is app-server JSON-RPC, while the in-repo ACP client expects ACP session methods.

## Naming / UX fallout

If the bridge is rewritten to the real Codex surface, the current ACP naming should be reconsidered:

- `--acp-command` likely becomes `--codex-command` or `--app-server-command`
- `--acp-arg` likely becomes `--codex-arg` / `--app-server-arg`
- `--acp-cwd` likely becomes `--codex-cwd` / `--app-server-cwd`
- `--acp-env` likely becomes `--codex-env` / `--app-server-env`

If compatibility with non-Codex ACP agents is still desired, keep ACP and Codex as separate transports rather than overloading one set of flags for both.

## Minimal-correct next step

Implement a dedicated Codex transport around app-server JSON-RPC (`thread/start`, `turn/start`, related notifications), then layer Telegram bridge defaults and CLI/docs on top of that verified transport.
